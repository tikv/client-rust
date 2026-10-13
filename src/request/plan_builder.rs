// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;
use std::sync::Arc;

use super::plan::PreserveShard;
use super::Keyspace;
use crate::backoff::Backoff;
use crate::config::RequestOrigin;
use crate::config::DEFAULT_TXN_PROTOCOL_VERSION;
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
use crate::Timestamp;

/// Builder type for plans (see that module for more).
pub struct PlanBuilder<PdC: PdClient, P: Plan, Ph: PlanBuilderPhase> {
    pd_client: Arc<PdC>,
    context: PlanContext,
    plan: P,
    phantom: PhantomData<Ph>,
}

/// Immutable client state required to prepare and execute a request plan.
///
/// Transaction protocol metadata is ignored for requests classified as raw.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlanContext {
    keyspace: Keyspace,
    request_origin: RequestOrigin,
    default_txn_protocol_version: u32,
}

impl PlanContext {
    pub(crate) fn new(keyspace: Keyspace) -> Self {
        Self {
            keyspace,
            request_origin: RequestOrigin::Unknown,
            default_txn_protocol_version: DEFAULT_TXN_PROTOCOL_VERSION,
        }
    }

    #[must_use]
    pub(crate) fn with_keyspace(mut self, keyspace: Keyspace) -> Self {
        self.keyspace = keyspace;
        self
    }

    /// Set the process origin attached to transaction RPCs.
    #[must_use]
    pub(crate) fn with_request_origin(mut self, request_origin: RequestOrigin) -> Self {
        self.request_origin = request_origin;
        self
    }

    /// Set the highest transaction protocol version this client can understand.
    #[must_use]
    pub(crate) fn with_default_txn_protocol_version(mut self, version: u32) -> Self {
        self.default_txn_protocol_version = version;
        self
    }

    pub(crate) fn keyspace(&self) -> Keyspace {
        self.keyspace
    }

    pub(crate) fn default_txn_protocol_version(&self) -> u32 {
        self.default_txn_protocol_version
    }

    pub(crate) fn request_origin(&self) -> RequestOrigin {
        self.request_origin
    }
}

/// Used to ensure that a plan has a designated target or targets, a target is
/// a particular TiKV server.
pub trait PlanBuilderPhase {}
pub struct NoTarget;
impl PlanBuilderPhase for NoTarget {}
pub struct Targetted;
impl PlanBuilderPhase for Targetted {}

impl<PdC: PdClient, Req: KvRequest> PlanBuilder<PdC, Dispatch<Req>, NoTarget> {
    pub fn new(pd_client: Arc<PdC>, keyspace: Keyspace, request: Req) -> Self {
        Self::new_with_context(pd_client, request, PlanContext::new(keyspace))
    }

    pub(crate) fn new_with_context(
        pd_client: Arc<PdC>,
        mut request: Req,
        context: PlanContext,
    ) -> Self {
        request.set_api_version(context.keyspace.api_version());
        PlanBuilder {
            pd_client: pd_client.clone(),
            context,
            plan: Dispatch {
                request,
                kv_client: None,
                context,
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
        timestamp: Timestamp,
        backoff: Backoff,
    ) -> PlanBuilder<PdC, ResolveLock<P, PdC>, Ph>
    where
        P::Result: HasLocks + HasRegionError,
    {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            context: self.context,
            plan: ResolveLock {
                inner: self.plan,
                timestamp,
                backoff,
                pd_client: self.pd_client,
                context: self.context,
            },
            phantom: PhantomData,
        }
    }

    pub fn cleanup_locks(
        self,
        ctx: ResolveLocksContext,
        options: ResolveLocksOptions,
        backoff: Backoff,
    ) -> PlanBuilder<PdC, CleanupLocks<P, PdC>, Ph>
    where
        P: Shardable + NextBatch,
        P::Result: HasLocks + HasNextBatch + HasRegionError + HasKeyErrors,
    {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            context: self.context,
            plan: CleanupLocks {
                inner: self.plan,
                ctx,
                options,
                store: None,
                backoff,
                pd_client: self.pd_client,
                context: self.context,
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
            context: self.context,
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
            context: self.context,
            plan: ProcessResponse {
                inner: self.plan,
                processor: DefaultProcessor,
            },
            phantom: PhantomData,
        }
    }
}

/// Named flags for [`PlanBuilder::make_retry_multi_region`]: passed positionally,
/// three same-typed bools would let a transposed call site compile silently.
#[derive(Default)]
struct RetryFlags {
    preserve_region_results: bool,
    terminal_on_undetermined: bool,
    terminal_on_dispatch_error: bool,
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
        self.make_retry_multi_region(backoff, RetryFlags::default())
    }

