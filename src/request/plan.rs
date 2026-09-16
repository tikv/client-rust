// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;
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
            && self
                .region_error
                .as_ref()
                .is_some_and(|error| error.not_leader.is_some())
    }

    fn is_fallback_server_busy(&self) -> bool {
        self.used_fallback
            && self.key_errors.is_none()
            && self
                .region_error
                .as_ref()
                .is_some_and(|error| error.server_is_busy.is_some())
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
            // A terminal undetermined outcome must never be masked by another shard's
            // determinate error. `collect::<Result<_>>()` would return the first error
            // by shard index, so if a lower-index shard failed with (say) a gRPC error
            // while a higher-index shard reported `UndeterminedResult`, the caller would
            // be told the request definitely failed — and might replay a transaction
            // that may already be durable. So an undetermined error
            // from ANY shard wins over a determinate one.
            let mut oks = Vec::with_capacity(results.len());
            let mut first_err: Option<Error> = None;
            let mut undetermined: Option<Error> = None;
            for r in results {
                match r {
                    Ok(v) => oks.push(v),
                    Err(e) if undetermined.is_none() && is_undetermined_region_error(&e) => {
                        undetermined = Some(e)
                    }
                    Err(e) if first_err.is_none() => first_err = Some(e),
                    Err(_) => {}
                }
            }
            if let Some(e) = undetermined.or(first_err) {
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

        if let Some(error) = key_errors {
            debug!("single_shard_handler:execute: key errors: {:?}", error);
            return Ok(vec![Err(Error::MultipleKeyErrors(error))]);
        }
        let Some(error) = region_error else {
            return Ok(vec![Ok(response)]);
        };

        if let Some(server_is_busy) = error.server_is_busy.as_ref() {
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
    } else if e.undetermined_result.is_some() {
        // The apply outcome is UNKNOWN (a raft timeout, errorpb.UndeterminedResult).
        // Default: retry — matching client-go's ACTION layers (ordinary prewrites and
        // secondary commits back off and re-send; re-applying an idempotent request
        // resolves the uncertainty). Routing is not suspect, so nothing is
        // invalidated. On backoff exhaustion the error escapes UNCHANGED, and commit
        // paths classify it via `is_undetermined_region_error`. Plans for which a
        // replay is unsafe never reach this arm — see `terminal_on_undetermined`.
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
}

impl<P: Plan, PdC: PdClient> Clone for ResolveLock<P, PdC> {
    fn clone(&self) -> Self {
        ResolveLock {
            inner: self.inner.clone(),
            timestamp: self.timestamp.clone(),
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
            let live_locks = resolve_locks(
                locks,
                self.timestamp.clone(),
                pd_client.clone(),
                self.keyspace,
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
}
