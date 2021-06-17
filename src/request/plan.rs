// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    backoff::Backoff,
    pd::PdClient,
    request::{KvRequest, Shardable},
    stats::tikv_stats,
    store::RegionStore,
    transaction::{resolve_locks, HasLocks},
    util::iter::FlatMapOkIterExt,
    Error, Key, KvPair, Result, Value,
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::{future::try_join_all, stream::StreamExt};
use std::{collections::VecDeque, marker::PhantomData, sync::Arc};
use tikv_client_proto::kvrpcpb;
use tikv_client_store::{HasError, HasRegionError, KvClient};
use tokio::sync::Semaphore;

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
pub struct Dispatch<Req: KvRequest> {
    pub request: Req,
    pub kv_client: Option<Arc<dyn KvClient + Send + Sync>>,
}

#[async_trait]
impl<Req: KvRequest> Plan for Dispatch<Req> {
    type Result = Req::Response;

    async fn execute(&self) -> Result<Self::Result> {
        let stats = tikv_stats(self.request.label());
        let result = self
            .kv_client
            .as_ref()
            .ok_or_else(|| {
                Error::StringError(
                    "Unreachable: kv_client has not been initialised in Dispatch".to_owned(),
                )
            })?
            .dispatch(&self.request)
            .await;
        let result = stats.done(result);
        result.map(|r| {
            *r.downcast()
                .expect("Downcast failed: request and response type mismatch")
        })
    }
}

impl<Req: KvRequest + HasKeys> HasKeys for Dispatch<Req> {
    fn get_keys(&self) -> Vec<Key> {
        self.request.get_keys()
    }
}

const MULTI_REGION_CONCURRENCY: usize = 16;

pub struct RetryableMultiRegion<P: Plan, PdC: PdClient> {
    pub(super) inner: P,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,
}

impl<P: Plan + Shardable, PdC: PdClient> RetryableMultiRegion<P, PdC>
where
    P::Result: HasError,
{
    // A plan may involve multiple shards
    #[async_recursion]
    async fn single_plan_handler(
        pd_client: Arc<PdC>,
        current_plan: P,
        backoff: Backoff,
        permits: Arc<Semaphore>,
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
            ));
            handles.push(handle);
        }
        Ok(try_join_all(handles)
            .await?
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect())
    }

    #[async_recursion]
    async fn single_shard_handler(
        pd_client: Arc<PdC>,
        plan: P,
        region_store: RegionStore,
        mut backoff: Backoff,
        permits: Arc<Semaphore>,
    ) -> Result<<Self as Plan>::Result> {
        // limit concurrent requests
        let permit = permits.acquire().await;
        let mut resp = plan.execute().await?;
        drop(permit);

        if let Some(e) = resp.error() {
            Ok(vec![Err(e)])
        } else if let Some(e) = resp.region_error() {
            match backoff.next_delay_duration() {
                Some(duration) => {
                    let ver_id = region_store.region.ver_id();
                    let region_error_resolved =
                        Self::handle_region_error(pd_client.clone(), e, region_store).await?;
                    // don't sleep if we have resolved the region error
                    if !region_error_resolved {
                        // TODO: should we move the invalidation to `handle_region_error`?
                        pd_client.invalidate_region_cache(ver_id).await;
                        futures_timer::Delay::new(duration).await;
                    }
                    Self::single_plan_handler(pd_client, plan, backoff, permits).await
                }
                None => Err(e),
            }
        } else {
            Ok(vec![Ok(resp)])
        }
    }

    // Returns whether the region error is successfully handled.
    // Returns error if the region error shouldn't be retried.
    async fn handle_region_error(
        pd_client: Arc<PdC>,
        error: Error,
        region_store: RegionStore,
    ) -> Result<bool> {
        match error {
            Error::RegionError(e) => {
                if e.has_not_leader() {
                    let not_leader = e.get_not_leader();
                    if not_leader.has_leader() {
                        Ok(pd_client
                            .update_leader(
                                region_store.region.ver_id(),
                                not_leader.get_leader().clone(),
                            )
                            .await
                            .is_ok())
                    } else {
                        // The peer doesn't know who is the current leader. Generally it's because
                        // the Raft group is in an election, but it's possible that the peer is
                        // isolated and removed from the Raft group. So it's necessary to reload
                        // the region from PD.
                        pd_client
                            .invalidate_region_cache(region_store.region.ver_id())
                            .await;
                        Ok(false)
                    }
                } else if e.has_store_not_match() {
                    pd_client
                        .invalidate_region_cache(region_store.region.ver_id())
                        .await;
                    Ok(false)
                } else if e.has_epoch_not_match() {
                    // TODO
                    Err(Error::RegionError(e))
                } else if e.has_server_is_busy()
                    || e.has_stale_command()
                    || e.has_region_not_found()
                    || e.has_max_timestamp_not_synced()
                {
                    Ok(false)
                } else if e.has_raft_entry_too_large() {
                    Err(Error::RegionError(e))
                } else {
                    pd_client
                        .invalidate_region_cache(region_store.region.ver_id())
                        .await;
                    Ok(false)
                }
            }
            // TODO: whether to retry here?
            Error::RegionForKeyNotFound { .. }
            | Error::RegionNotFoundInResponse { .. }
            | Error::LeaderNotFound { .. } => Ok(false),
            _ => {
                unreachable!()
            }
        }
    }
}