    /// Preserve all results, even some of them are Err.
    /// To pass all responses to merge, and handle partial successful results correctly.
    pub fn retry_multi_region_preserve_results(
        self,
        backoff: Backoff,
    ) -> PlanBuilder<PdC, RetryableMultiRegion<P, PdC>, Targetted> {
        self.make_retry_multi_region(
            backoff,
            RetryFlags {
                preserve_region_results: true,
                ..Default::default()
            },
        )
    }

    /// Like [`Self::retry_multi_region`], but an `errorpb.UndeterminedResult` is
    /// terminal on first sight. For commit points (primary commit; async/1PC
    /// prewrite) — see `RetryableMultiRegion::terminal_on_undetermined`.
    pub fn retry_multi_region_terminal_on_undetermined(
        self,
        backoff: Backoff,
    ) -> PlanBuilder<PdC, RetryableMultiRegion<P, PdC>, Targetted> {
        self.make_retry_multi_region(
            backoff,
            RetryFlags {
                terminal_on_undetermined: true,
                ..Default::default()
            },
        )
    }

    /// Like [`Self::retry_multi_region_terminal_on_undetermined`], and additionally
    /// a DISPATCH-stage gRPC error is terminal. For the request whose replay is not
    /// idempotent with respect to its own result (raw CAS): a lost response is as
    /// ambiguous as an undetermined apply outcome, and a replay could contradict the
    /// first attempt's own effect. Sharding/connection errors still retry — see
    /// `RetryableMultiRegion::terminal_on_dispatch_error`.
    pub fn retry_multi_region_terminal_on_ambiguous_outcome(
        self,
        backoff: Backoff,
    ) -> PlanBuilder<PdC, RetryableMultiRegion<P, PdC>, Targetted> {
        self.make_retry_multi_region(
            backoff,
            RetryFlags {
                terminal_on_undetermined: true,
                terminal_on_dispatch_error: true,
                ..Default::default()
            },
        )
    }

    fn make_retry_multi_region(
        self,
        backoff: Backoff,
        flags: RetryFlags,
    ) -> PlanBuilder<PdC, RetryableMultiRegion<P, PdC>, Targetted> {
        PlanBuilder {
            pd_client: self.pd_client.clone(),
            context: self.context,
            plan: RetryableMultiRegion {
                inner: self.plan,
                pd_client: self.pd_client,
                context: self.context,
                backoff,
                preserve_region_results: flags.preserve_region_results,
                terminal_on_undetermined: flags.terminal_on_undetermined,
                terminal_on_dispatch_error: flags.terminal_on_dispatch_error,
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
        set_single_region_store(self.plan, store, self.pd_client, self.context)
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
            context: self.context,
            plan: RetryableAllStores {
                inner: self.plan,
                pd_client: self.pd_client,
                context: self.context,
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
            context: self.context,
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
            context: self.context,
            plan: ExtractError { inner: self.plan },
            phantom: self.phantom,
        }
    }
}

fn set_single_region_store<PdC: PdClient, R: KvRequest>(
    mut plan: Dispatch<R>,
    store: RegionStore,
    pd_client: Arc<PdC>,
    context: PlanContext,
) -> Result<PlanBuilder<PdC, Dispatch<R>, Targetted>> {
    plan.request.prepare_txn_rpc(
        store.txn_protocol_version_range,
        context.default_txn_protocol_version(),
        context.request_origin().as_proto(),
    )?;
    plan.request.set_leader(&store.region_with_leader)?;
    plan.kv_client = Some(store.client);
    Ok(PlanBuilder {
        plan,
        pd_client,
        context,
        phantom: PhantomData,
    })
}

/// Indicates that a request operates on a single key.
pub trait SingleKey {
    #[allow(clippy::ptr_arg)]
    fn key(&self) -> &Vec<u8>;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::config::RequestOrigin;
    use crate::mock::{MockKvClient, MockPdClient};
    use crate::proto::kvrpcpb;
    use crate::store::TxnProtocolVersionRange;

    #[tokio::test]
    async fn context_metadata_is_applied_to_single_region_transaction_requests() {
        let context = PlanContext::new(Keyspace::Disable)
            .with_request_origin(RequestOrigin::TiFlash)
            .with_default_txn_protocol_version(1);
        let request = kvrpcpb::GetRequest {
            context: Some(kvrpcpb::Context::default()),
            ..Default::default()
        };
        let store = RegionStore::with_metadata(
            MockPdClient::region1(),
            1,
            TxnProtocolVersionRange { min: 0, max: 2 },
            Arc::new(MockKvClient::default()),
        );

        let builder = PlanBuilder::new_with_context(
            Arc::new(MockPdClient::new(MockKvClient::default())),
            request,
            context,
        )
        .single_region_with_store(store)
        .await
        .unwrap();
        let request_context = builder.plan.request.context.as_ref().unwrap();
        assert_eq!(request_context.txn_protocol_version, 1);
        assert_eq!(
            request_context.request_origin,
            kvrpcpb::RequestOrigin::TiFlash as i32
        );
    }
}
