// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tonic::transport::Channel;
use tonic::IntoRequest;

use crate::backoff::Backoff;
use crate::pd::PdClient;
use crate::proto::errorpb;
use crate::proto::kvrpcpb;
use crate::proto::tikvpb::tikv_client::TikvClient;
use crate::region::StoreId;
use crate::store::RegionWithLeader;
use crate::store::TxnProtocolRequirement;
use crate::store::TxnProtocolVersionRange;
use crate::Error;
use crate::Result;

#[async_trait]
pub trait Request: Any + Sync + Send + 'static {
    async fn dispatch(
        &self,
        client: &TikvClient<Channel>,
        timeout: Duration,
    ) -> Result<Box<dyn Any>>;
    fn label(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn set_leader(&mut self, leader: &RegionWithLeader) -> Result<()>;
    fn set_api_version(&mut self, api_version: kvrpcpb::ApiVersion);
    fn txn_protocol_requirement(&self) -> TxnProtocolRequirement;
    /// Validate and select the transaction protocol version, then apply RPC metadata.
    /// Incompatible requests leave the context unchanged; non-transactional requests
    /// are untouched. Routing and leader selection are handled separately.
    fn prepare_txn_rpc(
        &mut self,
        range: TxnProtocolVersionRange,
        default_version: u32,
        request_origin: i32,
    ) -> Result<()>;
}

fn select_txn_protocol_version(
    range: TxnProtocolVersionRange,
    default_version: u32,
    required_version: u32,
) -> Result<u32> {
    if let Some(selected) = range.select(default_version, required_version) {
        return Ok(selected);
    }
    let selected = range.preferred_version(default_version);
    let incompatible = crate::proto::errorpb::IncompatibleRequest {
        // This is a client-side error, not a server admission rejection.
        // `Unknown` keeps the reload/retry path from misclassifying it.
        reason: crate::proto::errorpb::IncompatibleRequestReason::Unknown as i32,
        message: format!(
            "no compatible transaction protocol version: client={default_version}, required={required_version}, store=[{}, {}]",
            range.min, range.max
        ),
        provided_txn_protocol_version: selected,
        min_compatible_txn_protocol_version: range.min,
        max_compatible_txn_protocol_version: range.max,
    };
    Err(Error::IncompatibleRequest(Box::new(incompatible)))
}

fn is_upper_txn_protocol_admission_rejection(
    incompatible: &errorpb::IncompatibleRequest,
    selected_version: u32,
) -> bool {
    incompatible.reason == errorpb::IncompatibleRequestReason::TxnProtocolVersionOutOfRange as i32
        && incompatible.min_compatible_txn_protocol_version
            <= incompatible.max_compatible_txn_protocol_version
        && incompatible.provided_txn_protocol_version == selected_version
        && incompatible.provided_txn_protocol_version
            > incompatible.max_compatible_txn_protocol_version
}

/// Reload only after a side-effect-free upper admission rejection, and only
/// propose a retry if the refreshed metadata selects a different valid version.
/// Callers retain responsibility for routing, applying the range, and returning
/// the original incompatible error when no progress is possible.
pub(crate) async fn reload_txn_protocol_range<PdC: PdClient>(
    pd_client: Arc<PdC>,
    store_id: StoreId,
    old_range: TxnProtocolVersionRange,
    default_version: u32,
    requirement: TxnProtocolRequirement,
    incompatible: &errorpb::IncompatibleRequest,
    backoff: &mut Backoff,
) -> Option<TxnProtocolVersionRange> {
    let selected = requirement.select(old_range, default_version)?;
    if !is_upper_txn_protocol_admission_rejection(incompatible, selected)
        || backoff.next_delay_duration().is_none()
    {
        return None;
    }
    let range = pd_client.reload_store(store_id).await.ok()??;
    let new_version = requirement.select(range, default_version)?;
    (new_version != selected).then_some(range)
}

// Derive the version from the payload so neither shared-lock op can be
// declared as legacy, even when it appears in the wrong request type.
fn required_txn_protocol_version(mutations: &[kvrpcpb::Mutation]) -> u32 {
    if mutations.iter().any(|mutation| {
        mutation.op == kvrpcpb::Op::SharedLock as i32
            || mutation.op == kvrpcpb::Op::SharedPessimisticLock as i32
    }) {
        kvrpcpb::TxnProtocolVersion::TxnVerSupportSharedLock as u32
    } else {
        0
    }
}

