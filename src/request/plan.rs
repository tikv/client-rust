// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::{marker::PhantomData, sync::Arc};

use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::{future::try_join_all, prelude::*};
use tokio::sync::Semaphore;

use tikv_client_proto::{errorpb, errorpb::EpochNotMatch, kvrpcpb};
use tikv_client_store::{HasKeyErrors, HasRegionError, HasRegionErrors, KvClient};

use crate::{
    backoff::Backoff,
    pd::PdClient,
    request::{codec::RequestCodec, KvRequest, Shardable},
    stats::tikv_stats,
    store::RegionStore,
    transaction::{resolve_locks, HasLocks},
    util::iter::FlatMapOkIterExt,
    Error, Result,
};

/// A plan for how to execute a request. A user builds up a plan with various
/// options, then exectutes it.
#[async_trait]
pub trait Plan: Sized + Clone + Sync + Send + 'static {
    /// The ultimate result of executing the plan (should be a high-level type, not a GRPC response).
    type Result: Send;

    /// Execute the plan.
    async fn execute(&self) -> Result<Self::Result>;
}

/// The simplest plan which just dispatches a request to a specific kv server.
#[derive(Clone)]
pub struct Dispatch<C, Req: KvRequest<C>> {
    pub request: Req,
    pub kv_client: Option<Arc<dyn KvClient + Send + Sync>>,
    codec: C,
}

impl<C, Req: KvRequest<C>> Dispatch<C, Req> {
    pub fn new(request: Req, kv_client: Option<Arc<dyn KvClient + Send + Sync>>, codec: C) -> Self {
        Self {
            request,
            kv_client,
            codec,
        }
    }
}

#[async_trait]
impl<C: RequestCodec, Req: KvRequest<C>> Plan for Dispatch<C, Req> {
    type Result = Req::Response;

    async fn execute(&self) -> Result<Self::Result> {
        let req = self.request.encode_request(&self.codec);
        let stats = tikv_stats(self.request.label());
        let result = self
            .kv_client
            .as_ref()
            .expect("Unreachable: kv_client has not been initialised in Dispatch")
            .dispatch(req.as_ref())
            .await;
        let result = stats.done(result);
        result.and_then(|r| {
            req.decode_response(
                &self.codec,
                *r.downcast()
                    .expect("Downcast failed: request and response type mismatch"),
            )
        })
    }
}

const MULTI_REGION_CONCURRENCY: usize = 16;

pub struct RetryableMultiRegion<P: Plan, PdC: PdClient> {
    pub(super) inner: P,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,

    /// Preserve all regions' results for other downstream plans to handle.
    /// If true, return Ok and preserve all regions' results, even if some of them are Err.
    /// Otherwise, return the first Err if there is any.
    pub preserve_region_results: bool,
}

