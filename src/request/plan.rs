// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::prelude::*;
use log::debug;
use log::error;
use log::info;
use log::warn;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::sleep;
use tonic::Code;

use crate::backoff::Backoff;
use crate::common::ErrorPriority;
use crate::pd::PdClient;
use crate::proto::errorpb;
use crate::proto::errorpb::EpochNotMatch;
use crate::proto::kvrpcpb;
use crate::proto::metapb;
use crate::proto::pdpb::Timestamp;
use crate::region::StoreId;
use crate::region::{RegionVerId, RegionWithLeader};
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

/// Declares origin and selects a compatible version per destination.
/// Wraps [`Dispatch`] inside lock and routing retries. Missing store ranges mean
/// `[0, 0]`; a potentially stale maximum permits one metadata reload.
///
/// A valid upper-bound rejection permits one immediate resend to the same store
/// using its returned range, without updating the cache. Routing retries and
/// re-sharding preserve spent allowances; advancing to a new scan page renews it.
/// Undetermined outcomes take precedence; other compatibility rejections are terminal.
pub struct TxnProtocolCompatibility<Req: KvRequest, PdC: PdClient> {
    pub(super) inner: Dispatch<Req>,
    pub(super) pd_client: Arc<PdC>,
    pub(super) policy: crate::TxnRequestPolicy,
    pub(super) store: Option<metapb::Store>,
    pub(super) resend_used: Arc<AtomicBool>,
}

impl<Req: KvRequest, PdC: PdClient> Clone for TxnProtocolCompatibility<Req, PdC> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            policy: self.policy,
            store: self.store.clone(),
            // Execution clones (e.g. cleanup pagination) share their shard's allowance.
            resend_used: self.resend_used.clone(),
        }
    }
}

impl<Req: KvRequest, PdC: PdClient> TxnProtocolCompatibility<Req, PdC> {
    pub(super) fn apply_region_store(&mut self, store: &RegionStore) -> Result<()> {
        self.inner.request.set_leader(&store.region_with_leader)?;
        self.inner.kv_client = Some(store.client.clone());
        self.store = Some(store.meta.clone());
        Ok(())
    }

    fn required_version(&self) -> u32 {
        let request = self.inner.request.as_any();
        let shared = request
            .downcast_ref::<kvrpcpb::PrewriteRequest>()
            .is_some_and(|r| {
                r.mutations
                    .iter()
                    .any(|m| m.op == kvrpcpb::Op::SharedLock as i32)
            })
            || request
                .downcast_ref::<kvrpcpb::PessimisticLockRequest>()
                .is_some_and(|r| {
                    r.mutations
                        .iter()
                        .any(|m| m.op == kvrpcpb::Op::SharedPessimisticLock as i32)
                });
        if shared {
            kvrpcpb::TxnProtocolVersion::TxnVerSupportSharedLock as u32
        } else {
            kvrpcpb::TxnProtocolVersion::TxnVerLegacy as u32
        }
    }

    fn select_version(&self, range: &metapb::TxnProtocolVersionRange) -> Option<u32> {
        let selected = (self.policy.protocol_version as u32).min(range.max);
        (range.min <= range.max && selected >= range.min && selected >= self.required_version())
            .then_some(selected)
    }
}

#[async_trait]
impl<Req: KvRequest, PdC: PdClient> Plan for TxnProtocolCompatibility<Req, PdC> {
    type Result = Req::Response;

    async fn execute(&self) -> Result<Self::Result> {
        let mut store = self.store.clone().ok_or_else(|| {
            Error::StringError("transaction protocol plan has no destination store".to_owned())
        })?;
        let mut range = store.txn_protocol_version_range.clone().unwrap_or_default();
        let mut selected = self.select_version(&range);
        if selected.is_none()
            && range.min <= range.max
            && self.required_version() > range.max
            && self.required_version() <= self.policy.protocol_version as u32
        {
            // Refresh once when only a stale maximum could block this payload.
            // A failed refresh keeps the cached range and local rejection semantics.
            match self.pd_client.reload_store(store.id).await {
                Ok(fresh) => {
                    store = fresh;
                    range = store.txn_protocol_version_range.clone().unwrap_or_default();
                    selected = self.select_version(&range);
                }
                Err(error) => debug!("store protocol refresh failed: {:?}", error),
            }
        }
        let mut version = selected.ok_or_else(|| {
            crate::stats::txn_protocol_rejection("local");
            Error::IncompatibleRequest(Box::new(errorpb::IncompatibleRequest {
                reason: errorpb::IncompatibleRequestReason::Unknown as i32,
                message: format!(
                    "store {} range [{}, {}], configured version {}, payload requires {}",
                    store.id,
                    range.min,
                    range.max,
                    self.policy.protocol_version as u32,
                    self.required_version(),
                ),
                provided_txn_protocol_version: (self.policy.protocol_version as u32).min(range.max),
                min_compatible_txn_protocol_version: range.min,
                max_compatible_txn_protocol_version: range.max,
            }))
        })?;
        let mut dispatch = self.inner.clone();
        loop {
            let context = dispatch.request.context_mut();
            context.request_origin = self.policy.origin as i32;
            context.txn_protocol_version = version;
            let mut response = dispatch.execute().await?;
            let Some(error) = response.region_error_ref() else {
                return Ok(response);
            };
            // Leave unknown apply outcomes to the retry plan's terminal policy.
            if error.undetermined_result.is_some() || error.incompatible_request.is_none() {
                return Ok(response);
            }
            let error = response
                .region_error()
                .expect("region error was just inspected");
            let rejection = error
                .incompatible_request
                .expect("incompatible request was just inspected");
            crate::stats::txn_protocol_rejection("server");
            let returned_range = metapb::TxnProtocolVersionRange {
                min: rejection.min_compatible_txn_protocol_version,
                max: rejection.max_compatible_txn_protocol_version,
            };
            let replacement = self.select_version(&returned_range);
            if rejection.reason
                != errorpb::IncompatibleRequestReason::TxnProtocolVersionOutOfRange as i32
                || returned_range.min > returned_range.max
                || rejection.provided_txn_protocol_version != version
                || version <= returned_range.max
                || replacement.is_none()
                || self.resend_used.swap(true, Ordering::Relaxed)
            {
                return Err(Error::IncompatibleRequest(Box::new(rejection)));
            }
            // Only this resend uses the returned range; never publish it to the cache.
            crate::stats::txn_protocol_resend();
            version = replacement.unwrap();
        }
    }
}

pub(crate) fn is_grpc_error(e: &Error) -> bool {
    matches!(e, Error::GrpcAPI(_) | Error::Grpc(_))
}

/// Await every task in `join_set`, reassembling the results in spawn order.
///
/// Contract: on any join failure (a panicked or cancelled task) the remaining tasks
/// are aborted via [`JoinSet::shutdown`] before the error is returned — an error from
/// the surrounding handler therefore means *no further effects from this call*. The
/// previous `try_join_all`-over-`JoinHandle`s code instead **detached** in-flight
/// tasks on early return, which let them race on after the caller had already
/// observed the failure and panicked the runtime's timer driver when a short-lived
/// runtime shut down underneath them (#534).
async fn collect_join_set_results<T>(
    mut join_set: JoinSet<(usize, T)>,
    task_count: usize,
    handler_name: &str,
) -> Result<Vec<T>>
where
    T: Send + 'static,
{
    let mut results = (0..task_count).map(|_| None).collect::<Vec<_>>();
    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok((idx, val)) => results[idx] = Some(val),
            Err(e) => {
                error!(
                    "{}: failed to join task ({} tasks): {}",
                    handler_name, task_count, e
                );
                join_set.shutdown().await;
                return Err(Error::JoinError(e));
            }
        }
    }

    Ok(results
        .into_iter()
        .map(|result| result.expect("all spawned tasks should return a result"))
        .collect())
}

/// Did the server say the request's outcome is UNKNOWN — e.g. a raft timeout where the
/// apply result was never observed (`errorpb.UndeterminedResult`)? A commit receiving
/// this must be reported as undetermined: reporting plain failure would invite the
/// caller to retry effects that may already be durable.
pub(crate) fn is_undetermined_region_error(e: &Error) -> bool {
    matches!(e, Error::RegionError(re) if re.undetermined_result.is_some())
}

/// Did this error come from the response body, rather than from a failure to obtain a
/// response at all?
///
/// These are the two error types the `HasKeyErrors` response impls construct — see
/// `has_key_error!` / `extract_errors` (a `kvrpcpb::KeyError` field) and `has_str_error!`
/// (a raw endpoint's string `error` field). Everything else reaching `key_errors()` got
/// there through the catch-all arm of `HasKeyErrors for Result<T, Error>`, which reports
/// any per-shard `Err` — a transport failure, an exhausted retry — and must not be
/// relabelled as something the server said about a key.
fn is_response_error(e: &Error) -> bool {
    matches!(e, Error::KeyError(_) | Error::KvError { .. })
}

pub struct RetryableMultiRegion<P: Plan, PdC: PdClient> {
    pub(super) inner: P,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,

    /// Preserve all regions' results for other downstream plans to handle.
    /// If true, return Ok and preserve all regions' results, even if some of them are Err.
    /// Otherwise, return the first Err if there is any.
    pub preserve_region_results: bool,

    /// Terminal treatment of `errorpb.UndeterminedResult` (an unknown raft apply
    /// outcome). client-go's transport never retries it and each ACTION decides
    /// (region_request.go: "should not retry ... processed by the caller"); its one
    /// terminal action is the primary non-async commit (commit.go: returns
    /// ErrResultUndetermined). Plans opt in when a replay could produce a WRONG
    /// RESULT (raw CAS replaying against its own effect) or when the request is a
    /// commit point and a later, different retry error must not overwrite the
    /// uncertainty (primary commit; async/1PC prewrite — stricter than client-go,
    /// in the safe direction). Everything else retries: re-applying an idempotent
    /// request resolves the uncertainty, and on backoff exhaustion the error
    /// escapes UNCHANGED for the caller to classify.
    pub terminal_on_undetermined: bool,

    /// Terminal treatment of a DISPATCH-stage gRPC error, for the request whose
    /// replay is not idempotent with respect to its own result: raw CAS. Once the
    /// request may have been sent, a lost response is as ambiguous as
    /// `errorpb.UndeterminedResult` — a replay would compare against the first
    /// attempt's own write and could report `succeed = false` for a write that
    /// happened. Sharding/connection errors still retry (the request was never
    /// sent), and commit points do NOT set this: replaying a commit is idempotent —
    /// it can only resolve the uncertainty — and client-go's transport likewise
    /// retries RPC errors there (region_request.go, onSendFail).
    pub terminal_on_dispatch_error: bool,
}

struct CandidateResponse<R> {
    response: R,
    key_errors: Option<Vec<Error>>,
    region_error: Option<errorpb::Error>,
    region_store: RegionStore,
    used_fallback: bool,
}

impl<R> CandidateResponse<R>
where
    R: HasKeyErrors + HasRegionError,
{
    fn new(mut response: R, region_store: RegionStore, used_fallback: bool) -> Self {
        let key_errors = response.key_errors();
        let region_error = response.region_error();
        Self {
            response,
            key_errors,
            region_error,
            region_store,
            used_fallback,
        }
    }

    fn is_fallback_not_leader(&self) -> bool {
        self.used_fallback
            && self.key_errors.is_none()
            && self.region_error.as_ref().is_some_and(|error| {
                error.not_leader.is_some() && error.undetermined_result.is_none()
            })
    }

    fn is_fallback_server_busy(&self) -> bool {
        self.used_fallback
            && self.key_errors.is_none()
            && self.region_error.as_ref().is_some_and(|error| {
                error.server_is_busy.is_some() && error.undetermined_result.is_none()
            })
    }

    fn accepted_fallback_leader(&self) -> Option<metapb::Peer> {
        if !self.used_fallback || self.region_error.is_some() {
            return None;
        }
        self.region_store.region_with_leader.leader.clone()
    }
}