macro_rules! impl_request {
    ($name: ident, $fun: ident, $label: literal, $requirement: expr) => {
        impl_request_with_requirement!($name, $fun, $label, |_: &kvrpcpb::$name| $requirement);
    };
}

macro_rules! impl_txn_request_with_payload_requirement {
    ($name: ident, $fun: ident, $label: literal) => {
        impl_request_with_requirement!($name, $fun, $label, |request: &kvrpcpb::$name| {
            TxnProtocolRequirement::Transaction {
                required_version: required_txn_protocol_version(&request.mutations),
            }
        });
    };
}

macro_rules! impl_request_with_requirement {
    ($name: ident, $fun: ident, $label: literal, $classifier: expr) => {
        #[async_trait]
        impl Request for kvrpcpb::$name {
            async fn dispatch(
                &self,
                client: &TikvClient<Channel>,
                timeout: Duration,
            ) -> Result<Box<dyn Any>> {
                let mut req = self.clone().into_request();
                req.set_timeout(timeout);
                client
                    .clone()
                    .$fun(req)
                    .await
                    .map(|r| Box::new(r.into_inner()) as Box<dyn Any>)
                    .map_err(Error::from)
            }

            fn label(&self) -> &'static str {
                $label
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn set_leader(&mut self, leader: &RegionWithLeader) -> Result<()> {
                let ctx = self.context.get_or_insert(kvrpcpb::Context::default());
                let leader_peer = leader.leader.as_ref().ok_or(Error::LeaderNotFound {
                    region: leader.ver_id(),
                })?;
                ctx.region_id = leader.region.id;
                ctx.region_epoch = leader.region.region_epoch.clone();
                ctx.peer = Some(leader_peer.clone());
                Ok(())
            }

            fn set_api_version(&mut self, api_version: kvrpcpb::ApiVersion) {
                let ctx = self.context.get_or_insert(kvrpcpb::Context::default());
                ctx.api_version = api_version.into();
            }

            fn txn_protocol_requirement(&self) -> TxnProtocolRequirement {
                ($classifier)(self)
            }

            fn prepare_txn_rpc(
                &mut self,
                range: TxnProtocolVersionRange,
                default_version: u32,
                request_origin: i32,
            ) -> Result<()> {
                match self.txn_protocol_requirement() {
                    TxnProtocolRequirement::NotTransaction => Ok(()),
                    TxnProtocolRequirement::Transaction { required_version } => {
                        let selected =
                            select_txn_protocol_version(range, default_version, required_version)?;
                        let context = self.context.get_or_insert_with(kvrpcpb::Context::default);
                        context.txn_protocol_version = selected;
                        if context.request_origin == kvrpcpb::RequestOrigin::Unknown as i32 {
                            context.request_origin = request_origin;
                        }
                        Ok(())
                    }
                }
            }
        }
    };
}

macro_rules! impl_raw_request {
    ($name: ident, $fun: ident, $label: literal) => {
        impl_request!($name, $fun, $label, TxnProtocolRequirement::NotTransaction);
    };
}

macro_rules! impl_txn_request {
    ($name: ident, $fun: ident, $label: literal) => {
        impl_request!(
            $name,
            $fun,
            $label,
            (TxnProtocolRequirement::Transaction {
                required_version: 0
            })
        );
    };
}

impl_raw_request!(RawGetRequest, raw_get, "raw_get");
impl_raw_request!(RawBatchGetRequest, raw_batch_get, "raw_batch_get");
impl_raw_request!(RawGetKeyTtlRequest, raw_get_key_ttl, "raw_get_key_ttl");
impl_raw_request!(RawPutRequest, raw_put, "raw_put");
impl_raw_request!(RawBatchPutRequest, raw_batch_put, "raw_batch_put");
impl_raw_request!(RawDeleteRequest, raw_delete, "raw_delete");
impl_raw_request!(RawBatchDeleteRequest, raw_batch_delete, "raw_batch_delete");
impl_raw_request!(RawScanRequest, raw_scan, "raw_scan");
impl_raw_request!(RawBatchScanRequest, raw_batch_scan, "raw_batch_scan");
impl_raw_request!(RawDeleteRangeRequest, raw_delete_range, "raw_delete_range");
impl_raw_request!(RawCasRequest, raw_compare_and_swap, "raw_compare_and_swap");
impl_raw_request!(RawCoprocessorRequest, raw_coprocessor, "raw_coprocessor");