impl<P: Plan + Shardable, PdC: PdClient> RetryableMultiRegion<P, PdC>
where
    P::Result: HasKeyErrors + HasRegionError,
{
    // A plan may involve multiple shards
    #[async_recursion]
    async fn single_plan_handler(
        pd_client: Arc<PdC>,
        current_plan: P,
        backoff: Backoff,
        permits: Arc<Semaphore>,
        preserve_region_results: bool,
    ) -> Result<<Self as Plan>::Result> {
        let shards = current_plan.shards(&pd_client).collect::<Vec<_>>().await;
        let mut handles = Vec::new();
        for shard in shards {
            let (shard, region_store) = shard?;
            let mut clone = current_plan.clone();
            clone.apply_shard(shard, &region_store)?;
            let handle = tokio::spawn(Self::single_shard_handler(
                pd_client.clone(),
                clone,
                region_store,
                backoff.clone(),
                permits.clone(),
                preserve_region_results,
            ));
            handles.push(handle);
        }

        let results = try_join_all(handles).await?;
        if preserve_region_results {
            Ok(results
                .into_iter()
                .flat_map_ok(|x| x)
                .map(|x| match x {
                    Ok(r) => r,
                    Err(e) => Err(e),
                })
                .collect())
        } else {
            Ok(results
                .into_iter()
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect())
        }
    }

    #[async_recursion]
    async fn single_shard_handler(
        pd_client: Arc<PdC>,
        plan: P,
        region_store: RegionStore,
        mut backoff: Backoff,
        permits: Arc<Semaphore>,
        preserve_region_results: bool,
    ) -> Result<<Self as Plan>::Result> {
        // limit concurrent requests
        let permit = permits.acquire().await.unwrap();
        let mut resp = plan.execute().await?;
        drop(permit);

        if let Some(e) = resp.key_errors() {
            Ok(vec![Err(Error::MultipleKeyErrors(e))])
        } else if let Some(e) = resp.region_error() {
            match backoff.next_delay_duration() {
                Some(duration) => {
                    let region_error_resolved =
                        Self::handle_region_error(pd_client.clone(), e, region_store).await?;
                    // don't sleep if we have resolved the region error
                    if !region_error_resolved {
                        futures_timer::Delay::new(duration).await;
                    }
                    Self::single_plan_handler(
                        pd_client,
                        plan,
                        backoff,
                        permits,
                        preserve_region_results,
                    )
                    .await
                }
                None => Err(Error::RegionError(e)),
            }
        } else {
            Ok(vec![Ok(resp)])
        }
    }

    // Returns
    // 1. Ok(true): error has been resolved, retry immediately
    // 2. Ok(false): backoff, and then retry
    // 3. Err(Error): can't be resolved, return the error to upper level
    async fn handle_region_error(
        pd_client: Arc<PdC>,
        mut e: errorpb::Error,
        region_store: RegionStore,
    ) -> Result<bool> {
        let ver_id = region_store.region_with_leader.ver_id();
        if e.has_not_leader() {
            let not_leader = e.get_not_leader();
            if not_leader.has_leader() {
                match pd_client
                    .update_leader(
                        region_store.region_with_leader.ver_id(),
                        not_leader.get_leader().clone(),
                    )
                    .await
                {
                    Ok(_) => Ok(true),
                    Err(e) => {
                        pd_client.invalidate_region_cache(ver_id).await;
                        Err(e)
                    }
                }
            } else {
                // The peer doesn't know who is the current leader. Generally it's because
                // the Raft group is in an election, but it's possible that the peer is
                // isolated and removed from the Raft group. So it's necessary to reload
                // the region from PD.
                pd_client.invalidate_region_cache(ver_id).await;
                Ok(false)
            }
        } else if e.has_store_not_match() {
            pd_client.invalidate_region_cache(ver_id).await;
            Ok(false)
        } else if e.has_epoch_not_match() {
            Self::on_region_epoch_not_match(
                pd_client.clone(),
                region_store,
                e.take_epoch_not_match(),
            )
            .await
        } else if e.has_stale_command() || e.has_region_not_found() {
            pd_client.invalidate_region_cache(ver_id).await;
            Ok(false)
        } else if e.has_server_is_busy()
            || e.has_raft_entry_too_large()
            || e.has_max_timestamp_not_synced()
        {
            Err(Error::RegionError(e))
        } else {
            // TODO: pass the logger around
            // info!("unknwon region error: {:?}", e);
            pd_client.invalidate_region_cache(ver_id).await;
            Ok(false)
        }
    }

    // Returns
    // 1. Ok(true): error has been resolved, retry immediately
    // 2. Ok(false): backoff, and then retry
    // 3. Err(Error): can't be resolved, return the error to upper level
    async fn on_region_epoch_not_match(
        pd_client: Arc<PdC>,
        region_store: RegionStore,
        error: EpochNotMatch,
    ) -> Result<bool> {
        let ver_id = region_store.region_with_leader.ver_id();
        if error.get_current_regions().is_empty() {
            pd_client.invalidate_region_cache(ver_id).await;
            return Ok(true);
        }

        for r in error.get_current_regions() {
            if r.get_id() == region_store.region_with_leader.id() {
                let returned_conf_ver = r.get_region_epoch().get_conf_ver();
                let returned_version = r.get_region_epoch().get_version();
                let current_conf_ver = region_store
                    .region_with_leader
                    .region
                    .get_region_epoch()
                    .get_conf_ver();
                let current_version = region_store
                    .region_with_leader
                    .region
                    .get_region_epoch()
                    .get_version();

                // Find whether the current region is ahead of TiKV's. If so, backoff.
                if returned_conf_ver < current_conf_ver || returned_version < current_version {
                    return Ok(false);
                }
            }
        }
        // TODO: finer grained processing
        pd_client.invalidate_region_cache(ver_id).await;
        Ok(false)
    }
}

impl<P: Plan, PdC: PdClient> Clone for RetryableMultiRegion<P, PdC> {
    fn clone(&self) -> Self {
        RetryableMultiRegion {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
            preserve_region_results: self.preserve_region_results,
        }
    }
}

