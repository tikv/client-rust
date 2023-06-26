// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![allow(clippy::useless_conversion)] // To support both prost & rust-protobuf.

use crate::{spawn_unary_success, KvStore};
use derive_new::new;
use futures::{FutureExt, TryFutureExt};
use grpcio::{Environment, Server, ServerBuilder};
use std::sync::Arc;
use tikv_client_proto::{kvrpcpb::*, tikvpb::*};

pub const MOCK_TIKV_PORT: u16 = 50019;

pub fn start_mock_tikv_server() -> Server {
    let env = Arc::new(Environment::new(1));
    let mut server = ServerBuilder::new(env)
        .register_service(create_tikv(MockTikv::new(KvStore::new())))
        .bind("localhost", MOCK_TIKV_PORT)
        .build()
        .unwrap();
    server.start();
    server
}

#[derive(Debug, Clone, new)]
pub struct MockTikv {
    inner: KvStore,
}

impl Tikv for MockTikv {
    fn kv_get(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::GetRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::GetResponse>,
    ) {
        todo!()
    }

    fn kv_scan(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::ScanRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::ScanResponse>,
    ) {
        todo!()
    }

    fn kv_prewrite(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::PrewriteRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::PrewriteResponse>,
    ) {
        todo!()
    }

    fn kv_pessimistic_lock(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::PessimisticLockRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::PessimisticLockResponse>,
    ) {
        todo!()
    }

    fn kv_pessimistic_rollback(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::PessimisticRollbackRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::PessimisticRollbackResponse>,
    ) {
        todo!()
    }

    fn kv_txn_heart_beat(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::TxnHeartBeatRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::TxnHeartBeatResponse>,
    ) {
        todo!()
    }

    fn kv_check_txn_status(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::CheckTxnStatusRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::CheckTxnStatusResponse>,
    ) {
        todo!()
    }

    fn kv_commit(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::CommitRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::CommitResponse>,
    ) {
        todo!()
    }

    fn kv_import(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::ImportRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::ImportResponse>,
    ) {
        todo!()
    }

    fn kv_cleanup(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::CleanupRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::CleanupResponse>,
    ) {
        todo!()
    }

    fn kv_batch_get(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::BatchGetRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::BatchGetResponse>,
    ) {
        todo!()
    }

    fn kv_batch_rollback(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::BatchRollbackRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::BatchRollbackResponse>,
    ) {
        todo!()
    }

    fn kv_scan_lock(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::ScanLockRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::ScanLockResponse>,
    ) {
        todo!()
    }

    fn kv_resolve_lock(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::ResolveLockRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::ResolveLockResponse>,
    ) {
        todo!()
    }

    fn kv_gc(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::GcRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::GcResponse>,
    ) {
        todo!()
    }

    fn kv_delete_range(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::DeleteRangeRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::DeleteRangeResponse>,
    ) {
        todo!()
    }