impl_txn_request!(GetRequest, kv_get, "kv_get");
impl_txn_request!(ScanRequest, kv_scan, "kv_scan");
impl_txn_request_with_payload_requirement!(PrewriteRequest, kv_prewrite, "kv_prewrite");
impl_txn_request!(CommitRequest, kv_commit, "kv_commit");
impl_txn_request!(BatchGetRequest, kv_batch_get, "kv_batch_get");
impl_txn_request!(BatchRollbackRequest, kv_batch_rollback, "kv_batch_rollback");
impl_txn_request!(
    PessimisticRollbackRequest,
    kv_pessimistic_rollback,
    "kv_pessimistic_rollback"
);
impl_txn_request!(ResolveLockRequest, kv_resolve_lock, "kv_resolve_lock");
impl_txn_request!(ScanLockRequest, kv_scan_lock, "kv_scan_lock");
impl_txn_request_with_payload_requirement!(
    PessimisticLockRequest,
    kv_pessimistic_lock,
    "kv_pessimistic_lock"
);
impl_txn_request!(TxnHeartBeatRequest, kv_txn_heart_beat, "kv_txn_heart_beat");
impl_txn_request!(
    CheckTxnStatusRequest,
    kv_check_txn_status,
    "kv_check_txn_status"
);
impl_txn_request!(
    CheckSecondaryLocksRequest,
    kv_check_secondary_locks,
    "kv_check_secondary_locks_request"
);
impl_txn_request!(GcRequest, kv_gc, "kv_gc");
impl_txn_request!(DeleteRangeRequest, kv_delete_range, "kv_delete_range");
impl_request!(
    UnsafeDestroyRangeRequest,
    unsafe_destroy_range,
    "unsafe_destroy_range",
    TxnProtocolRequirement::NotTransaction
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{MockKvClient, ProtocolTestClient, ReloadOutcome};

    #[rstest::rstest]
    #[case(ReloadOutcome::Changed, Some(TxnProtocolVersionRange { min: 0, max: 0 }))]
    #[case(ReloadOutcome::Unchanged, None)]
    #[case(ReloadOutcome::Invalid, None)]
    #[case(ReloadOutcome::Missing, None)]
    #[case(ReloadOutcome::Failed, None)]
    #[tokio::test]
    async fn reload_requires_a_different_compatible_version(
        #[case] outcome: ReloadOutcome,
        #[case] expected: Option<TxnProtocolVersionRange>,
    ) {
        let fixture = ProtocolTestClient::new(MockKvClient::default(), outcome);
        let range = reload_txn_protocol_range(
            fixture.pd_client,
            41,
            TxnProtocolVersionRange { min: 0, max: 1 },
            1,
            TxnProtocolRequirement::Transaction {
                required_version: 0,
            },
            &crate::mock::upper_admission_rejection(),
            &mut Backoff::no_jitter_backoff(0, 0, 1),
        )
        .await;
        assert_eq!(range, expected);
        assert_eq!(*fixture.reloads.lock().unwrap(), vec![41]);
    }

    #[test]
    fn only_a_well_formed_upper_admission_rejection_can_reload() {
        let mut incompatible = crate::mock::upper_admission_rejection();
        assert!(is_upper_txn_protocol_admission_rejection(&incompatible, 1));
        incompatible.reason = errorpb::IncompatibleRequestReason::Unknown as i32;
        assert!(!is_upper_txn_protocol_admission_rejection(&incompatible, 1));
        incompatible.reason =
            errorpb::IncompatibleRequestReason::TxnProtocolVersionOutOfRange as i32;
        incompatible.min_compatible_txn_protocol_version = 2;
        assert!(!is_upper_txn_protocol_admission_rejection(&incompatible, 1));
        incompatible.min_compatible_txn_protocol_version = 0;
        assert!(!is_upper_txn_protocol_admission_rejection(&incompatible, 0));
    }

    #[tokio::test]
    async fn reload_does_not_downgrade_below_the_request_requirement() {
        let fixture = ProtocolTestClient::new(MockKvClient::default(), ReloadOutcome::Changed);
        let incompatible = errorpb::IncompatibleRequest {
            provided_txn_protocol_version: 2,
            ..crate::mock::upper_admission_rejection()
        };
        let range = reload_txn_protocol_range(
            fixture.pd_client,
            41,
            TxnProtocolVersionRange { min: 0, max: 2 },
            2,
            TxnProtocolRequirement::Transaction {
                required_version: 2,
            },
            &incompatible,
            &mut Backoff::no_jitter_backoff(0, 0, 1),
        )
        .await;
        assert!(range.is_none());
        assert_eq!(*fixture.reloads.lock().unwrap(), vec![41]);
    }

    #[tokio::test]
    async fn raw_requests_and_exhausted_budgets_do_not_reload() {
        let fixture = ProtocolTestClient::new(MockKvClient::default(), ReloadOutcome::Changed);
        for (requirement, mut backoff) in [
            (
                TxnProtocolRequirement::NotTransaction,
                Backoff::no_jitter_backoff(0, 0, 1),
            ),
            (
                TxnProtocolRequirement::Transaction {
                    required_version: 0,
                },
                Backoff::no_backoff(),
            ),
        ] {
            let range = reload_txn_protocol_range(
                fixture.pd_client.clone(),
                41,
                TxnProtocolVersionRange { min: 0, max: 1 },
                1,
                requirement,
                &crate::mock::upper_admission_rejection(),
                &mut backoff,
            )
            .await;
            assert!(range.is_none());
        }
        assert!(fixture.reloads.lock().unwrap().is_empty());
    }

    #[test]
    fn request_classification_is_explicit() {
        macro_rules! assert_txn {
            ($($request:ty),+ $(,)?) => {$({
                let request = <$request>::default();
                assert_eq!(
                    Request::txn_protocol_requirement(&request),
                    TxnProtocolRequirement::Transaction { required_version: 0 },
                );
            })+};
        }
        macro_rules! assert_raw {
            ($($request:ty),+ $(,)?) => {$({
                let request = <$request>::default();
                assert_eq!(
                    Request::txn_protocol_requirement(&request),
                    TxnProtocolRequirement::NotTransaction,
                );
                let mut request = request;
                request.prepare_txn_rpc(
                    TxnProtocolVersionRange { min: 2, max: 1 },
                    1,
                    kvrpcpb::RequestOrigin::TiFlash as i32,
                ).unwrap();
                assert!(request.context.is_none());
            })+};
        }

        assert_raw!(
            kvrpcpb::RawGetRequest,
            kvrpcpb::RawBatchGetRequest,
            kvrpcpb::RawGetKeyTtlRequest,
            kvrpcpb::RawPutRequest,
            kvrpcpb::RawBatchPutRequest,
            kvrpcpb::RawDeleteRequest,
            kvrpcpb::RawBatchDeleteRequest,
            kvrpcpb::RawScanRequest,
            kvrpcpb::RawBatchScanRequest,
            kvrpcpb::RawDeleteRangeRequest,
            kvrpcpb::RawCasRequest,
            kvrpcpb::RawCoprocessorRequest,
            kvrpcpb::UnsafeDestroyRangeRequest,
        );
        assert_txn!(
            kvrpcpb::GetRequest,
            kvrpcpb::ScanRequest,
            kvrpcpb::PrewriteRequest,
            kvrpcpb::CommitRequest,
            kvrpcpb::BatchGetRequest,
            kvrpcpb::BatchRollbackRequest,
            kvrpcpb::PessimisticRollbackRequest,
            kvrpcpb::ResolveLockRequest,
            kvrpcpb::ScanLockRequest,
            kvrpcpb::PessimisticLockRequest,
            kvrpcpb::TxnHeartBeatRequest,
            kvrpcpb::CheckTxnStatusRequest,
            kvrpcpb::CheckSecondaryLocksRequest,
            kvrpcpb::GcRequest,
            kvrpcpb::DeleteRangeRequest,
        );
    }

    #[test]
    fn payload_requirement_recognizes_both_shared_ops_in_both_requests() {
        fn assert_requirement(mut request: impl Request, required_version: u32) {
            assert_eq!(
                request.txn_protocol_requirement(),
                TxnProtocolRequirement::Transaction { required_version },
            );
            let result = request.prepare_txn_rpc(
                TxnProtocolVersionRange { min: 0, max: 1 },
                1,
                kvrpcpb::RequestOrigin::Unknown as i32,
            );
            if required_version == 0 {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(Error::IncompatibleRequest(_))));
            }
        }

        for (op, required_version) in [
            (kvrpcpb::Op::Put, 0),
            (
                kvrpcpb::Op::SharedLock,
                kvrpcpb::TxnProtocolVersion::TxnVerSupportSharedLock as u32,
            ),
            (
                kvrpcpb::Op::SharedPessimisticLock,
                kvrpcpb::TxnProtocolVersion::TxnVerSupportSharedLock as u32,
            ),
        ] {
            let mutations = vec![
                kvrpcpb::Mutation {
                    op: kvrpcpb::Op::Put as i32,
                    ..Default::default()
                },
                kvrpcpb::Mutation {
                    op: op as i32,
                    ..Default::default()
                },
            ];
            assert_requirement(
                kvrpcpb::PrewriteRequest {
                    mutations: mutations.clone(),
                    ..Default::default()
                },
                required_version,
            );
            assert_requirement(
                kvrpcpb::PessimisticLockRequest {
                    mutations,
                    ..Default::default()
                },
                required_version,
            );
        }
    }

    #[test]
    fn transaction_metadata_is_reselected_per_store() {
        let mut request = kvrpcpb::GetRequest::default();
        request.context = Some(kvrpcpb::Context {
            txn_protocol_version: 99,
            request_origin: kvrpcpb::RequestOrigin::Unknown as i32,
            ..Default::default()
        });

        for (max, origin, expected_version) in [
            (0, kvrpcpb::RequestOrigin::TiFlash, 0),
            (2, kvrpcpb::RequestOrigin::Br, 1),
        ] {
            request
                .prepare_txn_rpc(TxnProtocolVersionRange { min: 0, max }, 1, origin as i32)
                .unwrap();
            let context = request.context.as_ref().unwrap();
            assert_eq!(context.txn_protocol_version, expected_version);
            assert_eq!(
                context.request_origin,
                kvrpcpb::RequestOrigin::TiFlash as i32
            );
        }
    }

    #[rstest::rstest]
    #[case::client_below_required(0, 2, kvrpcpb::Op::SharedLock)]
    #[case::store_below_required(0, 1, kvrpcpb::Op::SharedLock)]
    #[case::client_below_store_min(2, 2, kvrpcpb::Op::Put)]
    #[case::malformed_range(2, 1, kvrpcpb::Op::Put)]
    fn incompatible_selection_does_not_modify_context(
        #[case] min: u32,
        #[case] max: u32,
        #[case] op: kvrpcpb::Op,
        #[values(false, true)] existing_context: bool,
    ) {
        let context = existing_context.then_some(kvrpcpb::Context {
            txn_protocol_version: 99,
            request_origin: kvrpcpb::RequestOrigin::TiFlash as i32,
            ..Default::default()
        });
        let mut request = kvrpcpb::PrewriteRequest {
            context: context.clone(),
            mutations: vec![kvrpcpb::Mutation {
                op: op as i32,
                ..Default::default()
            }],
            ..Default::default()
        };
        let error = Request::prepare_txn_rpc(
            &mut request,
            TxnProtocolVersionRange { min, max },
            1,
            kvrpcpb::RequestOrigin::Br as i32,
        )
        .unwrap_err();
        assert!(matches!(error, Error::IncompatibleRequest(incompatible)
            if incompatible.reason == crate::proto::errorpb::IncompatibleRequestReason::Unknown as i32));
        assert_eq!(request.context, context);
    }
}