#[async_trait]
impl<P: Plan + Shardable, PdC: PdClient> Plan for RetryableMultiRegion<P, PdC>
where
    P::Result: HasKeyErrors + HasRegionError,
{
    type Result = Vec<Result<P::Result>>;

    async fn execute(&self) -> Result<Self::Result> {
        // Limit the maximum concurrency of multi-region request. If there are
        // too many concurrent requests, TiKV is more likely to return a "TiKV
        // is busy" error
        let concurrency_permits = Arc::new(Semaphore::new(MULTI_REGION_CONCURRENCY));
        Self::single_plan_handler(
            self.pd_client.clone(),
            self.inner.clone(),
            self.backoff.clone(),
            concurrency_permits.clone(),
            self.preserve_region_results,
        )
        .await
    }
}

/// A technique for merging responses into a single result (with type `Out`).
pub trait Merge<In>: Sized + Clone + Send + Sync + 'static {
    type Out: Send;

    fn merge(&self, input: Vec<Result<In>>) -> Result<Self::Out>;
}

#[derive(Clone)]
pub struct MergeResponse<P: Plan, In, M: Merge<In>> {
    pub inner: P,
    pub merge: M,
    pub phantom: PhantomData<In>,
}

#[async_trait]
impl<In: Clone + Send + Sync + 'static, P: Plan<Result = Vec<Result<In>>>, M: Merge<In>> Plan
    for MergeResponse<P, In, M>
{
    type Result = M::Out;

    async fn execute(&self) -> Result<Self::Result> {
        self.merge.merge(self.inner.execute().await?)
    }
}

/// A merge strategy which collects data from a response into a single type.
#[derive(Clone, Copy)]
pub struct Collect;

/// A merge strategy that only takes the first element. It's used for requests
/// that should have exactly one response, e.g. a get request.
#[derive(Clone, Copy)]
pub struct CollectSingle;

#[macro_export]
macro_rules! collect_first {
    ($type_: ty) => {
        impl Merge<$type_> for CollectSingle {
            type Out = $type_;

            fn merge(&self, mut input: Vec<Result<$type_>>) -> Result<Self::Out> {
                assert!(input.len() == 1);
                input.pop().unwrap()
            }
        }
    };
}

/// A merge strategy to be used with
/// [`preserve_shard`](super::plan_builder::PlanBuilder::preserve_shard).
/// It matches the shards preserved before and the values returned in the response.
#[derive(Clone, Debug)]
pub struct CollectWithShard;

/// A merge strategy which returns an error if any response is an error and
/// otherwise returns a Vec of the results.
#[derive(Clone, Copy)]
pub struct CollectError;

impl<T: Send> Merge<T> for CollectError {
    type Out = Vec<T>;

    fn merge(&self, input: Vec<Result<T>>) -> Result<Self::Out> {
        input.into_iter().collect()
    }
}

/// Process data into another kind of data.
pub trait Process<In>: Sized + Clone + Send + Sync + 'static {
    type Out: Send;

    fn process(&self, input: Result<In>) -> Result<Self::Out>;
}

#[derive(Clone)]
pub struct ProcessResponse<P: Plan, Pr: Process<P::Result>> {
    pub inner: P,
    pub processor: Pr,
}

#[async_trait]
impl<P: Plan, Pr: Process<P::Result>> Plan for ProcessResponse<P, Pr> {
    type Result = Pr::Out;

