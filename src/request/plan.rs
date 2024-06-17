// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;
use std::sync::Arc;

use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::future::try_join_all;
use futures::prelude::*;
use log::debug;
use log::info;
use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::backoff::Backoff;
use crate::pd::PdClient;
use crate::proto::errorpb;
use crate::proto::errorpb::EpochNotMatch;
use crate::proto::kvrpcpb;
use crate::request::shard::HasNextBatch;
use crate::request::NextBatch;
use crate::request::Shardable;
use crate::request::{KvRequest, StoreRequest};
use crate::stats::tikv_stats;
use crate::store::HasRegionError;
use crate::store::HasRegionErrors;
use crate::store::KvClient;
use crate::store::RegionStore;
use crate::store::{HasKeyErrors, Store};
use crate::transaction::resolve_locks;
use crate::transaction::HasLocks;
use crate::transaction::ResolveLocksContext;
use crate::transaction::ResolveLocksOptions;
use crate::util::iter::FlatMapOkIterExt;
use crate::Error;
use crate::Result;

use super::keyspace::Keyspace;

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
            .expect("Unreachable: kv_client has not been initialised in Dispatch")
            .dispatch(&self.request)
            .await;
        let result = stats.done(result);
        result.map(|r| {
            *r.downcast()
                .expect("Downcast failed: request and response type mismatch")
        })
    }
}

impl<Req: KvRequest + StoreRequest> StoreRequest for Dispatch<Req> {
    fn apply_store(&mut self, store: &Store) {
        self.kv_client = Some(store.client.clone());
        self.request.apply_store(store);
    }
}

const MULTI_REGION_CONCURRENCY: usize = 16;
const MULTI_STORES_CONCURRENCY: usize = 16;

fn is_grpc_error(e: &Error) -> bool {
    matches!(e, Error::GrpcAPI(_) | Error::Grpc(_))
}

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
        let res = plan.execute().await;
        drop(permit);

        let mut resp = match res {
            Ok(resp) => resp,
            Err(e) if is_grpc_error(&e) => {
                return Self::handle_grpc_error(
                    pd_client,
                    plan,
                    region_store,
                    backoff,
                    permits,
                    preserve_region_results,
                    e,
                )
                .await;
            }
            Err(e) => return Err(e),
        };

        if let Some(e) = resp.key_errors() {
            Ok(vec![Err(Error::MultipleKeyErrors(e))])
        } else if let Some(e) = resp.region_error() {
            match backoff.next_delay_duration() {
                Some(duration) => {
                    let region_error_resolved =
                        handle_region_error(pd_client.clone(), e, region_store).await?;
                    // don't sleep if we have resolved the region error
                    if !region_error_resolved {
                        sleep(duration).await;
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
                None => Err(Error::RegionError(Box::new(e))),
            }
        } else {
            Ok(vec![Ok(resp)])
        }
    }

    async fn handle_grpc_error(
        pd_client: Arc<PdC>,
        plan: P,
        region_store: RegionStore,
        mut backoff: Backoff,
        permits: Arc<Semaphore>,
        preserve_region_results: bool,
        e: Error,
    ) -> Result<<Self as Plan>::Result> {
        debug!("handle grpc error: {:?}", e);
        let ver_id = region_store.region_with_leader.ver_id();
        pd_client.invalidate_region_cache(ver_id).await;
        match backoff.next_delay_duration() {
            Some(duration) => {
                sleep(duration).await;
                Self::single_plan_handler(
                    pd_client,
                    plan,
                    backoff,
                    permits,
                    preserve_region_results,
                )
                .await
            }
            None => Err(e),
        }
    }
}

