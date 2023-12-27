// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;
use std::sync::Arc;

use super::plan::PreserveShard;
use super::Keyspace;
use crate::backoff::Backoff;
use crate::pd::PdClient;
use crate::request::plan::{CleanupLocks, RetryableAllStores};
use crate::request::shard::HasNextBatch;
use crate::request::Dispatch;
use crate::request::ExtractError;
use crate::request::KvRequest;
use crate::request::Merge;
use crate::request::MergeResponse;
use crate::request::NextBatch;
use crate::request::Plan;
use crate::request::Process;
use crate::request::ProcessResponse;
use crate::request::ResolveLock;
use crate::request::RetryableMultiRegion;
use crate::request::Shardable;
use crate::request::{DefaultProcessor, StoreRequest};
use crate::store::HasKeyErrors;
use crate::store::HasRegionError;
use crate::store::HasRegionErrors;
use crate::store::RegionStore;
use crate::transaction::HasLocks;
use crate::transaction::ResolveLocksContext;
use crate::transaction::ResolveLocksOptions;
use crate::Result;

/// Builder type for plans (see that module for more).
pub struct PlanBuilder<PdC: PdClient, P: Plan, Ph: PlanBuilderPhase> {
    pd_client: Arc<PdC>,
    plan: P,
    phantom: PhantomData<Ph>,
}

/// Used to ensure that a plan has a designated target or targets, a target is
/// a particular TiKV server.
pub trait PlanBuilderPhase {}
pub struct NoTarget;
impl PlanBuilderPhase for NoTarget {}
pub struct Targetted;
impl PlanBuilderPhase for Targetted {}

impl<PdC: PdClient, Req: KvRequest> PlanBuilder<PdC, Dispatch<Req>, NoTarget> {
    pub fn new(pd_client: Arc<PdC>, keyspace: Keyspace, mut request: Req) -> Self {
        request.set_api_version(keyspace.api_version());
        PlanBuilder {
            pd_client,
            plan: Dispatch {
                request,
                kv_client: None,
            },
            phantom: PhantomData,
        }
    }
}

impl<PdC: PdClient, P: Plan> PlanBuilder<PdC, P, Targetted> {
    /// Return the built plan, note that this can only be called once the plan
    /// has a target.
    pub fn plan(self) -> P {
        self.plan
    }
}

impl<PdC: PdClient, P: Plan, Ph: PlanBuilderPhase> PlanBuilder<PdC, P, Ph> {
    /// If there is a lock error, then resolve the lock and retry the request.
    pub fn resolve_lock(
        self,
        backoff: Backoff,
        keyspace: Keyspace,
    ) -> PlanBuilder<PdC, ResolveLock<P, PdC>, Ph>
    where
        P::Result: HasLocks,
    {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            plan: ResolveLock {
                inner: self.plan,
                backoff,
                pd_client: self.pd_client,
                keyspace,
            },
            phantom: PhantomData,
        }
    }

    pub fn cleanup_locks(
        self,
        ctx: ResolveLocksContext,
        options: ResolveLocksOptions,
        backoff: Backoff,
        keyspace: Keyspace,
    ) -> PlanBuilder<PdC, CleanupLocks<P, PdC>, Ph>
    where
        P: Shardable + NextBatch,
        P::Result: HasLocks + HasNextBatch + HasRegionError + HasKeyErrors,
    {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            plan: CleanupLocks {
                inner: self.plan,
                ctx,
                options,
                store: None,
                backoff,
                pd_client: self.pd_client,
                keyspace,
            },
            phantom: PhantomData,
        }
    }

    /// Merge the results of a request. Usually used where a request is sent to multiple regions
    /// to combine the responses from each region.
    pub fn merge<In, M: Merge<In>>(self, merge: M) -> PlanBuilder<PdC, MergeResponse<P, In, M>, Ph>
    where
        In: Clone + Send + Sync + 'static,
        P: Plan<Result = Vec<Result<In>>>,
    {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            plan: MergeResponse {
                inner: self.plan,
                merge,
                phantom: PhantomData,
            },
            phantom: PhantomData,
        }
    }

    /// Apply the default processing step to a response (usually only needed if the request is sent
    /// to a single region because post-porcessing can be incorporated in the merge step for
    /// multi-region requests).
    pub fn post_process_default(self) -> PlanBuilder<PdC, ProcessResponse<P, DefaultProcessor>, Ph>
    where
        P: Plan,
        DefaultProcessor: Process<P::Result>,
    {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            plan: ProcessResponse {
                inner: self.plan,
                processor: DefaultProcessor,
            },
            phantom: PhantomData,
        }
    }
}