    async fn execute(&self) -> Result<Self::Result> {
        self.processor.process(self.inner.execute().await)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DefaultProcessor;

pub struct ResolveLock<P: Plan, PdC: PdClient> {
    pub inner: P,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,
}

impl<P: Plan, PdC: PdClient> Clone for ResolveLock<P, PdC> {
    fn clone(&self) -> Self {
        ResolveLock {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan, PdC: PdClient> Plan for ResolveLock<P, PdC>
where
    P::Result: HasLocks,
{
    type Result = P::Result;

    async fn execute(&self) -> Result<Self::Result> {
        let mut result = self.inner.execute().await?;
        let mut clone = self.clone();
        loop {
            let locks = result.take_locks();
            if locks.is_empty() {
                return Ok(result);
            }

            if self.backoff.is_none() {
                return Err(Error::ResolveLockError);
            }

            let pd_client = self.pd_client.clone();
            if resolve_locks(locks, pd_client.clone()).await? {
                result = self.inner.execute().await?;
            } else {
                match clone.backoff.next_delay_duration() {
                    None => return Err(Error::ResolveLockError),
                    Some(delay_duration) => {
                        futures_timer::Delay::new(delay_duration).await;
                        result = clone.inner.execute().await?;
                    }
                }
            }
        }
    }
}

/// When executed, the plan extracts errors from its inner plan, and returns an
/// `Err` wrapping the error.
///
/// We usually need to apply this plan if (and only if) the output of the inner
/// plan is of a response type.
///
/// The errors come from two places: `Err` from inner plans, and `Ok(response)`
/// where `response` contains unresolved errors (`error` and `region_error`).
pub struct ExtractError<P: Plan> {
    pub inner: P,
}

impl<P: Plan> Clone for ExtractError<P> {
    fn clone(&self) -> Self {
        ExtractError {
            inner: self.inner.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan> Plan for ExtractError<P>
where
    P::Result: HasKeyErrors + HasRegionErrors,
{
    type Result = P::Result;

    async fn execute(&self) -> Result<Self::Result> {
        let mut result = self.inner.execute().await?;
        if let Some(errors) = result.key_errors() {
            Err(Error::ExtractedErrors(errors))
        } else if let Some(errors) = result.region_errors() {
            Err(Error::ExtractedErrors(
                errors.into_iter().map(Error::RegionError).collect(),
            ))
        } else {
            Ok(result)
        }
    }
}

/// When executed, the plan clones the shard and execute its inner plan, then
/// returns `(shard, response)`.
///
/// It's useful when the information of shard are lost in the response but needed
/// for processing.
pub struct PreserveShard<P: Plan + Shardable> {
    pub inner: P,
    pub shard: Option<P::Shard>,
}

impl<P: Plan + Shardable> Clone for PreserveShard<P> {
    fn clone(&self) -> Self {
        PreserveShard {
            inner: self.inner.clone(),
            shard: None,
        }
    }
}

#[async_trait]
impl<P> Plan for PreserveShard<P>
where
    P: Plan + Shardable,
{
    type Result = ResponseWithShard<P::Result, P::Shard>;

    async fn execute(&self) -> Result<Self::Result> {
        let res = self.inner.execute().await?;
        let shard = self
            .shard
            .as_ref()
            .expect("Unreachable: Shardable::apply_shard() is not called before executing PreserveShard")
            .clone();
        Ok(ResponseWithShard(res, shard))
    }
}

// contains a response and the corresponding shards
#[derive(Debug, Clone)]
pub struct ResponseWithShard<Resp, Shard>(pub Resp, pub Shard);

impl<Resp: HasKeyErrors, Shard> HasKeyErrors for ResponseWithShard<Resp, Shard> {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        self.0.key_errors()
    }
}

impl<Resp: HasLocks, Shard> HasLocks for ResponseWithShard<Resp, Shard> {
    fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
        self.0.take_locks()
    }
}

impl<Resp: HasRegionError, Shard> HasRegionError for ResponseWithShard<Resp, Shard> {
    fn region_error(&mut self) -> Option<errorpb::Error> {
        self.0.region_error()
    }
}

#[cfg(test)]
mod test {
    use futures::stream::{self, BoxStream};

    use tikv_client_proto::kvrpcpb::BatchGetResponse;

    use crate::mock::MockPdClient;

    use super::*;

    #[derive(Clone)]
    struct ErrPlan;

    #[async_trait]
    impl Plan for ErrPlan {
        type Result = BatchGetResponse;

        async fn execute(&self) -> Result<Self::Result> {
            Err(Error::Unimplemented)
        }
    }

    impl Shardable for ErrPlan {
        type Shard = ();

        fn shards(
            &self,
            _: &Arc<impl crate::pd::PdClient>,
        ) -> BoxStream<'static, crate::Result<(Self::Shard, crate::store::RegionStore)>> {
            Box::pin(stream::iter(1..=3).map(|_| Err(Error::Unimplemented))).boxed()
        }

        fn apply_shard(&mut self, _: Self::Shard, _: &crate::store::RegionStore) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_err() {
        let plan = RetryableMultiRegion {
            inner: ResolveLock {
                inner: ErrPlan,
                backoff: Backoff::no_backoff(),
                pd_client: Arc::new(MockPdClient::default()),
            },
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_backoff(),
            preserve_region_results: false,
        };
        assert!(plan.execute().await.is_err())
    }
}