    fn raw_get(
        &mut self,
        ctx: grpcio::RpcContext,
        req: tikv_client_proto::kvrpcpb::RawGetRequest,
        sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RawGetResponse>,
    ) {
        let mut resp = RawGetResponse::default();
        if let Some(v) = self.inner.raw_get(req.get_key()) {
            resp.set_value(v);
        } else {
            resp.set_not_found(true);
        }
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn raw_batch_get(
        &mut self,
        ctx: grpcio::RpcContext,
        mut req: tikv_client_proto::kvrpcpb::RawBatchGetRequest,
        sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RawBatchGetResponse>,
    ) {
        let mut resp = tikv_client_proto::kvrpcpb::RawBatchGetResponse::default();
        resp.set_pairs(self.inner.raw_batch_get(req.take_keys().into()).into());
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn raw_put(
        &mut self,
        ctx: grpcio::RpcContext,
        req: tikv_client_proto::kvrpcpb::RawPutRequest,
        sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RawPutResponse>,
    ) {
        self.inner
            .raw_put(req.get_key().to_vec(), req.get_value().to_vec());
        let resp = RawPutResponse::default();
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn raw_batch_put(
        &mut self,
        ctx: grpcio::RpcContext,
        mut req: tikv_client_proto::kvrpcpb::RawBatchPutRequest,
        sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RawBatchPutResponse>,
    ) {
        let pairs = req.take_pairs().into();
        self.inner.raw_batch_put(pairs);
        let resp = RawBatchPutResponse::default();
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn raw_delete(
        &mut self,
        ctx: grpcio::RpcContext,
        req: tikv_client_proto::kvrpcpb::RawDeleteRequest,
        sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RawDeleteResponse>,
    ) {
        let key = req.get_key();
        self.inner.raw_delete(key);
        let resp = RawDeleteResponse::default();
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn raw_batch_delete(
        &mut self,
        ctx: grpcio::RpcContext,
        mut req: tikv_client_proto::kvrpcpb::RawBatchDeleteRequest,
        sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RawBatchDeleteResponse>,
    ) {
        let keys = req.take_keys().into();
        self.inner.raw_batch_delete(keys);
        let resp = RawBatchDeleteResponse::default();
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn raw_scan(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::RawScanRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RawScanResponse>,
    ) {
        todo!()
    }

    fn raw_delete_range(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::RawDeleteRangeRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RawDeleteRangeResponse>,
    ) {
        todo!()
    }

    fn raw_batch_scan(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::RawBatchScanRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RawBatchScanResponse>,
    ) {
        todo!()
    }

    fn unsafe_destroy_range(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::UnsafeDestroyRangeRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::UnsafeDestroyRangeResponse>,
    ) {
        todo!()
    }

    fn register_lock_observer(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::RegisterLockObserverRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RegisterLockObserverResponse>,
    ) {
        todo!()
    }

    fn check_lock_observer(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::CheckLockObserverRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::CheckLockObserverResponse>,
    ) {
        todo!()
    }

    fn remove_lock_observer(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::RemoveLockObserverRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::RemoveLockObserverResponse>,
    ) {
        todo!()
    }

    fn physical_scan_lock(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::PhysicalScanLockRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::PhysicalScanLockResponse>,
    ) {
        todo!()
    }

    fn coprocessor(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::coprocessor::Request,
        _sink: grpcio::UnarySink<tikv_client_proto::coprocessor::Response>,
    ) {
        todo!()
    }

    fn coprocessor_stream(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::coprocessor::Request,
        _sink: grpcio::ServerStreamingSink<tikv_client_proto::coprocessor::Response>,
    ) {
        todo!()
    }

    fn batch_coprocessor(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::coprocessor::BatchRequest,
        _sink: grpcio::ServerStreamingSink<tikv_client_proto::coprocessor::BatchResponse>,
    ) {
        todo!()
    }

    fn raft(
        &mut self,
        _ctx: grpcio::RpcContext,
        _stream: grpcio::RequestStream<tikv_client_proto::raft_serverpb::RaftMessage>,
        _sink: grpcio::ClientStreamingSink<tikv_client_proto::raft_serverpb::Done>,
    ) {
        todo!()
    }

    fn batch_raft(
        &mut self,
        _ctx: grpcio::RpcContext,
        _stream: grpcio::RequestStream<tikv_client_proto::tikvpb::BatchRaftMessage>,
        _sink: grpcio::ClientStreamingSink<tikv_client_proto::raft_serverpb::Done>,
    ) {
        todo!()
    }

    fn snapshot(
        &mut self,
        _ctx: grpcio::RpcContext,
        _stream: grpcio::RequestStream<tikv_client_proto::raft_serverpb::SnapshotChunk>,
        _sink: grpcio::ClientStreamingSink<tikv_client_proto::raft_serverpb::Done>,
    ) {
        todo!()
    }

    fn split_region(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::SplitRegionRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::SplitRegionResponse>,
    ) {
        todo!()
    }

    fn read_index(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::ReadIndexRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::ReadIndexResponse>,
    ) {
        todo!()
    }

    fn mvcc_get_by_key(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::MvccGetByKeyRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::MvccGetByKeyResponse>,
    ) {
        todo!()
    }

    fn mvcc_get_by_start_ts(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: tikv_client_proto::kvrpcpb::MvccGetByStartTsRequest,
        _sink: grpcio::UnarySink<tikv_client_proto::kvrpcpb::MvccGetByStartTsResponse>,
    ) {
        todo!()
    }

    fn batch_commands(
        &mut self,
        _ctx: grpcio::RpcContext,
        _stream: grpcio::RequestStream<tikv_client_proto::tikvpb::BatchCommandsRequest>,
        _sink: grpcio::DuplexSink<tikv_client_proto::tikvpb::BatchCommandsResponse>,
    ) {
        todo!()
    }

    fn kv_check_secondary_locks(
        &mut self,
        _: grpcio::RpcContext<'_>,
        _: tikv_client_proto::kvrpcpb::CheckSecondaryLocksRequest,
        _: grpcio::UnarySink<tikv_client_proto::kvrpcpb::CheckSecondaryLocksResponse>,
    ) {
        todo!()
    }

    fn dispatch_mpp_task(
        &mut self,
        _: grpcio::RpcContext<'_>,
        _: tikv_client_proto::mpp::DispatchTaskRequest,
        _: grpcio::UnarySink<tikv_client_proto::mpp::DispatchTaskResponse>,
    ) {
        todo!()
    }

    fn cancel_mpp_task(
        &mut self,
        _: grpcio::RpcContext<'_>,
        _: tikv_client_proto::mpp::CancelTaskRequest,
        _: grpcio::UnarySink<tikv_client_proto::mpp::CancelTaskResponse>,
    ) {
        todo!()
    }

    fn establish_mpp_connection(
        &mut self,
        _: grpcio::RpcContext<'_>,
        _: tikv_client_proto::mpp::EstablishMppConnectionRequest,
        _: grpcio::ServerStreamingSink<tikv_client_proto::mpp::MppDataPacket>,
    ) {
        todo!()
    }

    fn check_leader(
        &mut self,
        _: grpcio::RpcContext<'_>,
        _: tikv_client_proto::kvrpcpb::CheckLeaderRequest,
        _: grpcio::UnarySink<tikv_client_proto::kvrpcpb::CheckLeaderResponse>,
    ) {
        todo!()
    }

    fn raw_get_key_ttl(
        &mut self,
        _: grpcio::RpcContext,
        _: RawGetKeyTtlRequest,
        _: grpcio::UnarySink<RawGetKeyTtlResponse>,
    ) {
        todo!()
    }

    fn raw_compare_and_swap(
        &mut self,
        _: grpcio::RpcContext,
        _: RawCasRequest,
        _: grpcio::UnarySink<RawCasResponse>,
    ) {
        todo!()
    }

    fn raw_coprocessor(
        &mut self,
        _: grpcio::RpcContext,
        _: RawCoprocessorRequest,
        _: grpcio::UnarySink<RawCoprocessorResponse>,
    ) {
        todo!()
    }

    fn get_store_safe_ts(
        &mut self,
        _: grpcio::RpcContext,
        _: StoreSafeTsRequest,
        _: grpcio::UnarySink<StoreSafeTsResponse>,
    ) {
        todo!()
    }

    fn get_lock_wait_info(
        &mut self,
        _: grpcio::RpcContext,
        _: GetLockWaitInfoRequest,
        _: grpcio::UnarySink<GetLockWaitInfoResponse>,
    ) {
        todo!()
    }
}