// Returns
// 1. Ok(true): error has been resolved, retry immediately
// 2. Ok(false): backoff, and then retry
// 3. Err(Error): can't be resolved, return the error to upper level
pub(crate) async fn handle_region_error<PdC: PdClient>(
    pd_client: Arc<PdC>,
    e: errorpb::Error,
    region_store: RegionStore,
) -> Result<bool> {
    let ver_id = region_store.region_with_leader.ver_id();
    if let Some(not_leader) = e.not_leader {
        if let Some(leader) = not_leader.leader {
            match pd_client
                .update_leader(region_store.region_with_leader.ver_id(), leader)
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
    } else if e.store_not_match.is_some() {
        pd_client.invalidate_region_cache(ver_id).await;
        Ok(false)
    } else if e.epoch_not_match.is_some() {
        on_region_epoch_not_match(pd_client.clone(), region_store, e.epoch_not_match.unwrap()).await
    } else if e.stale_command.is_some() || e.region_not_found.is_some() {
        pd_client.invalidate_region_cache(ver_id).await;
        Ok(false)
    } else if e.server_is_busy.is_some()
        || e.raft_entry_too_large.is_some()
        || e.max_timestamp_not_synced.is_some()
    {
        Err(Error::RegionError(Box::new(e)))
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
pub(crate) async fn on_region_epoch_not_match<PdC: PdClient>(
    pd_client: Arc<PdC>,
    region_store: RegionStore,
    error: EpochNotMatch,
) -> Result<bool> {
    let ver_id = region_store.region_with_leader.ver_id();
    if error.current_regions.is_empty() {
        pd_client.invalidate_region_cache(ver_id).await;
        return Ok(true);
    }

    for r in error.current_regions {
        if r.id == region_store.region_with_leader.id() {
            let region_epoch = r.region_epoch.unwrap();
            let returned_conf_ver = region_epoch.conf_ver;
            let returned_version = region_epoch.version;
            let current_region_epoch = region_store
                .region_with_leader
                .region
                .region_epoch
                .clone()
                .unwrap();
            let current_conf_ver = current_region_epoch.conf_ver;
            let current_version = current_region_epoch.version;

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

pub struct RetryableAllStores<P: Plan, PdC: PdClient> {
    pub(super) inner: P,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,
}

impl<P: Plan, PdC: PdClient> Clone for RetryableAllStores<P, PdC> {
    fn clone(&self) -> Self {
        RetryableAllStores {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
        }
    }
}

// About `HasRegionError`:
// Store requests should be return region errors.
// But as the response of only store request by now (UnsafeDestroyRangeResponse) has the `region_error` field,
// we require `HasRegionError` to check whether there is region error returned from TiKV.
#[async_trait]
impl<P: Plan + StoreRequest, PdC: PdClient> Plan for RetryableAllStores<P, PdC>
where
    P::Result: HasKeyErrors + HasRegionError,
{
    type Result = Vec<Result<P::Result>>;

    async fn execute(&self) -> Result<Self::Result> {
        let concurrency_permits = Arc::new(Semaphore::new(MULTI_STORES_CONCURRENCY));
        let stores = self.pd_client.clone().all_stores().await?;
        let mut handles = Vec::with_capacity(stores.len());
        for store in stores {
            let mut clone = self.inner.clone();
            clone.apply_store(&store);
            let handle = tokio::spawn(Self::single_store_handler(
                clone,
                self.backoff.clone(),
                concurrency_permits.clone(),
            ));
            handles.push(handle);
        }
        let results = try_join_all(handles).await?;
        Ok(results.into_iter().collect::<Vec<_>>())
    }
}

impl<P: Plan, PdC: PdClient> RetryableAllStores<P, PdC>
where
    P::Result: HasKeyErrors + HasRegionError,
{
    async fn single_store_handler(
        plan: P,
        mut backoff: Backoff,
        permits: Arc<Semaphore>,
    ) -> Result<P::Result> {
        loop {
            let permit = permits.acquire().await.unwrap();
            let res = plan.execute().await;
            drop(permit);

            match res {
                Ok(mut resp) => {
                    if let Some(e) = resp.key_errors() {
                        return Err(Error::MultipleKeyErrors(e));
                    } else if let Some(e) = resp.region_error() {
                        // Store request should not return region error.
                        return Err(Error::RegionError(Box::new(e)));
                    } else {
                        return Ok(resp);
                    }
                }
                Err(e) if is_grpc_error(&e) => match backoff.next_delay_duration() {
                    Some(duration) => {
                        sleep(duration).await;
                        continue;
                    }
                    None => return Err(e),
                },
                Err(e) => return Err(e),
            }
        }
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

#[doc(hidden)]
#[macro_export]
macro_rules! collect_single {
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
    pub keyspace: Keyspace,
}

impl<P: Plan, PdC: PdClient> Clone for ResolveLock<P, PdC> {
    fn clone(&self) -> Self {
        ResolveLock {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
            keyspace: self.keyspace,
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
                return Err(Error::ResolveLockError(locks));
            }

            let pd_client = self.pd_client.clone();
            let live_locks = resolve_locks(locks, pd_client.clone(), self.keyspace).await?;
            if live_locks.is_empty() {
                result = self.inner.execute().await?;
            } else {
                match clone.backoff.next_delay_duration() {
                    None => return Err(Error::ResolveLockError(live_locks)),
                    Some(delay_duration) => {
                        sleep(delay_duration).await;
                        result = clone.inner.execute().await?;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct CleanupLocksResult {
    pub region_error: Option<errorpb::Error>,
    pub key_error: Option<Vec<Error>>,
    pub resolved_locks: usize,
}

impl Clone for CleanupLocksResult {
    fn clone(&self) -> Self {
        Self {
            resolved_locks: self.resolved_locks,
            ..Default::default() // Ignore errors, which should be extracted by `extract_error()`.
        }
    }
}

impl HasRegionError for CleanupLocksResult {
    fn region_error(&mut self) -> Option<errorpb::Error> {
        self.region_error.take()
    }
}

impl HasKeyErrors for CleanupLocksResult {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        self.key_error.take()
    }
}

impl Merge<CleanupLocksResult> for Collect {
    type Out = CleanupLocksResult;

    fn merge(&self, input: Vec<Result<CleanupLocksResult>>) -> Result<Self::Out> {
        input
            .into_iter()
            .try_fold(CleanupLocksResult::default(), |acc, x| {
                Ok(CleanupLocksResult {
                    resolved_locks: acc.resolved_locks + x?.resolved_locks,
                    ..Default::default()
                })
            })
    }
}

pub struct CleanupLocks<P: Plan, PdC: PdClient> {
    pub inner: P,
    pub ctx: ResolveLocksContext,
    pub options: ResolveLocksOptions,
    pub store: Option<RegionStore>,
    pub pd_client: Arc<PdC>,
    pub keyspace: Keyspace,
    pub backoff: Backoff,
}

impl<P: Plan, PdC: PdClient> Clone for CleanupLocks<P, PdC> {
    fn clone(&self) -> Self {
        CleanupLocks {
            inner: self.inner.clone(),
            ctx: self.ctx.clone(),
            options: self.options,
            store: None,
            pd_client: self.pd_client.clone(),
            keyspace: self.keyspace,
            backoff: self.backoff.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan + Shardable + NextBatch, PdC: PdClient> Plan for CleanupLocks<P, PdC>
where
    P::Result: HasLocks + HasNextBatch + HasKeyErrors + HasRegionError,
{
    type Result = CleanupLocksResult;

    async fn execute(&self) -> Result<Self::Result> {
        let mut result = CleanupLocksResult::default();
        let mut inner = self.inner.clone();
        let mut lock_resolver = crate::transaction::LockResolver::new(self.ctx.clone());
        let region = &self.store.as_ref().unwrap().region_with_leader;
        let mut has_more_batch = true;

        while has_more_batch {
            let mut scan_lock_resp = inner.execute().await?;

            // Propagate errors to `retry_multi_region` for retry.
            if let Some(e) = scan_lock_resp.key_errors() {
                info!("CleanupLocks::execute, inner key errors:{:?}", e);
                result.key_error = Some(e);
                return Ok(result);
            } else if let Some(e) = scan_lock_resp.region_error() {
                info!("CleanupLocks::execute, inner region error:{}", e.message);
                result.region_error = Some(e);
                return Ok(result);
            }

            // Iterate to next batch of inner.
            match scan_lock_resp.has_next_batch() {
                Some(range) if region.contains(range.0.as_ref()) => {
                    debug!("CleanupLocks::execute, next range:{:?}", range);
                    inner.next_batch(range);
                }
                _ => has_more_batch = false,
            }

            let mut locks = scan_lock_resp.take_locks();
            if locks.is_empty() {
                break;
            }
            if locks.len() < self.options.batch_size as usize {
                has_more_batch = false;
            }

            if self.options.async_commit_only {
                locks = locks
                    .into_iter()
                    .filter(|l| l.use_async_commit)
                    .collect::<Vec<_>>();
            }
            debug!("CleanupLocks::execute, meet locks:{}", locks.len());

            let lock_size = locks.len();
            match lock_resolver
                .cleanup_locks(
                    self.store.clone().unwrap(),
                    locks,
                    self.pd_client.clone(),
                    self.keyspace,
                )
                .await
            {
                Ok(()) => {
                    result.resolved_locks += lock_size;
                }
                Err(Error::ExtractedErrors(mut errors)) => {
                    // Propagate errors to `retry_multi_region` for retry.
                    if let Error::RegionError(e) = errors.pop().unwrap() {
                        result.region_error = Some(*e);
                    } else {
                        result.key_error = Some(errors);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    return Err(e);
                }
            }

            // TODO: improve backoff
            // if self.backoff.is_none() {
            //     return Err(Error::ResolveLockError);
            // }
        }

        Ok(result)
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
                errors
                    .into_iter()
                    .map(|e| Error::RegionError(Box::new(e)))
                    .collect(),
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
    use futures::stream::BoxStream;
    use futures::stream::{self};

    use super::*;
    use crate::mock::MockPdClient;
    use crate::proto::kvrpcpb::BatchGetResponse;

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
                keyspace: Keyspace::Disable,
            },
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_backoff(),
            preserve_region_results: false,
        };
        assert!(plan.execute().await.is_err())
    }
}
