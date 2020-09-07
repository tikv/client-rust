// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use futures::{channel::oneshot, executor::block_on, FutureExt, TryFutureExt};
use grpcio::{ChannelBuilder, EnvBuilder, Environment, ResourceQuota, Server, ServerBuilder};
use io::Read;
use kvproto::{kvrpcpb::*, tikvpb::*};
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

pub const PORT: u16 = 50019;

#[derive(Debug, Clone)]
pub struct MockTikv {
    data: Arc<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>,
}

impl MockTikv {
    fn new() -> MockTikv {
        MockTikv {
            data: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

pub fn start_server() -> Server {
    let env = Arc::new(Environment::new(1));
    let mut server = ServerBuilder::new(env)
        .register_service(create_tikv(MockTikv::new()))
        .bind("localhost", PORT)
        .build()
        .unwrap();
    server.start();
    server
}

impl Tikv for MockTikv {
    fn kv_get(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::GetRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::GetResponse>,
    ) {
        let mut resp = GetResponse::default();
        resp.set_value(vec![2u8, 3u8]);
        let f = sink
            .success(resp)
            .map_err(move |e| panic!("failed to reply {:?}: {:?}", req, e))
            .map(|_| ());
        ctx.spawn(f)
    }

    fn kv_scan(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::ScanRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::ScanResponse>,
    ) {
        todo!()
    }

    fn kv_prewrite(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::PrewriteRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::PrewriteResponse>,
    ) {
        todo!()
    }

    fn kv_pessimistic_lock(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::PessimisticLockRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::PessimisticLockResponse>,
    ) {
        todo!()
    }

    fn kv_pessimistic_rollback(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::PessimisticRollbackRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::PessimisticRollbackResponse>,
    ) {
        todo!()
    }

    fn kv_txn_heart_beat(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::TxnHeartBeatRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::TxnHeartBeatResponse>,
    ) {
        todo!()
    }

    fn kv_check_txn_status(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::CheckTxnStatusRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::CheckTxnStatusResponse>,
    ) {
        todo!()
    }

    fn kv_commit(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::CommitRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::CommitResponse>,
    ) {
        todo!()
    }

    fn kv_import(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::ImportRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::ImportResponse>,
    ) {
        todo!()
    }

    fn kv_cleanup(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::CleanupRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::CleanupResponse>,
    ) {
        todo!()
    }

    fn kv_batch_get(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::BatchGetRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::BatchGetResponse>,
    ) {
        todo!()
    }

    fn kv_batch_rollback(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::BatchRollbackRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::BatchRollbackResponse>,
    ) {
        todo!()
    }

    fn kv_scan_lock(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::ScanLockRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::ScanLockResponse>,
    ) {
        todo!()
    }

    fn kv_resolve_lock(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::ResolveLockRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::ResolveLockResponse>,
    ) {
        todo!()
    }

    fn kv_gc(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::GcRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::GcResponse>,
    ) {
        todo!()
    }

    fn kv_delete_range(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::DeleteRangeRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::DeleteRangeResponse>,
    ) {
        todo!()
    }

    fn raw_get(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawGetRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawGetResponse>,
    ) {
        let mut resp = RawGetResponse::default();
        let data = self.data.read().unwrap();
        let value = data.get(req.get_key()).unwrap_or(&vec![]).to_vec();
        resp.set_value(value);
        let f = sink
            .success(resp)
            .map_err(move |e| panic!("failed to reply {:?}: {:?}", req, e))
            .map(|_| ());
        ctx.spawn(f)
    }

    fn raw_batch_get(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawBatchGetRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawBatchGetResponse>,
    ) {
        let data = self.data.read().unwrap();
        let mut resp = kvproto::kvrpcpb::RawBatchGetResponse::default();
        let mut pairs = vec![];
        for key in req.get_keys() {
            let mut pair = KvPair::new_();
            pair.set_value(data.get(key).unwrap_or(&vec![]).to_vec());
            pair.set_key(key.to_vec());
            pairs.push(pair);
        }
        resp.set_pairs(pairs);
        let f = sink
            .success(resp)
            .map_err(move |e| panic!("failed to reply {:?}: {:?}", req, e))
            .map(|_| ());
        ctx.spawn(f)
    }

    fn raw_put(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawPutRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawPutResponse>,
    ) {
        let mut data = self.data.write().unwrap();
        let key = req.get_key().to_vec();
        let value = req.get_value().to_vec();
        *data.entry(key.clone()).or_default() = value;
        let resp = RawPutResponse::default();
        let f = sink
            .success(resp)
            .map_err(move |e| panic!("failed to reply {:?}: {:?}", req, e))
            .map(|_| ());
        ctx.spawn(f)
    }

    fn raw_batch_put(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawBatchPutRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawBatchPutResponse>,
    ) {
        let pairs = req.get_pairs();
        let mut data = self.data.write().unwrap();
        for pair in pairs {
            *data.entry(pair.get_key().to_vec()).or_default() = pair.get_value().to_vec();
        }
        let resp = RawBatchPutResponse::default();
        let f = sink
            .success(resp)
            .map_err(move |e| panic!("failed to reply {:?}: {:?}", req, e))
            .map(|_| ());
        ctx.spawn(f)
    }

    fn raw_delete(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawDeleteRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawDeleteResponse>,
    ) {
        todo!()
    }

    fn raw_batch_delete(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawBatchDeleteRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawBatchDeleteResponse>,
    ) {
        todo!()
    }

    fn raw_scan(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawScanRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawScanResponse>,
    ) {
        todo!()
    }

    fn raw_delete_range(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawDeleteRangeRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawDeleteRangeResponse>,
    ) {
        todo!()
    }

    fn raw_batch_scan(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawBatchScanRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawBatchScanResponse>,
    ) {
        todo!()
    }

    fn ver_get(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::VerGetRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::VerGetResponse>,
    ) {
        todo!()
    }

    fn ver_batch_get(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::VerBatchGetRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::VerBatchGetResponse>,
    ) {
        todo!()
    }

    fn ver_mut(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::VerMutRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::VerMutResponse>,
    ) {
        todo!()
    }

    fn ver_batch_mut(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::VerBatchMutRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::VerBatchMutResponse>,
    ) {
        todo!()
    }

    fn ver_scan(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::VerScanRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::VerScanResponse>,
    ) {
        todo!()
    }

    fn ver_delete_range(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::VerDeleteRangeRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::VerDeleteRangeResponse>,
    ) {
        todo!()
    }

    fn unsafe_destroy_range(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::UnsafeDestroyRangeRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::UnsafeDestroyRangeResponse>,
    ) {
        todo!()
    }

    fn register_lock_observer(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RegisterLockObserverRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RegisterLockObserverResponse>,
    ) {
        todo!()
    }

    fn check_lock_observer(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::CheckLockObserverRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::CheckLockObserverResponse>,
    ) {
        todo!()
    }

    fn remove_lock_observer(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RemoveLockObserverRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RemoveLockObserverResponse>,
    ) {
        todo!()
    }

    fn physical_scan_lock(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::PhysicalScanLockRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::PhysicalScanLockResponse>,
    ) {
        todo!()
    }

    fn coprocessor(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::coprocessor::Request,
        sink: grpcio::UnarySink<kvproto::coprocessor::Response>,
    ) {
        todo!()
    }

    fn coprocessor_stream(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::coprocessor::Request,
        sink: grpcio::ServerStreamingSink<kvproto::coprocessor::Response>,
    ) {
        todo!()
    }

    fn batch_coprocessor(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::coprocessor::BatchRequest,
        sink: grpcio::ServerStreamingSink<kvproto::coprocessor::BatchResponse>,
    ) {
        todo!()
    }

    fn raft(
        &mut self,
        ctx: grpcio::RpcContext,
        stream: grpcio::RequestStream<kvproto::raft_serverpb::RaftMessage>,
        sink: grpcio::ClientStreamingSink<kvproto::raft_serverpb::Done>,
    ) {
        todo!()
    }

    fn batch_raft(
        &mut self,
        ctx: grpcio::RpcContext,
        stream: grpcio::RequestStream<kvproto::tikvpb::BatchRaftMessage>,
        sink: grpcio::ClientStreamingSink<kvproto::raft_serverpb::Done>,
    ) {
        todo!()
    }

    fn snapshot(
        &mut self,
        ctx: grpcio::RpcContext,
        stream: grpcio::RequestStream<kvproto::raft_serverpb::SnapshotChunk>,
        sink: grpcio::ClientStreamingSink<kvproto::raft_serverpb::Done>,
    ) {
        todo!()
    }

    fn split_region(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::SplitRegionRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::SplitRegionResponse>,
    ) {
        todo!()
    }

    fn read_index(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::ReadIndexRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::ReadIndexResponse>,
    ) {
        todo!()
    }

    fn mvcc_get_by_key(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::MvccGetByKeyRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::MvccGetByKeyResponse>,
    ) {
        todo!()
    }

    fn mvcc_get_by_start_ts(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::MvccGetByStartTsRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::MvccGetByStartTsResponse>,
    ) {
        todo!()
    }

    fn batch_commands(
        &mut self,
        ctx: grpcio::RpcContext,
        stream: grpcio::RequestStream<kvproto::tikvpb::BatchCommandsRequest>,
        sink: grpcio::DuplexSink<kvproto::tikvpb::BatchCommandsResponse>,
    ) {
        todo!()
    }

    // fn kv_check_secondary_locks(
    //     &mut self,
    //     _: grpcio::RpcContext<'_>,
    //     _: kvproto::kvrpcpb::CheckSecondaryLocksRequest,
    //     _: grpcio::UnarySink<kvproto::kvrpcpb::CheckSecondaryLocksResponse>,
    // ) {
    //     todo!()
    // }
}