enum CandidateRoundResult<R> {
    Response(Box<CandidateResponse<R>>),
    MapRegionToStoreError(Error),
    RoutingError {
        error: Error,
        invalidate_region: bool,
    },
    OtherError(Error),
}

#[derive(Default)]
struct CandidateState {
    busy_leader_peer_id: Option<u64>,
    busy_count: u8,
    follower_probe_done: bool,
}

impl CandidateState {
    const LEADER_BUSY_PROBE_THRESHOLD: u8 = 2;

    fn record_leader_busy(&mut self, peer_id: u64, estimated_wait_ms: u32) {
        if estimated_wait_ms != 0 || self.follower_probe_done {
            return;
        }
        if self.busy_leader_peer_id != Some(peer_id) {
            self.busy_leader_peer_id = Some(peer_id);
            self.busy_count = 0;
        }
        self.busy_count = self.busy_count.saturating_add(1);
    }

    fn take_follower_probe(&mut self) -> bool {
        if !self.follower_probe_done && self.busy_count >= Self::LEADER_BUSY_PROBE_THRESHOLD {
            self.follower_probe_done = true;
            true
        } else {
            false
        }
    }
}

struct RetryContext<PdC> {
    pd_client: Arc<PdC>,
    permits: Arc<Semaphore>,
    preserve_region_results: bool,
    terminal_on_undetermined: bool,
    terminal_on_dispatch_error: bool,
}

impl<PdC> Clone for RetryContext<PdC> {
    fn clone(&self) -> Self {
        Self {
            pd_client: self.pd_client.clone(),
            permits: self.permits.clone(),
            preserve_region_results: self.preserve_region_results,
            terminal_on_undetermined: self.terminal_on_undetermined,
            terminal_on_dispatch_error: self.terminal_on_dispatch_error,
        }
    }
}

enum GrpcErrorAction {
    TryNextPeer,
    TryNextPeerAndInvalidate,
    Return,
}

fn grpc_error_action(error: &Error) -> GrpcErrorAction {
    match error {
        // A transport setup error means this Store cannot currently be used.
        Error::Grpc(_) => GrpcErrorAction::TryNextPeerAndInvalidate,
        Error::GrpcAPI(status) if std::error::Error::source(status).is_some() => {
            // Tonic maps lower-level I/O and HTTP/2 failures to Status, and
            // keeps the transport error as its source. Its code is not always
            // Unavailable (for example, an HTTP/2 failure can be Internal or
            // ResourceExhausted), so inspect the source before the code.
            GrpcErrorAction::TryNextPeerAndInvalidate
        }
        Error::GrpcAPI(status) => match status.code() {
            // client-go retries a remote/keepalive cancellation after replacing
            // the connection. Dropping a Rust request future cancels this whole
            // task, so a Canceled status observed here came from the RPC side.
            // Tonic can synthesize source-less Internal/Unknown statuses when
            // an HTTP/2 stream ends before a complete response is decoded
            // (for example "Missing response message"). Those are
            // indistinguishable here from an application status, but treating
            // them as terminal would miss the cold-region recovery path when
            // the cached leader disconnects mid-response.
            Code::Unavailable | Code::Cancelled | Code::Internal | Code::Unknown => {
                GrpcErrorAction::TryNextPeerAndInvalidate
            }
            // A request deadline does not prove that the Store or Region route
            // is stale. Try another replica, but preserve the shared caches.
            Code::DeadlineExceeded => GrpcErrorAction::TryNextPeer,
            // Application-level statuses (invalid arguments, auth failures,
            // unsupported APIs, explicit server resource limits, etc.) cannot
            // be healed by replaying the request on every replica. client-go
            // uses a separate Store health RPC to distinguish this case; Rust
            // has no equivalent liveness subsystem, so an explicit status with
            // no transport source is the strongest available signal.
            _ => GrpcErrorAction::Return,
        },
        _ => GrpcErrorAction::Return,
    }
}

fn region_request_candidates(
    region: &RegionWithLeader,
    followers_only: bool,
) -> impl Iterator<Item = &metapb::Peer> + '_ {
    let leader_store_id = region.leader.as_ref().map(|leader| leader.store_id);
    // A request to an unreachable cached leader cannot wake a cold Raft
    // group. Lazily try each data-bearing voter after the cached leader
    // before invalidating it and going back to PD. Learners and witnesses
    // cannot campaign and are not useful for cold-region recovery.
    region
        .leader
        .iter()
        .filter(move |_| !followers_only)
        .chain(region.region.peers.iter().filter(move |peer| {
            !peer.is_witness
                && peer.role != metapb::PeerRole::Learner as i32
                && leader_store_id != Some(peer.store_id)
        }))
}

