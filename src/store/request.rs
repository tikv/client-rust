// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::any::Any;
use std::time::Duration;

use async_trait::async_trait;
use tonic::transport::Channel;
use tonic::IntoRequest;

use crate::proto::kvrpcpb;
use crate::proto::tikvpb::tikv_client::TikvClient;
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
    /// Set the context for the request.
    /// Should always use `set_context` other than modify the `self.context` directly.
    fn set_context(&mut self, context: kvrpcpb::Context);
    fn set_api_version(&mut self, api_version: kvrpcpb::ApiVersion);
}

macro_rules! impl_request {
    ($name: ident, $fun: ident, $label: literal) => {
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
                    .map_err(Error::GrpcAPI)
            }

            fn label(&self) -> &'static str {
                $label
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn set_context(&mut self, context: kvrpcpb::Context) {
                let api_version = self
                    .context
                    .as_ref()
                    .map(|c| c.api_version)
                    .unwrap_or_default();
                self.context = Some(context);
                self.set_api_version(kvrpcpb::ApiVersion::from_i32(api_version).unwrap());
            }

            fn set_api_version(&mut self, api_version: kvrpcpb::ApiVersion) {
                let context = self.context.get_or_insert(kvrpcpb::Context::default());
                context.api_version = api_version.into();
            }
        }
    };
}

impl_request!(RawGetRequest, raw_get, "raw_get");
impl_request!(RawBatchGetRequest, raw_batch_get, "raw_batch_get");
impl_request!(RawPutRequest, raw_put, "raw_put");
impl_request!(RawBatchPutRequest, raw_batch_put, "raw_batch_put");
impl_request!(RawDeleteRequest, raw_delete, "raw_delete");
impl_request!(RawBatchDeleteRequest, raw_batch_delete, "raw_batch_delete");
impl_request!(RawScanRequest, raw_scan, "raw_scan");
impl_request!(RawBatchScanRequest, raw_batch_scan, "raw_batch_scan");
impl_request!(RawDeleteRangeRequest, raw_delete_range, "raw_delete_range");
impl_request!(RawCasRequest, raw_compare_and_swap, "raw_compare_and_swap");
impl_request!(RawCoprocessorRequest, raw_coprocessor, "raw_coprocessor");

impl_request!(GetRequest, kv_get, "kv_get");
impl_request!(ScanRequest, kv_scan, "kv_scan");
impl_request!(PrewriteRequest, kv_prewrite, "kv_prewrite");
impl_request!(CommitRequest, kv_commit, "kv_commit");
impl_request!(CleanupRequest, kv_cleanup, "kv_cleanup");
impl_request!(BatchGetRequest, kv_batch_get, "kv_batch_get");
impl_request!(BatchRollbackRequest, kv_batch_rollback, "kv_batch_rollback");
impl_request!(
    PessimisticRollbackRequest,
    kv_pessimistic_rollback,
    "kv_pessimistic_rollback"
);
impl_request!(ResolveLockRequest, kv_resolve_lock, "kv_resolve_lock");
impl_request!(ScanLockRequest, kv_scan_lock, "kv_scan_lock");
impl_request!(
    PessimisticLockRequest,
    kv_pessimistic_lock,
    "kv_pessimistic_lock"
);
impl_request!(TxnHeartBeatRequest, kv_txn_heart_beat, "kv_txn_heart_beat");
impl_request!(
    CheckTxnStatusRequest,
    kv_check_txn_status,
    "kv_check_txn_status"
);
impl_request!(
    CheckSecondaryLocksRequest,
    kv_check_secondary_locks,
    "kv_check_secondary_locks_request"
);
impl_request!(GcRequest, kv_gc, "kv_gc");
impl_request!(DeleteRangeRequest, kv_delete_range, "kv_delete_range");
impl_request!(
    UnsafeDestroyRangeRequest,
    unsafe_destroy_range,
    "unsafe_destroy_range"
);