impl<P: Plan, PdC: PdClient> Clone for RetryableMultiRegion<P, PdC> {
    fn clone(&self) -> Self {
        RetryableMultiRegion {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan + Shardable, PdC: PdClient> Plan for RetryableMultiRegion<P, PdC>
where
    P::Result: HasError,
{
    type Result = Vec<Result<P::Result>>;

    async fn execute(&self) -> Result<Self::Result> {
        // The plans to be executed. There can be more children plans because one
        // shard can become more if it returns region error.
        let mut children_plans = VecDeque::new();
        children_plans.push_back((self.inner.clone(), self.backoff.clone()));

        // Limit the maximum concurrency of multi-region request. If there are
        // too many concurrent requests, TiKV is more likely to return a "TiKV
        // is busy" error
        let concurrency_permits = Arc::new(Semaphore::new(MULTI_REGION_CONCURRENCY));
        Self::single_plan_handler(
            self.pd_client.clone(),
            self.inner.clone(),
            self.backoff.clone(),
            concurrency_permits.clone(),
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

#[derive(Clone, Copy)]
pub struct CollectSingleKey;

#[macro_export]
macro_rules! collect_first {
    ($type_: ty) => {
        impl Merge<$type_> for CollectSingleKey {
            type Out = $type_;

            fn merge(&self, mut input: Vec<Result<$type_>>) -> Result<Self::Out> {
                assert!(input.len() == 1);
                input.pop().unwrap()
            }
        }
    };
}

/// A merge strategy to be used with
/// [`preserve_keys`](super::plan_builder::PlanBuilder::preserve_keys).
/// It matches the keys preserved before and the values returned in the response.
#[derive(Clone, Debug)]
pub struct CollectAndMatchKey;

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
pub struct ProcessResponse<P: Plan, In, Pr: Process<In>> {
    pub inner: P,
    pub processor: Pr,
    pub phantom: PhantomData<In>,
}

#[async_trait]
impl<In: Clone + Sync + Send + 'static, P: Plan<Result = In>, Pr: Process<In>> Plan
    for ProcessResponse<P, In, Pr>
{
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
            // FIXME: read_through_cache in resolve_locks?
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

impl<P: Plan + HasKeys, PdC: PdClient> HasKeys for ResolveLock<P, PdC> {
    fn get_keys(&self) -> Vec<Key> {
        self.inner.get_keys()
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
    P::Result: HasError,
{
    type Result = P::Result;

    async fn execute(&self) -> Result<Self::Result> {
        let mut result = self.inner.execute().await?;
        if let Some(error) = result.error() {
            Err(error)
        } else if let Some(error) = result.region_error() {
            Err(error)
        } else {
            Ok(result)
        }
    }
}

/// When executed, the plan clones the keys and execute its inner plan, then
/// returns `(keys, response)`.
///
/// It's useful when the information of keys are lost in the response but needed
/// for processing.
pub struct PreserveKey<P: Plan + HasKeys> {
    pub inner: P,
}

impl<P: Plan + HasKeys> Clone for PreserveKey<P> {
    fn clone(&self) -> Self {
        PreserveKey {
            inner: self.inner.clone(),
        }
    }
}

#[async_trait]
impl<P> Plan for PreserveKey<P>
where
    P: Plan + HasKeys,
{
    type Result = ResponseAndKeys<P::Result>;

    async fn execute(&self) -> Result<Self::Result> {
        let keys = self.inner.get_keys();
        let res = self.inner.execute().await?;
        Ok(ResponseAndKeys(res, keys))
    }
}

pub trait HasKeys {
    fn get_keys(&self) -> Vec<Key>;
}

// contains a response and the corresponding keys
// currently only used for matching keys and values in pessimistic lock requests
#[derive(Debug, Clone)]
pub struct ResponseAndKeys<Resp>(Resp, Vec<Key>);

impl<Resp: HasError> HasError for ResponseAndKeys<Resp> {
    fn error(&mut self) -> Option<Error> {
        self.0.error()
    }
}

impl<Resp: HasLocks> HasLocks for ResponseAndKeys<Resp> {
    fn take_locks(&mut self) -> Vec<tikv_client_proto::kvrpcpb::LockInfo> {
        self.0.take_locks()
    }
}

impl<Resp: HasRegionError> HasRegionError for ResponseAndKeys<Resp> {
    fn region_error(&mut self) -> Option<Error> {
        self.0.region_error()
    }
}

impl Merge<ResponseAndKeys<kvrpcpb::PessimisticLockResponse>> for CollectAndMatchKey {
    type Out = Vec<KvPair>;

    fn merge(
        &self,
        input: Vec<Result<ResponseAndKeys<kvrpcpb::PessimisticLockResponse>>>,
    ) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|ResponseAndKeys(mut resp, keys)| {
                let values = resp.take_values();
                let not_founds = resp.take_not_founds();
                let v: Vec<_> = if not_founds.is_empty() {
                    // Legacy TiKV does not distiguish not existing key and existing key
                    // that with empty value. We assume that key does not exist if value
                    // is empty.
                    let values: Vec<Value> = values.into_iter().filter(|v| v.is_empty()).collect();
                    keys.into_iter().zip(values).map(From::from).collect()
                } else {
                    assert_eq!(values.len(), not_founds.len());
                    let values: Vec<Value> = values
                        .into_iter()
                        .zip(not_founds.into_iter())
                        .filter_map(|(v, not_found)| if not_found { None } else { Some(v) })
                        .collect();
                    keys.into_iter().zip(values).map(From::from).collect()
                };
                // FIXME sucks to collect and re-iterate, but the iterators have different types
                v.into_iter()
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::mock::{mock_store, MockPdClient};
    use futures::{
        stream::{self, BoxStream},
        TryStreamExt,
    };
    use tikv_client_proto::kvrpcpb::BatchGetResponse;

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
        type Shard = u8;

        fn shards(
            &self,
            _: &Arc<impl crate::pd::PdClient>,
        ) -> BoxStream<'static, crate::Result<(Self::Shard, crate::store::RegionStore)>> {
            Box::pin(stream::iter(1..=3).map(|_| Err(Error::Unimplemented)))
                .map_ok(|_: u8| (42, mock_store()))
                .boxed()
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
        };
        assert!(plan.execute().await.is_err())
    }
}