impl<P: Plan + Shardable, PdC: PdClient> RetryableMultiRegion<P, PdC>
where
    P::Result: HasKeyErrors + HasRegionError,
{
    // A plan may involve multiple shards
    #[async_recursion]
    async fn single_plan_handler(
        context: RetryContext<PdC>,
        current_plan: P,
        backoff: Backoff,
    ) -> Result<<Self as Plan>::Result> {
        let shards = current_plan
            .shards(&context.pd_client)
            .collect::<Vec<_>>()
            .await;
        let shards_len = shards.len();
        debug!("single_plan_handler, shards: {}", shards_len);
        let mut join_set = JoinSet::new();
        for (idx, shard) in shards.into_iter().enumerate() {
            let (shard, region) = match shard {
                Ok(shard) => shard,
                Err(e) => {
                    join_set.shutdown().await;
                    return Err(e);
                }
            };
            let clone = current_plan.clone_then_apply_shard(shard);
            let shard_context = context.clone();
            let backoff = backoff.clone();
            join_set.spawn(async move {
                (
                    idx,
                    Self::single_shard_handler(
                        shard_context,
                        clone,
                        region,
                        backoff,
                        CandidateState::default(),
                    )
                    .await,
                )
            });
        }

        let results = collect_join_set_results(join_set, shards_len, "single_plan_handler").await?;

        if context.preserve_region_results {
            Ok(results
                .into_iter()
                .flat_map_ok(|x| x)
                .map(|x| match x {
                    Ok(r) => r,
                    Err(e) => Err(e),
                })
                .collect())
        } else {
            // Unknown apply outcomes and compatibility rejections must survive
            // ordinary failures from other shards. Equal priorities retain the
            // first error by shard index.
            let mut oks = Vec::with_capacity(results.len());
            let mut selected_error: Option<Error> = None;
            for r in results {
                match r {
                    Ok(v) => oks.push(v),
                    Err(e)
                        if selected_error
                            .as_ref()
                            .is_none_or(|selected| e.priority() < selected.priority()) =>
                    {
                        selected_error = Some(e)
                    }
                    Err(_) => {}
                }
            }
            if let Some(e) = selected_error {
                return Err(e);
            }
            Ok(oks.into_iter().flatten().collect())
        }
    }

    async fn execute_candidate(
        pd_client: &Arc<PdC>,
        plan: &mut P,
        region: &RegionWithLeader,
        peer: &metapb::Peer,
        original_leader_store_id: Option<StoreId>,
        permits: &Semaphore,
        terminal_on_dispatch_error: bool,
    ) -> CandidateRoundResult<P::Result> {
        let is_fallback = original_leader_store_id != Some(peer.store_id);
        let mut candidate_region = region.clone();
        candidate_region.leader = Some(peer.clone());

        let region_store = match pd_client
            .clone()
            .map_region_to_store(candidate_region)
            .await
        {
            Ok(region_store) => region_store,
            Err(err) => {
                debug!(
                    "single_shard_handler::map_store, fallback: {}, error: {:?}",
                    is_fallback, err
                );
                // Mapping includes opening the TiKV channel. Drop the Store
                // entry so a later attempt can reload a changed address from
                // PD instead of reconnecting to stale metadata indefinitely.
                pd_client.invalidate_store_cache(peer.store_id).await;
                return CandidateRoundResult::MapRegionToStoreError(err);
            }
        };
        if let Err(error) = plan.apply_store(&region_store) {
            // Applying a successfully mapped Store mutates the request. This is
            // not a PD/connection lookup failure and trying another peer cannot
            // repair an invalid request.
            return CandidateRoundResult::OtherError(error);
        }

        // Fallback attempts are sequential inside one shard, so at most one
        // concurrency permit is held at a time.
        let permit = permits.acquire().await.unwrap();
        let result = plan.execute().await;
        drop(permit);

        match result {
            Ok(response) => CandidateRoundResult::Response(Box::new(CandidateResponse::new(
                response,
                region_store,
                is_fallback,
            ))),
            Err(error) if is_grpc_error(&error) => {
                debug!(
                    "single_shard_handler:execute: grpc error, fallback: {}, error: {:?}",
                    is_fallback, error
                );
                // A non-idempotent request (raw CAS) may have reached the
                // server and must not be replayed, including on a follower.
                if terminal_on_dispatch_error {
                    pd_client.invalidate_region_cache(region.ver_id()).await;
                    pd_client.invalidate_store_cache(peer.store_id).await;
                    return CandidateRoundResult::OtherError(error);
                }

                match grpc_error_action(&error) {
                    GrpcErrorAction::TryNextPeerAndInvalidate => {
                        pd_client.invalidate_store_cache(peer.store_id).await;
                        CandidateRoundResult::RoutingError {
                            error,
                            invalidate_region: true,
                        }
                    }
                    GrpcErrorAction::TryNextPeer => CandidateRoundResult::RoutingError {
                        error,
                        invalidate_region: false,
                    },
                    GrpcErrorAction::Return => CandidateRoundResult::OtherError(error),
                }
            }
            Err(error) => {
                debug!("single_shard_handler:execute: error: {:?}", error);
                CandidateRoundResult::OtherError(error)
            }
        }
    }

    async fn execute_candidate_round(
        pd_client: &Arc<PdC>,
        plan: &mut P,
        region: &RegionWithLeader,
        permits: &Semaphore,
        terminal_on_dispatch_error: bool,
        followers_only: bool,
    ) -> CandidateRoundResult<P::Result> {
        let original_leader_store_id = region.leader.as_ref().map(|peer| peer.store_id);
        let mut last_map_region_to_store_error = None;
        let mut last_routing_error = None;
        let mut invalidate_region = false;
        let mut saw_fallback_not_leader = false;
        let mut last_server_busy_response = None;

        for peer in region_request_candidates(region, followers_only) {
            match Self::execute_candidate(
                pd_client,
                plan,
                region,
                peer,
                original_leader_store_id,
                permits,
                terminal_on_dispatch_error,
            )
            .await
            {
                CandidateRoundResult::MapRegionToStoreError(error) => {
                    // Mapping includes Store lookup and opening a TiKV channel.
                    // One unavailable candidate must not hide a later healthy
                    // voter, especially while waking a cold Region.
                    last_map_region_to_store_error = Some(error);
                }
                CandidateRoundResult::RoutingError {
                    error,
                    invalidate_region: candidate_invalidates_region,
                } => {
                    // Preserve the fact that at least one mapped candidate had
                    // a transport routing failure. A later mapping failure must
                    // not hide the need to reload an exhausted Region route.
                    invalidate_region |= candidate_invalidates_region;
                    last_routing_error = Some(error);
                }
                CandidateRoundResult::Response(response) if response.is_fallback_not_leader() => {
                    // A follower hint is unnecessary for the three-replica
                    // cold-region path: keep walking the Region's voters. A
                    // real new leader will accept its normal (non-replica-read)
                    // request later in this same round.
                    saw_fallback_not_leader = true;
                }
                CandidateRoundResult::Response(response) if response.is_fallback_server_busy() => {
                    // A follower probe can itself be rejected at the read-pool
                    // entrance. Keep trying other voters before restoring the
                    // cached leader.
                    last_server_busy_response = Some(response);
                }
                CandidateRoundResult::OtherError(error) => {
                    return CandidateRoundResult::OtherError(error);
                }
                response @ CandidateRoundResult::Response(_) => return response,
            }
        }

        if invalidate_region {
            // A transport failure means the cached Region route may be stale.
            // Do not let a later fallback ServerIsBusy response preserve that
            // route: the follower may have rejected the request before
            // validating Region membership.
            if let Some(error) = last_routing_error.take() {
                return CandidateRoundResult::RoutingError {
                    error,
                    invalidate_region: true,
                };
            }
        }

        if saw_fallback_not_leader && !followers_only {
            // The cached peer list may be stale even though follower hints are
            // deliberately ignored. Reload it after exhausting every voter.
            return CandidateRoundResult::RoutingError {
                error: Error::LeaderNotFound {
                    region: region.ver_id(),
                },
                invalidate_region: true,
            };
        }

        if let Some(response) = last_server_busy_response {
            CandidateRoundResult::Response(response)
        } else if let Some(error) = last_routing_error {
            CandidateRoundResult::RoutingError {
                error,
                invalidate_region,
            }
        } else if let Some(error) = last_map_region_to_store_error {
            CandidateRoundResult::MapRegionToStoreError(error)
        } else {
            CandidateRoundResult::RoutingError {
                error: Error::LeaderNotFound {
                    region: region.ver_id(),
                },
                invalidate_region: !followers_only,
            }
        }
    }

    async fn update_leader_after_fallback(
        pd_client: &Arc<PdC>,
        region_ver_id: &RegionVerId,
        leader: Option<metapb::Peer>,
    ) {
        // A response without a Region error proves that a normal request was
        // accepted by the fallback peer as leader. Update the cache before
        // returning key-level errors, which are interpreted by higher layers.
        if let Some(peer) = leader {
            if let Err(error) = pd_client.update_leader(region_ver_id.clone(), peer).await {
                debug!(
                    "failed to update leader after follower fallback: {:?}",
                    error
                );
            }
        }
    }

    async fn retry_same_region(
        context: RetryContext<PdC>,
        plan: P,
        region: RegionWithLeader,
        mut backoff: Backoff,
        candidate_state: CandidateState,
        error: Error,
    ) -> Result<<Self as Plan>::Result> {
        match backoff.next_delay_duration() {
            Some(duration) => {
                sleep(duration).await;
                Self::single_shard_handler(context, plan, region, backoff, candidate_state).await
            }
            None => Err(error),
        }
    }

    #[async_recursion]
    async fn single_shard_handler(
        context: RetryContext<PdC>,
        mut plan: P,
        region: RegionWithLeader,
        backoff: Backoff,
        mut candidate_state: CandidateState,
    ) -> Result<<Self as Plan>::Result> {
        let region_ver_id = region.ver_id();
        let store_id = region.get_store_id().ok();
        debug!(
            "single_shard_handler, region: {:?}, store: {:?}",
            region_ver_id, store_id
        );

        let followers_only = candidate_state.take_follower_probe();
        let response = match Self::execute_candidate_round(
            &context.pd_client,
            &mut plan,
            &region,
            &context.permits,
            context.terminal_on_dispatch_error,
            followers_only,
        )
        .await
        {
            CandidateRoundResult::Response(response) => *response,
            CandidateRoundResult::MapRegionToStoreError(error) if followers_only => {
                return Self::retry_same_region(
                    context,
                    plan,
                    region,
                    backoff,
                    candidate_state,
                    error,
                )
                .await;
            }
            CandidateRoundResult::MapRegionToStoreError(error) => {
                // Every candidate failed before dispatch while mapping its Store.
                // The cached peer list may have been replaced completely, so
                // reload the Region instead of retrying the same stale peers.
                return Self::retry_after_routing_error(
                    context,
                    plan,
                    region_ver_id,
                    backoff,
                    error,
                )
                .await;
            }
            CandidateRoundResult::OtherError(error) => return Err(error),
            CandidateRoundResult::RoutingError {
                error: Error::LeaderNotFound { .. },
                ..
            } if followers_only => {
                // Every probed follower answered NotLeader. Ignore all hints
                // and restore the cached leader without charging another
                // backoff: the one-shot probe cannot fire again in this
                // request, so the next leader error follows the normal path.
                return Self::single_shard_handler(context, plan, region, backoff, candidate_state)
                    .await;
            }
            CandidateRoundResult::RoutingError { error, .. } if followers_only => {
                return Self::retry_same_region(
                    context,
                    plan,
                    region,
                    backoff,
                    candidate_state,
                    error,
                )
                .await;
            }
            CandidateRoundResult::RoutingError {
                error,
                invalidate_region: false,
            } => {
                return Self::retry_after_error(context, plan, backoff, error).await;
            }
            CandidateRoundResult::RoutingError {
                error,
                invalidate_region: true,
            } => {
                return Self::retry_after_routing_error(
                    context,
                    plan,
                    region_ver_id,
                    backoff,
                    error,
                )
                .await;
            }
        };

        Self::handle_candidate_response(
            context,
            plan,
            region,
            backoff,
            candidate_state,
            followers_only,
            response,
        )
        .await
    }

    async fn handle_candidate_response(
        context: RetryContext<PdC>,
        plan: P,
        region: RegionWithLeader,
        backoff: Backoff,
        candidate_state: CandidateState,
        followers_only: bool,
        response: CandidateResponse<P::Result>,
    ) -> Result<<Self as Plan>::Result> {
        let region_ver_id = region.ver_id();
        let fallback_leader = response.accepted_fallback_leader();
        Self::update_leader_after_fallback(&context.pd_client, &region_ver_id, fallback_leader)
            .await;
        let CandidateResponse {
            response,
            key_errors,
            region_error,
            region_store,
            used_fallback,
        } = response;

        if region_error
            .as_ref()
            .is_some_and(|error| error.undetermined_result.is_some())
        {
            return Self::handle_region_response(
                context,
                plan,
                region.ver_id(),
                region_store,
                backoff,
                region_error.unwrap(),
            )
            .await;
        }
        if let Some(error) = key_errors {
            debug!("single_shard_handler:execute: key errors: {:?}", error);
            return Ok(vec![Err(Error::MultipleKeyErrors(error))]);
        }
        let Some(error) = region_error else {
            return Ok(vec![Ok(response)]);
        };

        if let Some(server_is_busy) = error
            .server_is_busy
            .as_ref()
            .filter(|_| error.undetermined_result.is_none())
        {
            if !used_fallback && server_is_busy.estimated_wait_ms == 0 {
                let mut candidate_state = candidate_state;
                if let Some(peer) = region_store.region_with_leader.leader.as_ref() {
                    candidate_state.record_leader_busy(peer.id, server_is_busy.estimated_wait_ms);
                }
                return Self::retry_same_region(
                    context,
                    plan,
                    region,
                    backoff,
                    candidate_state,
                    Error::RegionError(Box::new(error)),
                )
                .await;
            }

            if followers_only {
                // The one-shot probe was inconclusive. Restore the cached
                // leader and resume the normal ServerIsBusy backoff path.
                return Self::retry_same_region(
                    context,
                    plan,
                    region,
                    backoff,
                    candidate_state,
                    Error::RegionError(Box::new(error)),
                )
                .await;
            }
        }

        Self::handle_region_response(context, plan, region.ver_id(), region_store, backoff, error)
            .await
    }

    async fn handle_region_response(
        context: RetryContext<PdC>,
        plan: P,
        region_ver_id: RegionVerId,
        region_store: RegionStore,
        mut backoff: Backoff,
        error: errorpb::Error,
    ) -> Result<<Self as Plan>::Result> {
        debug!(
            "single_shard_handler:execute: region error: {:?}, region: {:?}",
            error, region_ver_id
        );
        // For CAS and commit points, an unknown apply outcome must surface on
        // first sight: replaying could contradict its own effect, and a later
        // different error must not overwrite the uncertainty.
        if context.terminal_on_undetermined && error.undetermined_result.is_some() {
            return Err(Error::RegionError(Box::new(error)));
        }

        match backoff.next_delay_duration() {
            Some(duration) => {
                let region_error_resolved =
                    handle_region_error(context.pd_client.clone(), error, region_store).await?;
                if !region_error_resolved {
                    sleep(duration).await;
                }
                Self::single_plan_handler(context, plan, backoff).await
            }
            None => {
                warn!(
                    "giving up after exhausting retries on region error, region: {:?}",
                    region_ver_id
                );
                Err(Error::RegionError(Box::new(error)))
            }
        }
    }

    async fn retry_after_routing_error(
        context: RetryContext<PdC>,
        plan: P,
        region: RegionVerId,
        backoff: Backoff,
        error: Error,
    ) -> Result<<Self as Plan>::Result> {
        debug!("retry_after_routing_error: {:?}", error);
        context.pd_client.invalidate_region_cache(region).await;
        Self::retry_after_error(context, plan, backoff, error).await
    }

    async fn retry_after_error(
        context: RetryContext<PdC>,
        plan: P,
        mut backoff: Backoff,
        error: Error,
    ) -> Result<<Self as Plan>::Result> {
        match backoff.next_delay_duration() {
            Some(duration) => {
                sleep(duration).await;
                Self::single_plan_handler(context, plan, backoff).await
            }
            None => Err(error),
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
    let store_id = region_store.region_with_leader.get_store_id();
    debug!("handling region error: {:?}, region: {:?}", e, ver_id);
    if e.undetermined_result.is_some() {
        // The apply outcome is UNKNOWN (a raft timeout, errorpb.UndeterminedResult).
        // Default: retry — matching client-go's ACTION layers (ordinary prewrites and
        // secondary commits back off and re-send; re-applying an idempotent request
        // resolves the uncertainty). Routing is not suspect, so nothing is
        // invalidated. On backoff exhaustion the error escapes UNCHANGED, and commit
        // paths classify it via `is_undetermined_region_error`. Plans for which a
        // replay is unsafe never reach this arm — see `terminal_on_undetermined`.
        // This outcome takes precedence over other fields in the envelope.
        return Ok(false);
    }
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
        if let Ok(store_id) = store_id {
            pd_client.invalidate_store_cache(store_id).await;
        }
        Ok(false)
    } else if e.epoch_not_match.is_some() {
        on_region_epoch_not_match(pd_client.clone(), region_store, e.epoch_not_match.unwrap()).await
    } else if e.stale_command.is_some() || e.region_not_found.is_some() {
        pd_client.invalidate_region_cache(ver_id).await;
        Ok(false)
    } else if e.server_is_busy.is_some() {
        // ServerIsBusy is a definitive rejection, so retrying is safe. The
        // cached leader remains valid; the caller applies the normal Region
        // backoff and may run the one-shot follower probe for ServerIsBusy(0).
        Ok(false)
    } else if e.raft_entry_too_large.is_some() || e.max_timestamp_not_synced.is_some() {
        Err(Error::RegionError(Box::new(e)))
    } else {
        debug!(
            "unknown region error, invalidating region and store caches, region: {:?}: {:?}",
            ver_id, e
        );
        pd_client.invalidate_region_cache(ver_id).await;
        if let Ok(store_id) = store_id {
            pd_client.invalidate_store_cache(store_id).await;
        }
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
            terminal_on_undetermined: self.terminal_on_undetermined,
            terminal_on_dispatch_error: self.terminal_on_dispatch_error,
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
        let context = RetryContext {
            pd_client: self.pd_client.clone(),
            permits: concurrency_permits,
            preserve_region_results: self.preserve_region_results,
            terminal_on_undetermined: self.terminal_on_undetermined,
            terminal_on_dispatch_error: self.terminal_on_dispatch_error,
        };
        Self::single_plan_handler(context, self.inner.clone(), self.backoff.clone()).await
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
        let stores_len = stores.len();
        let mut join_set = JoinSet::new();
        for (idx, store) in stores.into_iter().enumerate() {
            let mut clone = self.inner.clone();
            clone.apply_store(&store);
            let backoff = self.backoff.clone();
            let concurrency_permits = concurrency_permits.clone();
            join_set.spawn(async move {
                (
                    idx,
                    Self::single_store_handler(clone, backoff, concurrency_permits).await,
                )
            });
        }

        let results =
            collect_join_set_results(join_set, stores_len, "single_store_handler").await?;
        Ok(results)
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
    pub timestamp: Timestamp,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,
    pub keyspace: Keyspace,
    pub request_policy: crate::TxnRequestPolicy,
}

impl<P: Plan, PdC: PdClient> Clone for ResolveLock<P, PdC> {
    fn clone(&self) -> Self {
        ResolveLock {
            inner: self.inner.clone(),
            timestamp: self.timestamp.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
            keyspace: self.keyspace,
            request_policy: self.request_policy,
        }
    }
}

#[async_trait]
impl<P: Plan, PdC: PdClient> Plan for ResolveLock<P, PdC>
where
    P::Result: HasLocks + HasRegionError,
{
    type Result = P::Result;

    async fn execute(&self) -> Result<Self::Result> {
        let mut result = self.inner.execute().await?;
        let mut clone = self.clone();
        loop {
            if result.region_error_ref().is_some() {
                return Ok(result);
            }
            let locks = result.take_locks();
            if locks.is_empty() {
                return Ok(result);
            }

            if self.backoff.is_none() {
                return Err(Error::ResolveLockError(locks));
            }

            let pd_client = self.pd_client.clone();
            let live_locks = resolve_locks(
                locks,
                self.timestamp.clone(),
                pd_client.clone(),
                self.keyspace,
                self.request_policy,
            )
            .await?;
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
    fn region_error_ref(&self) -> Option<&errorpb::Error> {
        self.region_error.as_ref()
    }

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
    pub request_policy: crate::TxnRequestPolicy,
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
            request_policy: self.request_policy,
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
        let mut lock_resolver =
            crate::transaction::LockResolver::new(self.ctx.clone(), self.request_policy);
        let region = &self.store.as_ref().unwrap().region_with_leader;
        let mut has_more_batch = true;

        while has_more_batch {
            let mut scan_lock_resp = inner.execute().await?;

            // Propagate errors to `retry_multi_region` for retry.
            result.region_error = scan_lock_resp.region_error();
            result.key_error = scan_lock_resp.key_errors();
            if result.region_error.is_some() || result.key_error.is_some() {
                info!("CleanupLocks::execute, inner errors:{:?}", result);
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

            // BEFORE any filter: a shared-lock wrapper's fields (including
            // `use_async_commit`) must not be read — filtering on them would silently
            // drop the real member locks. Refuse instead; see `reject_shared_locks`.
            crate::transaction::reject_shared_locks(&locks)?;
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
                // Propagate errors to `retry_multi_region` for retry. The variant now
                // says which kind arrived, so neither arm has to pop an element to find
                // out. That also fixes a real leak in the old single-variant code: it
                // popped the last error to type-test it, and on the key-error path
                // assigned the *remaining* vec to `key_error` — so a lone key error was
                // dropped and `key_errors()` went on to report `Some(vec![])`, an error
                // carrying no errors.
                Err(Error::MultipleRegionErrors(mut errors)) => {
                    if let Some(Error::RegionError(e)) = errors.pop() {
                        result.region_error = Some(*e);
                    }
                    return Ok(result);
                }
                Err(Error::MultipleKeyErrors(errors)) => {
                    result.key_error = Some(errors);
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
        let mut key_errors = result.key_errors();
        let mut region_errors = result
            .region_errors()
            .map(|errors| errors.into_iter().map(Error::from).collect::<Vec<_>>());
        // Preserved shard failures arrive through key_errors(), while bare
        // responses can carry region errors. Inspect both sources before applying
        // the ordinary wrapping rules, so neither can mask a protocol outcome.
        let selected = region_errors
            .iter()
            .flatten()
            .chain(key_errors.iter().flatten())
            .enumerate()
            .filter(|(_, error)| error.priority() != ErrorPriority::Ordinary)
            .min_by_key(|(_, error)| error.priority())
            .map(|(index, _)| index);
        if let Some(index) = selected {
            let region_count = region_errors.as_ref().map_or(0, Vec::len);
            return Err(if index < region_count {
                region_errors.as_mut().unwrap().remove(index)
            } else {
                key_errors.as_mut().unwrap().remove(index - region_count)
            });
        }
        // Each branch names what it found. Previously both produced one
        // `ExtractedErrors`, so a caller had to pop an element and inspect its type to
        // learn which kind it was holding — and whether a caller saw that variant at
        // all depended on whether a collapsing merge sat between the retry layer and
        // this plan (see `MultipleKeyErrors`' docs).
        if let Some(errors) = key_errors {
            // `HasKeyErrors for Result<T, Error>` yields *any* per-shard `Err`, not only
            // errors the response carried — that is how a plan keeping its
            // `Vec<Result<_>>` (see `retry_multi_region_preserve_results`) carries a hard
            // failure through. Wrap only what the response itself reported; a transport
            // or exhausted-retry error keeps its own class instead of being relabelled.
            //
            // "Reported by the response" is exactly the two error types the `HasKeyErrors`
            // response impls construct: `KeyError` from a `kvrpcpb::KeyError` field
            // (`has_key_error!`, `extract_errors`) and `KvError` from a raw endpoint's
            // string `error` field (`has_str_error!`). Both must be treated alike, or
            // plan shape would still decide the variant for raw operations — the very
            // thing this split removes.
            if errors.iter().all(is_response_error) {
                Err(Error::MultipleKeyErrors(errors))
            } else {
                Err(errors
                    .into_iter()
                    .find(|e| !is_response_error(e))
                    .expect("a non-response error exists: the all() test above failed"))
            }
        } else if let Some(errors) = region_errors {
            Err(Error::MultipleRegionErrors(errors))
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
    fn region_error_ref(&self) -> Option<&errorpb::Error> {
        self.0.region_error_ref()
    }

    fn region_error(&mut self) -> Option<errorpb::Error> {
        self.0.region_error()
    }
}

#[cfg(test)]
mod test {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use futures::stream::BoxStream;
    use futures::stream::{self};

    use super::*;
    use crate::mock::MockPdClient;
    use crate::proto::kvrpcpb::BatchGetResponse;

    #[test]
    fn grpc_statuses_are_classified_without_cache_churn_for_deadlines() {
        #[derive(Debug)]
        struct StatusWithSource(tonic::Status);

        impl std::fmt::Display for StatusWithSource {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl std::error::Error for StatusWithSource {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        let unavailable = Error::GrpcAPI(tonic::Status::unavailable("down"));
        assert!(matches!(
            grpc_error_action(&unavailable),
            GrpcErrorAction::TryNextPeerAndInvalidate
        ));

        let deadline = Error::GrpcAPI(tonic::Status::deadline_exceeded("slow"));
        assert!(matches!(
            grpc_error_action(&deadline),
            GrpcErrorAction::TryNextPeer
        ));

        let invalid = Error::GrpcAPI(tonic::Status::invalid_argument("bad request"));
        assert!(matches!(
            grpc_error_action(&invalid),
            GrpcErrorAction::Return
        ));

        let explicit_unknown = Error::GrpcAPI(tonic::Status::unknown("stream closed"));
        assert!(matches!(
            grpc_error_action(&explicit_unknown),
            GrpcErrorAction::TryNextPeerAndInvalidate
        ));

        let source_less_internal =
            Error::GrpcAPI(tonic::Status::internal("Missing response message."));
        assert!(matches!(
            grpc_error_action(&source_less_internal),
            GrpcErrorAction::TryNextPeerAndInvalidate
        ));

        let explicit_resource_limit = Error::GrpcAPI(tonic::Status::resource_exhausted("limit"));
        assert!(matches!(
            grpc_error_action(&explicit_resource_limit),
            GrpcErrorAction::Return
        ));

        let transport_status = tonic::Status::from_error(Box::new(StatusWithSource(
            tonic::Status::resource_exhausted("http2 overload"),
        )));
        assert_eq!(transport_status.code(), Code::ResourceExhausted);
        let transport_error = Error::GrpcAPI(transport_status);
        assert!(matches!(
            grpc_error_action(&transport_error),
            GrpcErrorAction::TryNextPeerAndInvalidate
        ));
    }

    #[test]
    fn server_busy_probe_counts_only_zero_wait_for_the_same_leader() {
        let mut state = CandidateState::default();
        state.record_leader_busy(1, 0);
        state.record_leader_busy(1, 10);
        assert!(!state.take_follower_probe());

        state.record_leader_busy(1, 0);
        assert!(state.take_follower_probe());
        assert!(!state.take_follower_probe(), "the probe is one-shot");

        let mut state = CandidateState::default();
        state.record_leader_busy(1, 0);
        state.record_leader_busy(2, 0);
        assert!(
            !state.take_follower_probe(),
            "a new leader must not inherit the old leader's busy count"
        );
    }

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
        ) -> BoxStream<'static, crate::Result<(Self::Shard, RegionWithLeader)>> {
            Box::pin(stream::iter(1..=3).map(|_| Err(Error::Unimplemented))).boxed()
        }

        fn apply_shard(&mut self, _: Self::Shard) {}

        fn apply_store(&mut self, _: &crate::store::RegionStore) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_err() {
        let plan = RetryableMultiRegion {
            inner: ResolveLock {
                inner: ErrPlan,
                timestamp: Timestamp::default(),
                backoff: Backoff::no_backoff(),
                pd_client: Arc::new(MockPdClient::default()),
                keyspace: Keyspace::Disable,
                request_policy: crate::TxnRequestPolicy::default(),
            },
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_backoff(),
            preserve_region_results: false,
            terminal_on_undetermined: false,
            terminal_on_dispatch_error: false,
        };
        assert!(plan.execute().await.is_err())
    }

    #[tokio::test]
    async fn test_join_set_results_keep_spawn_order() {
        let mut join_set = JoinSet::new();
        for (idx, delay_ms) in [(0, 30), (1, 10), (2, 20)] {
            join_set.spawn(async move {
                sleep(Duration::from_millis(delay_ms)).await;
                (idx, idx)
            });
        }

        let results = collect_join_set_results(join_set, 3, "test_handler")
            .await
            .unwrap();

        assert_eq!(results, vec![0, 1, 2]);
    }

    /// Always answers with an undetermined apply outcome, and counts how many times
    /// it is dispatched. The retry loop re-shards a CLONE per attempt, so the count
    /// lives behind an `Arc` the clones share.
    #[derive(Clone, Default)]
    struct UndeterminedPlan {
        dispatches: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Plan for UndeterminedPlan {
        type Result = BatchGetResponse;

        async fn execute(&self) -> Result<Self::Result> {
            self.dispatches.fetch_add(1, Ordering::SeqCst);
            // The server says the apply outcome is UNKNOWN.
            Ok(BatchGetResponse {
                region_error: Some(errorpb::Error {
                    undetermined_result: Some(errorpb::UndeterminedResult::default()),
                    ..Default::default()
                }),
                ..Default::default()
            })
        }
    }

    impl Shardable for UndeterminedPlan {
        type Shard = ();

        fn shards(
            &self,
            _: &Arc<impl crate::pd::PdClient>,
        ) -> BoxStream<'static, crate::Result<(Self::Shard, RegionWithLeader)>> {
            Box::pin(stream::iter(vec![Ok(((), MockPdClient::region1()))])).boxed()
        }

        fn apply_shard(&mut self, _: Self::Shard) {}

        fn apply_store(&mut self, _: &crate::store::RegionStore) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn an_undetermined_result_is_terminal_on_first_sight_for_optedin_plans() {
        // CAS and commit points: a replay could contradict its own effect, and a
        // later different error must not overwrite the uncertainty. The error alone
        // cannot prove terminality — retrying to exhaustion resurfaces the SAME
        // error — so the proof is the dispatch count: a generous backoff that WOULD
        // allow 10 retries, and exactly one dispatch anyway.
        let inner = UndeterminedPlan::default();
        let dispatches = inner.dispatches.clone();
        let plan = RetryableMultiRegion {
            inner,
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_jitter_backoff(1, 2, 10),
            preserve_region_results: false,
            terminal_on_undetermined: true,
            terminal_on_dispatch_error: false,
        };
        let err = plan.execute().await.unwrap_err();
        assert!(
            is_undetermined_region_error(&err),
            "want the undetermined region error surfaced unchanged, got {err:?}"
        );
        assert_eq!(
            dispatches.load(Ordering::SeqCst),
            1,
            "terminal means the first sight is the last: no replay may follow"
        );
    }

    #[tokio::test]
    async fn an_undetermined_result_stays_recognizable_when_retries_exhaust() {
        // Everything else retries (client-go's action-layer behavior for ordinary
        // prewrites and secondary commits); when the backoff exhausts, the error
        // must escape UNCHANGED so commit paths can still classify it as undetermined.
        // The dispatch count proves the retries actually happened — the complement
        // of the terminal test above.
        const RETRIES: u32 = 3;
        let inner = UndeterminedPlan::default();
        let dispatches = inner.dispatches.clone();
        let plan = RetryableMultiRegion {
            inner,
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_jitter_backoff(1, 2, RETRIES),
            preserve_region_results: false,
            terminal_on_undetermined: false,
            terminal_on_dispatch_error: false,
        };
        let err = plan.execute().await.unwrap_err();
        assert!(
            is_undetermined_region_error(&err),
            "want the undetermined region error preserved, got {err:?}"
        );
        assert_eq!(
            dispatches.load(Ordering::SeqCst),
            RETRIES as usize + 1,
            "the default path retries: one initial dispatch plus one per backoff attempt"
        );
    }

    /// Always fails DISPATCH with a gRPC status error (the shape of a lost response:
    /// the request may have reached the server), counting dispatches like
    /// [`UndeterminedPlan`].
    #[derive(Clone, Default)]
    struct GrpcErrPlan {
        dispatches: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Plan for GrpcErrPlan {
        type Result = BatchGetResponse;

        async fn execute(&self) -> Result<Self::Result> {
            self.dispatches.fetch_add(1, Ordering::SeqCst);
            Err(Error::GrpcAPI(tonic::Status::unavailable("response lost")))
        }
    }

    impl Shardable for GrpcErrPlan {
        type Shard = ();

        fn shards(
            &self,
            _: &Arc<impl crate::pd::PdClient>,
        ) -> BoxStream<'static, crate::Result<(Self::Shard, RegionWithLeader)>> {
            Box::pin(stream::iter(vec![Ok(((), MockPdClient::region1()))])).boxed()
        }

        fn apply_shard(&mut self, _: Self::Shard) {}

        fn apply_store(&mut self, _: &crate::store::RegionStore) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn a_dispatch_error_is_terminal_on_first_sight_for_nonidempotent_plans() {
        // Raw CAS: a dispatch that may have reached the server is as ambiguous as
        // UndeterminedResult — a replay would compare against its own effect. Same
        // proof shape as the terminal-on-undetermined test: a backoff that WOULD
        // allow 10 retries, and exactly one dispatch anyway.
        let inner = GrpcErrPlan::default();
        let dispatches = inner.dispatches.clone();
        let plan = RetryableMultiRegion {
            inner,
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_jitter_backoff(1, 2, 10),
            preserve_region_results: false,
            terminal_on_undetermined: true,
            terminal_on_dispatch_error: true,
        };
        let err = plan.execute().await.unwrap_err();
        assert!(
            is_grpc_error(&err),
            "want the gRPC error surfaced unchanged, got {err:?}"
        );
        assert_eq!(
            dispatches.load(Ordering::SeqCst),
            1,
            "a non-idempotent request must never be replayed once it may have been sent"
        );
    }

    #[tokio::test]
    async fn a_dispatch_error_still_retries_for_commit_points() {
        // Commit points set only `terminal_on_undetermined`: replaying a commit is
        // idempotent — it can only resolve the uncertainty — and client-go's
        // transport likewise retries RPC errors there (region_request.go,
        // onSendFail). On exhaustion the error escapes unchanged for the commit
        // path to classify as undetermined.
        const RETRIES: u32 = 3;
        let inner = GrpcErrPlan::default();
        let dispatches = inner.dispatches.clone();
        let plan = RetryableMultiRegion {
            inner,
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_jitter_backoff(1, 2, RETRIES),
            preserve_region_results: false,
            terminal_on_undetermined: true,
            terminal_on_dispatch_error: false,
        };
        let err = plan.execute().await.unwrap_err();
        assert!(
            is_grpc_error(&err),
            "want the gRPC error preserved for the caller to classify, got {err:?}"
        );
        assert_eq!(
            dispatches.load(Ordering::SeqCst),
            RETRIES as usize + 1,
            "commit dispatches retry gRPC errors: one initial plus one per backoff attempt"
        );
    }

    #[test]
    fn undetermined_region_errors_are_recognized() {
        // errorpb.UndeterminedResult: the apply outcome is UNKNOWN. The commit path
        // must map this to UndeterminedError — a plain failure would invite the caller
        // to retry effects that may already be durable.
        let undetermined = Error::RegionError(Box::new(errorpb::Error {
            undetermined_result: Some(errorpb::UndeterminedResult::default()),
            ..Default::default()
        }));
        assert!(is_undetermined_region_error(&undetermined));

        let busy = Error::RegionError(Box::new(errorpb::Error {
            server_is_busy: Some(errorpb::ServerIsBusy::default()),
            ..Default::default()
        }));
        assert!(!is_undetermined_region_error(&busy));
        assert!(!is_undetermined_region_error(&Error::StringError(
            "x".to_owned()
        )));
    }

    /// A two-shard plan: the LOWER-index shard fails determinately, the HIGHER-index
    /// shard reports `UndeterminedResult`. Reproduces the masking hazard the shard
    /// aggregation guards against.
    #[derive(Clone)]
    struct MaskingPlan {
        idx: usize,
    }

    #[async_trait]
    impl Plan for MaskingPlan {
        type Result = BatchGetResponse;

        async fn execute(&self) -> Result<Self::Result> {
            if self.idx == 0 {
                // Determinate, lower index — the error that MUST NOT win.
                Err(Error::Unimplemented)
            } else {
                // Undetermined, higher index.
                Ok(BatchGetResponse {
                    region_error: Some(errorpb::Error {
                        undetermined_result: Some(errorpb::UndeterminedResult::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
            }
        }
    }

    impl Shardable for MaskingPlan {
        type Shard = usize;

        fn shards(
            &self,
            _: &Arc<impl crate::pd::PdClient>,
        ) -> BoxStream<'static, crate::Result<(Self::Shard, RegionWithLeader)>> {
            Box::pin(stream::iter(vec![
                Ok((0usize, MockPdClient::region1())),
                Ok((1usize, MockPdClient::region2())),
            ]))
            .boxed()
        }

        fn apply_shard(&mut self, shard: Self::Shard) {
            self.idx = shard;
        }

        fn apply_store(&mut self, _: &crate::store::RegionStore) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn an_undetermined_shard_is_not_masked_by_another_shards_error() {
        // THE MASKING HAZARD. Shard 0 fails determinately (lower index); shard 1 is
        // undetermined. First-error-by-index would report shard 0's determinate
        // failure and hide the fact that shard 1 may have applied — claiming clean
        // failure for a write that may be durable.
        // The aggregation must surface the undetermined error instead.
        let plan = RetryableMultiRegion {
            inner: MaskingPlan { idx: 0 },
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_backoff(),
            preserve_region_results: false,
            terminal_on_undetermined: true,
            terminal_on_dispatch_error: false,
        };
        let err = plan.execute().await.unwrap_err();
        assert!(
            is_undetermined_region_error(&err),
            "the undetermined shard must win over the determinate one, got {err:?}"
        );
    }

    /// A single-shard plan whose response carries a key error or a region error.
    #[derive(Clone)]
    struct FailingPlan {
        key_error: bool,
    }

    #[async_trait]
    impl Plan for FailingPlan {
        type Result = kvrpcpb::GetResponse;

        async fn execute(&self) -> Result<Self::Result> {
            let mut resp = kvrpcpb::GetResponse::default();
            if self.key_error {
                resp.error = Some(kvrpcpb::KeyError {
                    abort: "boom".to_owned(),
                    ..Default::default()
                });
            } else {
                resp.region_error = Some(errorpb::Error {
                    server_is_busy: Some(errorpb::ServerIsBusy::default()),
                    ..Default::default()
                });
            }
            Ok(resp)
        }
    }

    impl Shardable for FailingPlan {
        type Shard = ();

        fn shards(
            &self,
            _: &Arc<impl crate::pd::PdClient>,
        ) -> BoxStream<'static, crate::Result<(Self::Shard, RegionWithLeader)>> {
            Box::pin(stream::iter(vec![Ok(((), MockPdClient::region1()))])).boxed()
        }

        fn apply_shard(&mut self, _: Self::Shard) {}

        fn apply_store(&mut self, _: &crate::store::RegionStore) -> Result<()> {
            Ok(())
        }
    }

    fn retryable(key_error: bool) -> RetryableMultiRegion<FailingPlan, MockPdClient> {
        RetryableMultiRegion {
            inner: FailingPlan { key_error },
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_backoff(),
            preserve_region_results: false,
            terminal_on_undetermined: false,
            terminal_on_dispatch_error: false,
        }
    }

    /// THE INVARIANT THIS REFACTOR BUYS. A response key error must surface as
    /// `MultipleKeyErrors` whether or not a collapsing merge sits between the retry
    /// layer and `ExtractError`.
    ///
    /// Before the split these two shapes produced *different* variants — without a
    /// merge, `ExtractError` normalized the per-shard `Vec<Result<_>>` into
    /// `ExtractedErrors`; with `CollectSingle`, the merge popped the `Result` out first
    /// so `MultipleKeyErrors` escaped untouched. Callers matched one and silently missed
    /// the other, and adding a merge to any plan flipped it with no compile error.
    #[tokio::test]
    async fn key_errors_surface_under_one_variant_whatever_the_plan_shape() {
        // Shape A: no collapsing merge — ExtractError sees Vec<Result<_>>.
        let unmerged = ExtractError {
            inner: retryable(true),
        };
        assert!(
            matches!(unmerged.execute().await, Err(Error::MultipleKeyErrors(_))),
            "unmerged plan must report key errors as MultipleKeyErrors"
        );

        // Shape B: CollectSingle pops the single Result out before ExtractError.
        let merged = ExtractError {
            inner: MergeResponse {
                inner: retryable(true),
                merge: CollectSingle,
                phantom: PhantomData,
            },
        };
        assert!(
            matches!(merged.execute().await, Err(Error::MultipleKeyErrors(_))),
            "merged plan must report key errors as MultipleKeyErrors too"
        );
    }

    /// A bare plan whose response carries a region error, with no retry layer above it.
    #[derive(Clone)]
    struct RegionErrPlan;

    #[async_trait]
    impl Plan for RegionErrPlan {
        type Result = kvrpcpb::GetResponse;

        async fn execute(&self) -> Result<Self::Result> {
            Ok(kvrpcpb::GetResponse {
                region_error: Some(errorpb::Error {
                    server_is_busy: Some(errorpb::ServerIsBusy::default()),
                    ..Default::default()
                }),
                ..Default::default()
            })
        }
    }

    /// Raw endpoints report failures as a string `error` field, which `has_str_error!`
    /// turns into `Error::KvError` rather than `Error::KeyError`. That must be treated
    /// as response-originated too — otherwise `batch_put_with_ttl` (no merge) would
    /// return a bare `KvError` while `put_with_ttl` (CollectSingle) returned
    /// `MultipleKeyErrors`, and plan shape would still decide the public variant.
    #[tokio::test]
    async fn raw_string_errors_are_response_errors_under_either_plan_shape() {
        #[derive(Clone)]
        struct RawErrPlan;

        #[async_trait]
        impl Plan for RawErrPlan {
            type Result = kvrpcpb::RawPutResponse;

            async fn execute(&self) -> Result<Self::Result> {
                Ok(kvrpcpb::RawPutResponse {
                    error: "boom".to_owned(),
                    ..Default::default()
                })
            }
        }

        // Unmerged: ExtractError classifies the response's string error.
        let unmerged = ExtractError { inner: RawErrPlan };
        match unmerged.execute().await {
            Err(Error::MultipleKeyErrors(errors)) => {
                assert!(matches!(errors.as_slice(), [Error::KvError { .. }]));
            }
            other => panic!("want MultipleKeyErrors([KvError]), got {other:?}"),
        }

        // And a shard error carried through as a Vec<Result<_>> classifies the same way.
        #[derive(Clone)]
        struct PreservedRawErr;

        #[async_trait]
        impl Plan for PreservedRawErr {
            type Result = Vec<Result<kvrpcpb::RawPutResponse>>;

            async fn execute(&self) -> Result<Self::Result> {
                Ok(vec![Ok(kvrpcpb::RawPutResponse {
                    error: "boom".to_owned(),
                    ..Default::default()
                })])
            }
        }

        let preserved = ExtractError {
            inner: PreservedRawErr,
        };
        assert!(
            matches!(preserved.execute().await, Err(Error::MultipleKeyErrors(_))),
            "the same response error must not change variant with plan shape"
        );
    }

    /// A hard per-shard failure must keep its own class. `HasKeyErrors for
    /// Result<T, Error>` reports *any* `Err` as a key error, so without a guard in
    /// `ExtractError` a gRPC or exhausted-retry failure carried through a
    /// preserve-results plan would be relabelled `MultipleKeyErrors` — the exact
    /// mislabelling this refactor exists to remove.
    #[tokio::test]
    async fn a_non_key_error_is_not_relabelled_as_a_key_error() {
        #[derive(Clone)]
        struct PreservedShardErr;

        #[async_trait]
        impl Plan for PreservedShardErr {
            type Result = Vec<Result<kvrpcpb::GetResponse>>;

            async fn execute(&self) -> Result<Self::Result> {
                Ok(vec![Err(Error::Unimplemented)])
            }
        }

        let plan = ExtractError {
            inner: PreservedShardErr,
        };
        match plan.execute().await {
            Err(Error::Unimplemented) => {}
            other => panic!("want the hard error propagated unchanged, got {other:?}"),
        }
    }

    /// The region-error counterpart: a distinct variant, so a caller no longer has to
    /// pop an element and type-test it to learn which kind it is holding.
    ///
    /// Note the shape. `RetryableMultiRegion` consumes region errors itself — it retries
    /// them and, once the backoff is spent, returns a bare `Error::RegionError`. So
    /// `ExtractError` only ever sees a region error for a plan with no retry layer above
    /// it, which is exactly `resolve_lock_with_retry`'s shape: it drives the backoff
    /// itself and needs the error handed back rather than retried underneath it.
    #[tokio::test]
    async fn region_errors_surface_as_multiple_region_errors() {
        let plan = ExtractError {
            inner: RegionErrPlan,
        };
        match plan.execute().await {
            Err(Error::MultipleRegionErrors(errors)) => {
                assert!(matches!(errors.as_slice(), [Error::RegionError(_)]));
            }
            other => panic!("want MultipleRegionErrors, got {other:?}"),
        }
    }

    fn protocol_store(client: crate::mock::MockKvClient, range: Option<(u32, u32)>) -> RegionStore {
        let mut store = RegionStore::new(MockPdClient::region1(), Arc::new(client));
        store.meta = metapb::Store {
            id: store.region_with_leader.get_store_id().unwrap(),
            txn_protocol_version_range: range
                .map(|(min, max)| metapb::TxnProtocolVersionRange { min, max }),
            ..Default::default()
        };
        store
    }

    fn admission_rejection() -> errorpb::IncompatibleRequest {
        errorpb::IncompatibleRequest {
            reason: errorpb::IncompatibleRequestReason::TxnProtocolVersionOutOfRange as i32,
            message: "admission rejection".into(),
            provided_txn_protocol_version: 1,
            min_compatible_txn_protocol_version: 0,
            max_compatible_txn_protocol_version: 0,
        }
    }

    #[rstest::rstest]
    #[case(None, 1, Some(0))]
    #[case(Some((0, 0)), 1, Some(0))]
    #[case(Some((0, 2)), 1, Some(1))]
    #[case(Some((1, 1)), 1, Some(1))]
    #[case(Some((0, 1)), 0, Some(0))]
    #[case(Some((1, 1)), 0, None)]
    #[case(Some((2, 3)), 1, None)]
    #[case(Some((2, 1)), 1, None)]
    #[tokio::test]
    async fn protocol_selection_declares_origin_and_version_before_dispatch(
        #[case] range: Option<(u32, u32)>,
        #[case] configured: i32,
        #[case] expected: Option<u32>,
    ) {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let client = crate::mock::MockKvClient::with_dispatch_hook(move |request| {
            observed.fetch_add(1, Ordering::SeqCst);
            let context = request
                .downcast_ref::<kvrpcpb::GetRequest>()
                .unwrap()
                .context
                .as_ref()
                .unwrap();
            assert_eq!(Some(context.txn_protocol_version), expected);
            assert_eq!(context.request_origin, crate::RequestOrigin::TiFlash as i32);
            Ok(Box::new(kvrpcpb::GetResponse {
                value: b"value".to_vec(),
                ..Default::default()
            }))
        });
        let policy = crate::TxnRequestPolicy {
            origin: crate::RequestOrigin::TiFlash,
            protocol_version: crate::TxnProtocolVersion::try_from(configured).unwrap(),
        };
        let plan = crate::request::PlanBuilder::new(
            Arc::new(MockPdClient::default()),
            Keyspace::Disable,
            kvrpcpb::GetRequest::default(),
        )
        .txn_protocol_compatibility(policy)
        .single_region_with_store(protocol_store(client, range))
        .await
        .unwrap()
        .plan();
        let result = plan.execute().await;
        if expected.is_some() {
            assert_eq!(result.unwrap().value, b"value");
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        } else {
            let Err(Error::IncompatibleRequest(error)) = result else {
                panic!("expected a local rejection")
            };
            assert_eq!(
                error.reason,
                errorpb::IncompatibleRequestReason::Unknown as i32
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
    }

    #[rstest::rstest]
    #[case("upper", true)]
    #[case("unknown_reason", false)]
    #[case("invalid_range", false)]
    #[case("wrong_echo", false)]
    #[case("lower_bound", false)]
    #[case("in_range", false)]
    #[tokio::test]
    async fn protocol_rejection_resends_only_valid_upper_overflow(
        #[case] scenario: &str,
        #[case] resend: bool,
    ) {
        let mut rejection = admission_rejection();
        match scenario {
            "unknown_reason" => rejection.reason = 42,
            "invalid_range" => rejection.min_compatible_txn_protocol_version = 2,
            "wrong_echo" => rejection.provided_txn_protocol_version = 0,
            "lower_bound" => {
                rejection.min_compatible_txn_protocol_version = 2;
                rejection.max_compatible_txn_protocol_version = 2;
            }
            "in_range" => rejection.max_compatible_txn_protocol_version = 1,
            "upper" => {}
            _ => unreachable!(),
        }
        let original = rejection.clone();
        let versions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = versions.clone();
        let client = crate::mock::MockKvClient::with_dispatch_hook(move |request| {
            let version = request
                .downcast_ref::<kvrpcpb::GetRequest>()
                .unwrap()
                .context
                .as_ref()
                .unwrap()
                .txn_protocol_version;
            let mut sent = observed.lock().unwrap();
            sent.push(version);
            Ok(Box::new(kvrpcpb::GetResponse {
                region_error: (sent.len() == 1).then(|| errorpb::Error {
                    incompatible_request: Some(rejection.clone()),
                    not_leader: Some(Default::default()),
                    server_is_busy: Some(Default::default()),
                    ..Default::default()
                }),
                ..Default::default()
            }))
        });
        let routed = client.clone();
        let pd = Arc::new(
            MockPdClient::new(client)
                .with_map_region_to_store_hook(move |region| {
                    let mut store = protocol_store(routed.clone(), Some((0, 1)));
                    store.region_with_leader = region;
                    Ok(store)
                })
                .with_invalidate_region_hook(|_| {
                    panic!("compatibility rejection invalidated routing")
                }),
        );
        let plan = crate::request::PlanBuilder::new(
            pd,
            Keyspace::Disable,
            kvrpcpb::GetRequest {
                key: vec![1],
                ..Default::default()
            },
        )
        .txn_protocol_compatibility(crate::TxnRequestPolicy::default())
        .retry_multi_region(Backoff::no_jitter_backoff(1, 1, 3))
        .plan();
        let result = plan.execute().await;
        if resend {
            assert!(result.is_ok());
            assert_eq!(*versions.lock().unwrap(), vec![1, 0]);
        } else {
            let Err(Error::IncompatibleRequest(error)) = result else {
                panic!("expected terminal server rejection")
            };
            assert_eq!(*error, original);
            assert_eq!(*versions.lock().unwrap(), vec![1]);
        }
    }

    #[rstest::rstest]
    #[case(false, true, 2)]
    #[case(false, false, 4)]
    #[case(true, true, 1)]
    #[tokio::test]
    async fn protocol_undetermined_envelope_obeys_retry_policy(
        #[case] terminal: bool,
        #[case] recover: bool,
        #[case] expected_calls: usize,
    ) {
        let original = errorpb::Error {
            message: "unknown apply outcome".into(),
            undetermined_result: Some(Default::default()),
            incompatible_request: Some(admission_rejection()),
            not_leader: Some(Default::default()),
            server_is_busy: Some(Default::default()),
            ..Default::default()
        };
        let envelope = original.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let client = crate::mock::MockKvClient::with_dispatch_hook(move |request| {
            // Lock resolution and compatibility recovery must not handle this
            // mixed response before the retry policy sees the unknown outcome.
            let request = request.downcast_ref::<kvrpcpb::GetRequest>().unwrap();
            assert_eq!(request.context.as_ref().unwrap().txn_protocol_version, 1);
            let call = observed.fetch_add(1, Ordering::SeqCst);
            if recover && call > 0 {
                return Ok(Box::new(kvrpcpb::GetResponse {
                    value: b"recovered".to_vec(),
                    ..Default::default()
                }));
            }
            Ok(Box::new(kvrpcpb::GetResponse {
                region_error: Some(envelope.clone()),
                error: Some(kvrpcpb::KeyError {
                    abort: "must not mask unknown outcome".into(),
                    locked: Some(kvrpcpb::LockInfo {
                        key: vec![1],
                        primary_lock: vec![1],
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }))
        });
        let routed = client.clone();
        let pd = Arc::new(
            MockPdClient::new(client)
                .with_map_region_to_store_hook(move |region| {
                    let mut store = protocol_store(routed.clone(), Some((0, 1)));
                    store.region_with_leader = region;
                    Ok(store)
                })
                .with_invalidate_region_hook(|_| panic!("unknown outcome invalidated routing")),
        );
        let plan = crate::request::PlanBuilder::new(
            pd,
            Keyspace::Disable,
            kvrpcpb::GetRequest {
                key: vec![1],
                ..Default::default()
            },
        )
        .txn_protocol_compatibility(crate::TxnRequestPolicy::default())
        .resolve_lock(
            Timestamp::default(),
            Backoff::no_jitter_backoff(1, 1, 3),
            Keyspace::Disable,
        );
        let backoff = Backoff::no_jitter_backoff(1, 1, 3);
        let plan = if terminal {
            plan.retry_multi_region_terminal_on_undetermined(backoff)
        } else {
            plan.retry_multi_region(backoff)
        }
        .plan();
        let result = plan.execute().await;
        if recover && !terminal {
            assert_eq!(result.unwrap().pop().unwrap().unwrap().value, b"recovered");
        } else {
            let Err(Error::RegionError(error)) = result else {
                panic!("expected original unknown outcome")
            };
            assert_eq!(*error, original);
        };
        assert_eq!(calls.load(Ordering::SeqCst), expected_calls);
    }

    #[tokio::test]
    async fn protocol_allowances_are_independent_for_siblings_and_fresh_sends() {
        let versions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = versions.clone();
        let client = crate::mock::MockKvClient::with_dispatch_hook(move |request| {
            let request = request.downcast_ref::<kvrpcpb::BatchGetRequest>().unwrap();
            let version = request.context.as_ref().unwrap().txn_protocol_version;
            observed
                .lock()
                .unwrap()
                .push((request.keys.clone(), version));
            Ok(Box::new(kvrpcpb::BatchGetResponse {
                region_error: (version == 1).then(|| errorpb::Error {
                    incompatible_request: Some(admission_rejection()),
                    ..Default::default()
                }),
                ..Default::default()
            }))
        });
        let routed = client.clone();
        let pd = Arc::new(
            MockPdClient::new(client).with_map_region_to_store_hook(move |region| {
                let mut store = protocol_store(routed.clone(), Some((0, 1)));
                store.region_with_leader = region;
                Ok(store)
            }),
        );
        let plan = crate::request::PlanBuilder::new(
            pd,
            Keyspace::Disable,
            kvrpcpb::BatchGetRequest {
                keys: vec![vec![1], vec![11]],
                ..Default::default()
            },
        )
        .txn_protocol_compatibility(crate::TxnRequestPolicy::default())
        .retry_multi_region(Backoff::no_backoff())
        .plan();
        for _ in 0..2 {
            assert!(plan.execute().await.unwrap().iter().all(Result::is_ok));
        }
        let mut sent = versions.lock().unwrap().clone();
        sent.sort();
        assert_eq!(
            sent,
            vec![
                (vec![vec![1]], 0),
                (vec![vec![1]], 0),
                (vec![vec![1]], 1),
                (vec![vec![1]], 1),
                (vec![vec![11]], 0),
                (vec![vec![11]], 0),
                (vec![vec![11]], 1),
                (vec![vec![11]], 1),
            ]
        );
    }

    #[rstest::rstest]
    #[case(1, false)]
    #[case(2, false)]
    #[case(2, true)]
    #[tokio::test]
    async fn protocol_payload_requirement_controls_reload_and_dispatch(
        #[case] configured: i32,
        #[case] reload_succeeds: bool,
    ) {
        let reloads = Arc::new(AtomicUsize::new(0));
        let observed = reloads.clone();
        let pd = Arc::new(MockPdClient::default().with_reload_store_hook(move |id| {
            observed.fetch_add(1, Ordering::SeqCst);
            if !reload_succeeds {
                return Err(Error::Unimplemented);
            }
            Ok(metapb::Store {
                id,
                txn_protocol_version_range: Some(metapb::TxnProtocolVersionRange {
                    min: 0,
                    max: 2,
                }),
                ..Default::default()
            })
        }));
        let calls = Arc::new(AtomicUsize::new(0));
        let sent = calls.clone();
        let client = crate::mock::MockKvClient::with_dispatch_hook(move |request| {
            sent.fetch_add(1, Ordering::SeqCst);
            assert_eq!(
                request
                    .downcast_ref::<kvrpcpb::PrewriteRequest>()
                    .unwrap()
                    .context
                    .as_ref()
                    .unwrap()
                    .txn_protocol_version,
                2
            );
            Ok(Box::new(kvrpcpb::PrewriteResponse::default()))
        });
        // Direct policies let the test exercise future shared-lock payload negotiation.
        let policy = crate::TxnRequestPolicy {
            protocol_version: crate::TxnProtocolVersion::try_from(configured).unwrap(),
            ..Default::default()
        };
        let plan = crate::request::PlanBuilder::new(
            pd,
            Keyspace::Disable,
            kvrpcpb::PrewriteRequest {
                mutations: vec![kvrpcpb::Mutation {
                    op: kvrpcpb::Op::SharedLock as i32,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .txn_protocol_compatibility(policy)
        .single_region_with_store(protocol_store(client, Some((0, 1))))
        .await
        .unwrap()
        .plan();
        let result = plan.execute().await;
        assert_eq!(reloads.load(Ordering::SeqCst), usize::from(configured == 2));
        let should_send = configured == 2 && reload_succeeds;
        assert_eq!(calls.load(Ordering::SeqCst), usize::from(should_send));
        if should_send {
            assert!(result.is_ok());
        } else {
            let Err(Error::IncompatibleRequest(error)) = result else {
                panic!("expected local rejection")
            };
            assert_eq!(
                error.reason,
                errorpb::IncompatibleRequestReason::Unknown as i32
            );
            assert_eq!(error.max_compatible_txn_protocol_version, 1);
        }
    }

    #[tokio::test]
    async fn protocol_allowance_survives_region_retry_and_resharding() {
        let versions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = versions.clone();
        let invalidations = Arc::new(AtomicUsize::new(0));
        let invalidated = invalidations.clone();
        let client = crate::mock::MockKvClient::with_dispatch_hook(move |request| {
            let version = request
                .downcast_ref::<kvrpcpb::GetRequest>()
                .unwrap()
                .context
                .as_ref()
                .unwrap()
                .txn_protocol_version;
            let mut sent = observed.lock().unwrap();
            sent.push(version);
            let error = if sent.len() == 2 {
                errorpb::Error {
                    region_not_found: Some(Default::default()),
                    ..Default::default()
                }
            } else {
                errorpb::Error {
                    incompatible_request: Some(admission_rejection()),
                    ..Default::default()
                }
            };
            Ok(Box::new(kvrpcpb::GetResponse {
                region_error: Some(error),
                ..Default::default()
            }))
        });
        let routed = client.clone();
        let pd = Arc::new(
            MockPdClient::new(client)
                .with_map_region_to_store_hook(move |region| {
                    let mut store = protocol_store(routed.clone(), Some((0, 1)));
                    store.region_with_leader = region;
                    Ok(store)
                })
                .with_invalidate_region_hook(move |_| {
                    invalidated.fetch_add(1, Ordering::SeqCst);
                }),
        );
        let plan = crate::request::PlanBuilder::new(
            pd,
            Keyspace::Disable,
            kvrpcpb::GetRequest {
                key: vec![1],
                ..Default::default()
            },
        )
        .txn_protocol_compatibility(crate::TxnRequestPolicy::default())
        .retry_multi_region(Backoff::no_jitter_backoff(1, 1, 3))
        .plan();
        let Err(Error::IncompatibleRequest(error)) = plan.execute().await else {
            panic!("expected exhausted compatibility allowance")
        };
        assert_eq!(*error, admission_rejection());
        assert_eq!(*versions.lock().unwrap(), vec![1, 0, 1]);
        assert_eq!(invalidations.load(Ordering::SeqCst), 1);
    }

    #[rstest::rstest]
    #[case(false, false)]
    #[case(true, false)]
    #[case(false, true)]
    #[case(true, true)]
    #[tokio::test]
    async fn protocol_error_extraction_preserves_priority_across_sources(
        #[case] reverse: bool,
        #[case] undetermined: bool,
    ) {
        #[derive(Clone)]
        struct Errors {
            reverse: bool,
            undetermined: bool,
        }
        #[async_trait]
        impl Plan for Errors {
            type Result = Vec<Result<kvrpcpb::GetResponse>>;
            async fn execute(&self) -> Result<Self::Result> {
                let mut errors = vec![
                    Err(Error::Unimplemented),
                    Ok(kvrpcpb::GetResponse {
                        region_error: Some(errorpb::Error {
                            incompatible_request: Some(admission_rejection()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                ];
                if self.undetermined {
                    errors.push(Err(Error::RegionError(Box::new(errorpb::Error {
                        message: "original unknown outcome".into(),
                        undetermined_result: Some(Default::default()),
                        ..Default::default()
                    }))));
                }
                if self.reverse {
                    errors.reverse();
                }
                Ok(errors)
            }
        }
        let error = ExtractError {
            inner: Errors {
                reverse,
                undetermined,
            },
        }
        .execute()
        .await
        .unwrap_err();
        assert_eq!(
            error.priority(),
            if undetermined {
                ErrorPriority::Undetermined
            } else {
                ErrorPriority::Incompatible
            }
        );
        let Error::RegionError(error) = error else {
            panic!("original region envelope must be preserved")
        };
        if undetermined {
            assert_eq!(error.message, "original unknown outcome");
        } else {
            assert_eq!(error.incompatible_request, Some(admission_rejection()));
        }
    }

    #[rstest::rstest]
    #[case(ErrorPriority::Ordinary, ErrorPriority::Incompatible)]
    #[case(ErrorPriority::Incompatible, ErrorPriority::Ordinary)]
    #[case(ErrorPriority::Incompatible, ErrorPriority::Undetermined)]
    #[case(ErrorPriority::Undetermined, ErrorPriority::Incompatible)]
    #[case(ErrorPriority::Ordinary, ErrorPriority::Ordinary)]
    #[tokio::test]
    async fn protocol_shard_aggregation_preserves_highest_priority(
        #[case] first: ErrorPriority,
        #[case] second: ErrorPriority,
    ) {
        #[derive(Clone)]
        struct ShardErrors {
            index: usize,
            priorities: [ErrorPriority; 2],
        }
        #[async_trait]
        impl Plan for ShardErrors {
            type Result = kvrpcpb::GetResponse;
            async fn execute(&self) -> Result<Self::Result> {
                Err(match self.priorities[self.index] {
                    ErrorPriority::Ordinary => Error::StringError(format!("shard {}", self.index)),
                    ErrorPriority::Incompatible => {
                        Error::IncompatibleRequest(Box::new(admission_rejection()))
                    }
                    ErrorPriority::Undetermined => Error::RegionError(Box::new(errorpb::Error {
                        undetermined_result: Some(Default::default()),
                        ..Default::default()
                    })),
                })
            }
        }
        impl Shardable for ShardErrors {
            type Shard = usize;
            fn shards(
                &self,
                _: &Arc<impl PdClient>,
            ) -> BoxStream<'static, Result<(usize, RegionWithLeader)>> {
                Box::pin(stream::iter(vec![
                    Ok((0, MockPdClient::region1())),
                    Ok((1, MockPdClient::region2())),
                ]))
            }
            fn apply_shard(&mut self, index: usize) {
                self.index = index;
            }
            fn apply_store(&mut self, _: &RegionStore) -> Result<()> {
                Ok(())
            }
        }
        let plan = RetryableMultiRegion {
            inner: ShardErrors {
                index: 0,
                priorities: [first, second],
            },
            pd_client: Arc::new(MockPdClient::default()),
            backoff: Backoff::no_backoff(),
            preserve_region_results: false,
            terminal_on_undetermined: false,
            terminal_on_dispatch_error: false,
        };
        let error = plan.execute().await.unwrap_err();
        assert_eq!(error.priority(), first.min(second));
        if first == ErrorPriority::Ordinary && second == ErrorPriority::Ordinary {
            assert!(matches!(error, Error::StringError(message) if message == "shard 0"));
        }
    }

    #[tokio::test]
    async fn protocol_version_is_reselected_for_a_fallback_store() {
        let sent = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = sent.clone();
        let client = crate::mock::MockKvClient::with_dispatch_hook(move |request| {
            let context = request
                .downcast_ref::<kvrpcpb::GetRequest>()
                .unwrap()
                .context
                .as_ref()
                .unwrap();
            let store = context.peer.as_ref().unwrap().store_id;
            observed
                .lock()
                .unwrap()
                .push((store, context.txn_protocol_version));
            if store == 41 {
                return Err(Error::GrpcAPI(tonic::Status::unavailable(
                    "leader unavailable",
                )));
            }
            Ok(Box::new(kvrpcpb::GetResponse::default()))
        });
        let mut region = MockPdClient::region1();
        region.region.peers = vec![
            region.leader.clone().unwrap(),
            metapb::Peer {
                id: 2,
                store_id: 44,
                ..Default::default()
            },
        ];
        let located = region.clone();
        let routed = client.clone();
        let pd = Arc::new(
            MockPdClient::new(client)
                .with_region_for_key_hook(move |_| Ok(located.clone()))
                .with_map_region_to_store_hook(move |region| {
                    let id = region.get_store_id().unwrap();
                    let mut store = protocol_store(routed.clone(), Some((0, u32::from(id == 41))));
                    store.meta.id = id;
                    store.region_with_leader = region;
                    Ok(store)
                })
                .with_update_leader_hook(|_, leader| {
                    assert_eq!(leader.store_id, 44);
                    Ok(())
                }),
        );
        let plan = crate::request::PlanBuilder::new(
            pd,
            Keyspace::Disable,
            kvrpcpb::GetRequest {
                key: vec![1],
                ..Default::default()
            },
        )
        .txn_protocol_compatibility(crate::TxnRequestPolicy::default())
        .retry_multi_region(Backoff::no_backoff())
        .plan();
        assert!(plan.execute().await.is_ok());
        assert_eq!(*sent.lock().unwrap(), vec![(41, 1), (44, 0)]);
    }

    #[tokio::test]
    async fn protocol_shared_pessimistic_payload_cannot_be_declared_legacy() {
        let client = crate::mock::MockKvClient::with_dispatch_hook(|_| {
            panic!("unsupported payload reached dispatch")
        });
        let plan = crate::request::PlanBuilder::new(
            Arc::new(MockPdClient::default()),
            Keyspace::Disable,
            kvrpcpb::PessimisticLockRequest {
                mutations: vec![kvrpcpb::Mutation {
                    op: kvrpcpb::Op::SharedPessimisticLock as i32,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .txn_protocol_compatibility(crate::TxnRequestPolicy::default())
        .single_region_with_store(protocol_store(client, Some((0, 2))))
        .await
        .unwrap()
        .plan();
        let Err(Error::IncompatibleRequest(error)) = plan.execute().await else {
            panic!("expected payload rejection")
        };
        assert_eq!(error.provided_txn_protocol_version, 1);
        assert_eq!(error.max_compatible_txn_protocol_version, 2);
    }

    #[test]
    fn protocol_error_borrow_preserves_response_and_wrapper_state() {
        let original = errorpb::Error {
            message: "ordinary region error".into(),
            not_leader: Some(Default::default()),
            ..Default::default()
        };
        let mut wrapped: Result<ResponseWithShard<kvrpcpb::GetResponse, Vec<u8>>> =
            Ok(ResponseWithShard(
                kvrpcpb::GetResponse {
                    value: b"value".to_vec(),
                    region_error: Some(original.clone()),
                    ..Default::default()
                },
                vec![1],
            ));
        assert_eq!(wrapped.region_error_ref(), Some(&original));
        assert_eq!(wrapped.region_error_ref(), Some(&original));
        assert_eq!(wrapped.region_error(), Some(original));
        assert!(wrapped.region_error_ref().is_none());
        let ResponseWithShard(response, shard) = wrapped.unwrap();
        assert_eq!(response.value, b"value");
        assert_eq!(shard, vec![1]);
        let mut failed: Result<kvrpcpb::GetResponse> = Err(Error::Unimplemented);
        assert!(failed.region_error_ref().is_none());
        assert!(failed.region_error().is_none());
        assert!(matches!(failed, Err(Error::Unimplemented)));
    }
    #[rstest::rstest]
    #[case(false)]
    #[case(true)]
    #[tokio::test]
    async fn protocol_cleanup_undetermined_response_reaches_retry_policy(#[case] recover: bool) {
        let original = errorpb::Error {
            undetermined_result: Some(Default::default()),
            incompatible_request: Some(admission_rejection()),
            ..Default::default()
        };
        let envelope = original.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let client = crate::mock::MockKvClient::with_dispatch_hook(move |request| {
            let request = request.downcast_ref::<kvrpcpb::ScanLockRequest>().unwrap();
            assert_eq!(request.context.as_ref().unwrap().txn_protocol_version, 1);
            let call = observed.fetch_add(1, Ordering::SeqCst);
            if recover && call > 0 {
                return Ok(Box::new(kvrpcpb::ScanLockResponse::default()));
            }
            Ok(Box::new(kvrpcpb::ScanLockResponse {
                region_error: Some(envelope.clone()),
                error: Some(kvrpcpb::KeyError {
                    abort: "must not mask unknown outcome".into(),
                    ..Default::default()
                }),
                locks: vec![kvrpcpb::LockInfo {
                    key: vec![1],
                    primary_lock: vec![1],
                    ..Default::default()
                }],
                ..Default::default()
            }))
        });
        let routed = client.clone();
        let pd = Arc::new(
            MockPdClient::new(client)
                .with_map_region_to_store_hook(move |region| {
                    let mut store = protocol_store(routed.clone(), Some((0, 1)));
                    store.region_with_leader = region;
                    Ok(store)
                })
                .with_invalidate_region_hook(|_| panic!("unknown outcome invalidated routing")),
        );
        let plan = crate::request::PlanBuilder::new(
            pd,
            Keyspace::Disable,
            kvrpcpb::ScanLockRequest {
                start_key: vec![1],
                end_key: vec![9],
                limit: 1,
                ..Default::default()
            },
        )
        .txn_protocol_compatibility(crate::TxnRequestPolicy::default())
        .cleanup_locks(
            crate::transaction::ResolveLocksContext::default(),
            crate::transaction::ResolveLocksOptions::default(),
            Backoff::no_backoff(),
            Keyspace::Disable,
        )
        .retry_multi_region(Backoff::no_jitter_backoff(1, 1, 1))
        .plan();
        let result = plan.execute().await;
        if recover {
            assert!(result.unwrap().iter().all(Result::is_ok));
        } else {
            let Err(Error::RegionError(error)) = result else {
                panic!("expected original unknown outcome")
            };
            assert_eq!(*error, original);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[rstest::rstest]
    #[case(false)]
    #[case(true)]
    #[tokio::test]
    async fn protocol_cleanup_pages_renew_allowance_but_page_retries_do_not(
        #[case] second_page_routing_error: bool,
    ) {
        let sent = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = sent.clone();
        let client = crate::mock::MockKvClient::with_dispatch_hook(move |request| {
            let request = request.downcast_ref::<kvrpcpb::ScanLockRequest>().unwrap();
            let version = request.context.as_ref().unwrap().txn_protocol_version;
            observed
                .lock()
                .unwrap()
                .push((request.start_key.clone(), version));
            let response = if version == 1 {
                kvrpcpb::ScanLockResponse {
                    region_error: Some(errorpb::Error {
                        incompatible_request: Some(admission_rejection()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            } else if request.start_key == vec![1] {
                kvrpcpb::ScanLockResponse {
                    // A full page advances the scan. The async-only cleanup
                    // filter excludes this lock, keeping this test about pagination.
                    locks: vec![kvrpcpb::LockInfo {
                        key: vec![1],
                        ..Default::default()
                    }],
                    ..Default::default()
                }
            } else {
                assert_eq!(request.start_key, vec![1, 0]);
                kvrpcpb::ScanLockResponse {
                    region_error: second_page_routing_error.then(|| errorpb::Error {
                        region_not_found: Some(Default::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            };
            Ok(Box::new(response))
        });
        let routed = client.clone();
        let invalidations = Arc::new(AtomicUsize::new(0));
        let invalidated = invalidations.clone();
        let pd = Arc::new(
            MockPdClient::new(client)
                .with_map_region_to_store_hook(move |region| {
                    let mut store = protocol_store(routed.clone(), Some((0, 1)));
                    store.region_with_leader = region;
                    Ok(store)
                })
                .with_invalidate_region_hook(move |_| {
                    invalidated.fetch_add(1, Ordering::SeqCst);
                }),
        );
        let plan = crate::request::PlanBuilder::new(
            pd,
            Keyspace::Disable,
            kvrpcpb::ScanLockRequest {
                start_key: vec![1],
                end_key: vec![9],
                limit: 1,
                ..Default::default()
            },
        )
        .txn_protocol_compatibility(crate::TxnRequestPolicy::default())
        .cleanup_locks(
            crate::transaction::ResolveLocksContext::default(),
            crate::transaction::ResolveLocksOptions {
                async_commit_only: true,
                batch_size: 1,
            },
            Backoff::no_backoff(),
            Keyspace::Disable,
        )
        .retry_multi_region(Backoff::no_jitter_backoff(1, 1, 1))
        .plan();
        let result = plan.execute().await;
        let mut expected = vec![(vec![1], 1), (vec![1], 0), (vec![1, 0], 1), (vec![1, 0], 0)];
        if second_page_routing_error {
            // Region retry restarts the scan, but cannot replenish the allowance
            // spent by the failed page's resend.
            let Err(Error::IncompatibleRequest(error)) = result else {
                panic!("expected spent page allowance")
            };
            assert_eq!(*error, admission_rejection());
            expected.push((vec![1], 1));
            assert_eq!(invalidations.load(Ordering::SeqCst), 1);
        } else {
            assert!(result.unwrap().iter().all(Result::is_ok));
            assert_eq!(invalidations.load(Ordering::SeqCst), 0);
        }
        assert_eq!(*sent.lock().unwrap(), expected);
    }
}