impl<PdC: PdClient, P: Plan + Shardable> PlanBuilder<PdC, P, NoTarget>
where
    P::Result: HasKeyErrors + HasRegionError,
{
    /// Split the request into shards sending a request to the region of each shard.
    pub fn retry_multi_region(
        self,
        backoff: Backoff,
    ) -> PlanBuilder<PdC, RetryableMultiRegion<P, PdC>, Targetted> {
        self.make_retry_multi_region(backoff, false)
    }

    /// Preserve all results, even some of them are Err.
    /// To pass all responses to merge, and handle partial successful results correctly.
    pub fn retry_multi_region_preserve_results(
        self,
        backoff: Backoff,
    ) -> PlanBuilder<PdC, RetryableMultiRegion<P, PdC>, Targetted> {
        self.make_retry_multi_region(backoff, true)
    }

    fn make_retry_multi_region(
        self,
        backoff: Backoff,
        preserve_region_results: bool,
    ) -> PlanBuilder<PdC, RetryableMultiRegion<P, PdC>, Targetted> {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            plan: RetryableMultiRegion {
                inner: self.plan,
                pd_client: self.pd_client,
                backoff,
                preserve_region_results,
            },
            phantom: PhantomData,
        }
    }
}

impl<PdC: PdClient, R: KvRequest> PlanBuilder<PdC, Dispatch<R>, NoTarget> {
    /// Target the request at a single region; caller supplies the store to target.
    pub async fn single_region_with_store(
        self,
        store: RegionStore,
    ) -> Result<PlanBuilder<PdC, Dispatch<R>, Targetted>> {
        set_single_region_store(self.plan, store, self.pd_client)
    }
}

impl<PdC: PdClient, P: Plan + StoreRequest> PlanBuilder<PdC, P, NoTarget>
where
    P::Result: HasKeyErrors + HasRegionError,
{
    pub fn all_stores(
        self,
        backoff: Backoff,
    ) -> PlanBuilder<PdC, RetryableAllStores<P, PdC>, Targetted> {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            plan: RetryableAllStores {
                inner: self.plan,
                pd_client: self.pd_client,
                backoff,
            },
            phantom: PhantomData,
        }
    }
}

impl<PdC: PdClient, P: Plan + Shardable> PlanBuilder<PdC, P, NoTarget>
where
    P::Result: HasKeyErrors,
{
    pub fn preserve_shard(self) -> PlanBuilder<PdC, PreserveShard<P>, NoTarget> {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            plan: PreserveShard {
                inner: self.plan,
                shard: None,
            },
            phantom: PhantomData,
        }
    }
}

impl<PdC: PdClient, P: Plan> PlanBuilder<PdC, P, Targetted>
where
    P::Result: HasKeyErrors + HasRegionErrors,
{
    pub fn extract_error(self) -> PlanBuilder<PdC, ExtractError<P>, Targetted> {
        PlanBuilder {
            pd_client: self.pd_client,
            plan: ExtractError { inner: self.plan },
            phantom: self.phantom,
        }
    }
}

fn set_single_region_store<PdC: PdClient, R: KvRequest>(
    mut plan: Dispatch<R>,
    store: RegionStore,
    pd_client: Arc<PdC>,
) -> Result<PlanBuilder<PdC, Dispatch<R>, Targetted>> {
    plan.request.set_leader(&store.region_with_leader)?;
    plan.kv_client = Some(store.client);
    Ok(PlanBuilder {
        plan,
        pd_client,
        phantom: PhantomData,
    })
}

/// Indicates that a request operates on a single key.
pub trait SingleKey {
    #[allow(clippy::ptr_arg)]
    fn key(&self) -> &Vec<u8>;
}
