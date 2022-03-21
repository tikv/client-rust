// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{Error, Result};
use async_trait::async_trait;
use grpcio::CallOption;
use std::any::Any;
use tikv_client_proto::{kvrpcpb, tikvpb::TikvClient};

#[async_trait]
pub trait Request: Any + Sync + Send + 'static {
    async fn dispatch(&self, client: &TikvClient, options: CallOption) -> Result<Box<dyn Any>>;
    fn label(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn set_context(&mut self, context: kvrpcpb::Context);
}

macro_rules! impl_request {
    ($name: ident, $fun: ident, $label: literal) => {
        #[async_trait]
        impl Request for kvrpcpb::$name {
            async fn dispatch(
                &self,
                client: &TikvClient,
                options: CallOption,
            ) -> Result<Box<dyn Any>> {
                client
                    .$fun(self, options)?
                    .await
                    .map(|r| Box::new(r) as Box<dyn Any>)
                    .map_err(Error::Grpc)
            }

            fn label(&self) -> &'static str {
                $label
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn set_context(&mut self, context: kvrpcpb::Context) {
                kvrpcpb::$name::set_context(self, context)
            }
        }
    };
}

impl_request!(RawGetRequest, raw_get_async_opt, "raw_get");
impl_request!(RawBatchGetRequest, raw_batch_get_async_opt, "raw_batch_get");
impl_request!(RawPutRequest, raw_put_async_opt, "raw_put");
impl_request!(RawBatchPutRequest, raw_batch_put_async_opt, "raw_batch_put");
impl_request!(RawDeleteRequest, raw_delete_async_opt, "raw_delete");
impl_request!(
    RawBatchDeleteRequest,
    raw_batch_delete_async_opt,
    "raw_batch_delete"
);
impl_request!(RawScanRequest, raw_scan_async_opt, "raw_scan");
impl_request!(
    RawBatchScanRequest,
    raw_batch_scan_async_opt,
    "raw_batch_scan"
);
impl_request!(
    RawDeleteRangeRequest,
    raw_delete_range_async_opt,
    "raw_delete_range"
);
impl_request!(
    RawCasRequest,
    raw_compare_and_swap_async_opt,
    "raw_compare_and_swap"
);
impl_request!(
    RawCoprocessorRequest,
    raw_coprocessor_async_opt,
    "raw_coprocessor"
);

impl_request!(GetRequest, kv_get_async_opt, "kv_get");
impl_request!(ScanRequest, kv_scan_async_opt, "kv_scan");
impl_request!(PrewriteRequest, kv_prewrite_async_opt, "kv_prewrite");
impl_request!(CommitRequest, kv_commit_async_opt, "kv_commit");
impl_request!(CleanupRequest, kv_cleanup_async_opt, "kv_cleanup");
impl_request!(BatchGetRequest, kv_batch_get_async_opt, "kv_batch_get");
impl_request!(
    BatchRollbackRequest,
    kv_batch_rollback_async_opt,
    "kv_batch_rollback"
);
impl_request!(
    PessimisticRollbackRequest,
    kv_pessimistic_rollback_async_opt,
    "kv_pessimistic_rollback"
);
impl_request!(
    ResolveLockRequest,
    kv_resolve_lock_async_opt,
    "kv_resolve_lock"
);
impl_request!(ScanLockRequest, kv_scan_lock_async_opt, "kv_scan_lock");
impl_request!(
    PessimisticLockRequest,
    kv_pessimistic_lock_async_opt,
    "kv_pessimistic_lock"
);
impl_request!(
    TxnHeartBeatRequest,
    kv_txn_heart_beat_async_opt,
    "kv_txn_heart_beat"
);
impl_request!(
    CheckTxnStatusRequest,
    kv_check_txn_status_async_opt,
    "kv_check_txn_status"
);
impl_request!(
    CheckSecondaryLocksRequest,
    kv_check_secondary_locks_async_opt,
    "kv_check_secondary_locks_request"
);
impl_request!(GcRequest, kv_gc_async_opt, "kv_gc");
impl_request!(
    DeleteRangeRequest,
    kv_delete_range_async_opt,
    "kv_delete_range"
);
