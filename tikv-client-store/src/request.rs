// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{Error, Result};
use async_trait::async_trait;
use grpcio::CallOption;
use std::any::Any;
use tikv_client_common::internal_err;
use tikv_client_proto::{
    kvrpcpb,
    tikvpb::{
        batch_commands_request::{self, request::Cmd::*},
        batch_commands_response, TikvClient,
    },
};

#[async_trait]
pub trait Request: Any + Sync + Send + 'static {
    async fn dispatch(
        &self,
        client: &TikvClient,
        options: CallOption,
    ) -> Result<Box<dyn Any + Send>>;
    fn label(&self) -> &'static str;
    fn as_any(&self) -> &(dyn Any + Send);
    fn set_context(&mut self, context: kvrpcpb::Context);
    fn to_batch_request(&self) -> batch_commands_request::Request {
        batch_commands_request::Request { cmd: None }
    }
    fn support_batch(&self) -> bool {
        false
    }
}

macro_rules! impl_request {
    ($name: ident, $fun: ident, $label: literal, $cmd: ident) => {
        #[async_trait]
        impl Request for kvrpcpb::$name {
            async fn dispatch(
                &self,
                client: &TikvClient,
                options: CallOption,
            ) -> Result<Box<dyn Any + Send>> {
                client
                    .$fun(self, options)?
                    .await
                    .map(|r| Box::new(r) as Box<dyn Any + Send>)
                    .map_err(Error::Grpc)
            }

            fn label(&self) -> &'static str {
                $label
            }

            fn as_any(&self) -> &(dyn Any + Send) {
                self
            }

            fn set_context(&mut self, context: kvrpcpb::Context) {
                kvrpcpb::$name::set_context(self, context)
            }

            fn to_batch_request(&self) -> batch_commands_request::Request {
                let req = batch_commands_request::Request {
                    cmd: Some($cmd(self.clone())),
                };
                req
            }

            fn support_batch(&self) -> bool {
                true
            }
        }
    };
}

impl_request!(RawGetRequest, raw_get_async_opt, "raw_get", RawGet);
impl_request!(
    RawBatchGetRequest,
    raw_batch_get_async_opt,
    "raw_batch_get",
    RawBatchGet
);
impl_request!(RawPutRequest, raw_put_async_opt, "raw_put", RawPut);
impl_request!(
    RawBatchPutRequest,
    raw_batch_put_async_opt,
    "raw_batch_put",
    RawBatchPut
);
impl_request!(
    RawDeleteRequest,
    raw_delete_async_opt,
    "raw_delete",
    RawDelete
);
impl_request!(
    RawBatchDeleteRequest,
    raw_batch_delete_async_opt,
    "raw_batch_delete",
    RawBatchDelete
);
impl_request!(RawScanRequest, raw_scan_async_opt, "raw_scan", RawScan);
impl_request!(
    RawBatchScanRequest,
    raw_batch_scan_async_opt,
    "raw_batch_scan",
    RawBatchScan
);
impl_request!(
    RawDeleteRangeRequest,
    raw_delete_range_async_opt,
    "raw_delete_range",
    RawDeleteRange
);

impl_request!(
    RawCoprocessorRequest,
    raw_coprocessor_async_opt,
    "raw_coprocessor",
    RawCoprocessor
);

impl_request!(GetRequest, kv_get_async_opt, "kv_get", Get);
impl_request!(ScanRequest, kv_scan_async_opt, "kv_scan", Scan);
impl_request!(
    PrewriteRequest,
    kv_prewrite_async_opt,
    "kv_prewrite",
    Prewrite
);
impl_request!(CommitRequest, kv_commit_async_opt, "kv_commit", Commit);
impl_request!(CleanupRequest, kv_cleanup_async_opt, "kv_cleanup", Cleanup);
impl_request!(
    BatchGetRequest,
    kv_batch_get_async_opt,
    "kv_batch_get",
    BatchGet
);
impl_request!(
    BatchRollbackRequest,
    kv_batch_rollback_async_opt,
    "kv_batch_rollback",
    BatchRollback
);
impl_request!(
    PessimisticRollbackRequest,
    kv_pessimistic_rollback_async_opt,
    "kv_pessimistic_rollback",
    PessimisticRollback
);
impl_request!(
    ResolveLockRequest,
    kv_resolve_lock_async_opt,
    "kv_resolve_lock",
    ResolveLock
);
impl_request!(
    ScanLockRequest,
    kv_scan_lock_async_opt,
    "kv_scan_lock",
    ScanLock
);
impl_request!(
    PessimisticLockRequest,
    kv_pessimistic_lock_async_opt,
    "kv_pessimistic_lock",
    PessimisticLock
);
impl_request!(
    TxnHeartBeatRequest,
    kv_txn_heart_beat_async_opt,
    "kv_txn_heart_beat",
    TxnHeartBeat
);
impl_request!(
    CheckTxnStatusRequest,
    kv_check_txn_status_async_opt,
    "kv_check_txn_status",
    CheckTxnStatus
);
impl_request!(
    CheckSecondaryLocksRequest,
    kv_check_secondary_locks_async_opt,
    "kv_check_secondary_locks_request",
    CheckSecondaryLocks
);
impl_request!(GcRequest, kv_gc_async_opt, "kv_gc", Gc);
impl_request!(
    DeleteRangeRequest,
    kv_delete_range_async_opt,
    "kv_delete_range",
    DeleteRange
);

#[async_trait]
impl Request for kvrpcpb::RawCasRequest {
    async fn dispatch(
        &self,
        client: &TikvClient,
        options: CallOption,
    ) -> Result<Box<dyn Any + Send>> {
        client
            .raw_compare_and_swap_async_opt(self, options)?
            .await
            .map(|r| Box::new(r) as Box<dyn Any + Send>)
            .map_err(Error::Grpc)
    }
    fn label(&self) -> &'static str {
        "raw_compare_and_swap"
    }
    fn as_any(&self) -> &(dyn Any + Send) {
        self
    }
    fn set_context(&mut self, _: tikv_client_proto::kvrpcpb::Context) {
        todo!()
    }
    fn to_batch_request(&self) -> batch_commands_request::Request {
        batch_commands_request::Request { cmd: None }
    }
}

pub fn from_batch_commands_resp(
    resp: batch_commands_response::Response,
) -> Result<Box<dyn Any + Send>> {
    match resp.cmd {
        Some(batch_commands_response::response::Cmd::Get(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::Scan(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::Prewrite(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::Commit(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::Import(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::Cleanup(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::BatchGet(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::BatchRollback(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::ScanLock(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::ResolveLock(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::Gc(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::DeleteRange(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawGet(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawBatchGet(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawPut(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawBatchPut(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawDelete(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawBatchDelete(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawScan(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawDeleteRange(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawBatchScan(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::Coprocessor(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::PessimisticLock(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::PessimisticRollback(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::CheckTxnStatus(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::TxnHeartBeat(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::CheckSecondaryLocks(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        Some(batch_commands_response::response::Cmd::RawCoprocessor(cmd)) => {
            Ok(Box::new(cmd) as Box<dyn Any + Send>)
        }
        _ => Err(internal_err!("batch_commands_resp.cmd is None".to_owned())),
    }
}
