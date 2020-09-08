// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use futures::{FutureExt, TryFutureExt};
use grpcio::{Environment, Server, ServerBuilder};
use kvproto::{kvrpcpb::*, tikvpb::*};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

pub const PORT: u16 = 50019;

#[derive(Debug, Clone)]
pub struct MockTikv {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MockTikv {
    fn new() -> MockTikv {
        MockTikv {
            data: Arc::new(RwLock::new(HashMap::new())),
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
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::ScanRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::ScanResponse>,
    ) {
        todo!()
    }

    fn kv_prewrite(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::PrewriteRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::PrewriteResponse>,
    ) {
        todo!()
    }

    fn kv_pessimistic_lock(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::PessimisticLockRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::PessimisticLockResponse>,
    ) {
        todo!()
    }

    fn kv_pessimistic_rollback(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::PessimisticRollbackRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::PessimisticRollbackResponse>,
    ) {
        todo!()
    }

    fn kv_txn_heart_beat(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::TxnHeartBeatRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::TxnHeartBeatResponse>,
    ) {
        todo!()
    }

    fn kv_check_txn_status(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::CheckTxnStatusRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::CheckTxnStatusResponse>,
    ) {
        todo!()
    }

    fn kv_commit(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::CommitRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::CommitResponse>,
    ) {
        todo!()
    }

    fn kv_import(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::ImportRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::ImportResponse>,
    ) {
        todo!()
    }

    fn kv_cleanup(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::CleanupRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::CleanupResponse>,
    ) {
        todo!()
    }

    fn kv_batch_get(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::BatchGetRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::BatchGetResponse>,
    ) {
        todo!()
    }

    fn kv_batch_rollback(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::BatchRollbackRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::BatchRollbackResponse>,
    ) {
        todo!()
    }

    fn kv_scan_lock(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::ScanLockRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::ScanLockResponse>,
    ) {
        todo!()
    }

    fn kv_resolve_lock(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::ResolveLockRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::ResolveLockResponse>,
    ) {
        todo!()
    }

    fn kv_gc(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::GcRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::GcResponse>,
    ) {
        todo!()
    }

    fn kv_delete_range(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::DeleteRangeRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::DeleteRangeResponse>,
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
        let key = req.get_key();
        let mut data = self.data.write().unwrap();
        let res = data.remove(key);
        let mut resp = RawDeleteResponse::default();
        if res.is_none() {
            resp.set_error("Key not exist".to_owned());
        }
        let f = sink
            .success(resp)
            .map_err(move |e| panic!("failed to reply {:?}: {:?}", req, e))
            .map(|_| ());
        ctx.spawn(f)
    }

    fn raw_batch_delete(
        &mut self,
        ctx: grpcio::RpcContext,
        req: kvproto::kvrpcpb::RawBatchDeleteRequest,
        sink: grpcio::UnarySink<kvproto::kvrpcpb::RawBatchDeleteResponse>,
    ) {
        let keys: &[Vec<u8>] = req.get_keys();
        let mut data = self.data.write().unwrap();
        let mut pairs = vec![];
        keys.iter()
            .filter(|&key| !data.contains_key(key))
            .for_each(|key| pairs.push(std::str::from_utf8(key).unwrap()));
        let mut resp = RawBatchDeleteResponse::default();
        if pairs.is_empty() {
            keys.iter().for_each(|key| {
                data.remove(key).unwrap();
            });
        } else {
            resp.set_error(format!("Non-existent keys:[{}]", pairs.join(", ")));
        }
        let f = sink
            .success(resp)
            .map_err(move |e| panic!("failed to reply {:?}: {:?}", req, e))
            .map(|_| ());
        ctx.spawn(f)
    }

    fn raw_scan(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::RawScanRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::RawScanResponse>,
    ) {
        todo!()
    }

    fn raw_delete_range(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::RawDeleteRangeRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::RawDeleteRangeResponse>,
    ) {
        todo!()
    }

    fn raw_batch_scan(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::RawBatchScanRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::RawBatchScanResponse>,
    ) {
        todo!()
    }

    fn ver_get(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::VerGetRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::VerGetResponse>,
    ) {
        todo!()
    }

    fn ver_batch_get(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::VerBatchGetRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::VerBatchGetResponse>,
    ) {
        todo!()
    }

    fn ver_mut(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::VerMutRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::VerMutResponse>,
    ) {
        todo!()
    }

    fn ver_batch_mut(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::VerBatchMutRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::VerBatchMutResponse>,
    ) {
        todo!()
    }

    fn ver_scan(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::VerScanRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::VerScanResponse>,
    ) {
        todo!()
    }

    fn ver_delete_range(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::VerDeleteRangeRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::VerDeleteRangeResponse>,
    ) {
        todo!()
    }

    fn unsafe_destroy_range(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::UnsafeDestroyRangeRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::UnsafeDestroyRangeResponse>,
    ) {
        todo!()
    }

    fn register_lock_observer(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::RegisterLockObserverRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::RegisterLockObserverResponse>,
    ) {
        todo!()
    }

    fn check_lock_observer(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::CheckLockObserverRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::CheckLockObserverResponse>,
    ) {
        todo!()
    }

    fn remove_lock_observer(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::RemoveLockObserverRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::RemoveLockObserverResponse>,
    ) {
        todo!()
    }

    fn physical_scan_lock(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::PhysicalScanLockRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::PhysicalScanLockResponse>,
    ) {
        todo!()
    }

    fn coprocessor(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::coprocessor::Request,
        _sink: grpcio::UnarySink<kvproto::coprocessor::Response>,
    ) {
        todo!()
    }

    fn coprocessor_stream(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::coprocessor::Request,
        _sink: grpcio::ServerStreamingSink<kvproto::coprocessor::Response>,
    ) {
        todo!()
    }

    fn batch_coprocessor(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::coprocessor::BatchRequest,
        _sink: grpcio::ServerStreamingSink<kvproto::coprocessor::BatchResponse>,
    ) {
        todo!()
    }

    fn raft(
        &mut self,
        _ctx: grpcio::RpcContext,
        _stream: grpcio::RequestStream<kvproto::raft_serverpb::RaftMessage>,
        _sink: grpcio::ClientStreamingSink<kvproto::raft_serverpb::Done>,
    ) {
        todo!()
    }

    fn batch_raft(
        &mut self,
        _ctx: grpcio::RpcContext,
        _stream: grpcio::RequestStream<kvproto::tikvpb::BatchRaftMessage>,
        _sink: grpcio::ClientStreamingSink<kvproto::raft_serverpb::Done>,
    ) {
        todo!()
    }

    fn snapshot(
        &mut self,
        _ctx: grpcio::RpcContext,
        _stream: grpcio::RequestStream<kvproto::raft_serverpb::SnapshotChunk>,
        _sink: grpcio::ClientStreamingSink<kvproto::raft_serverpb::Done>,
    ) {
        todo!()
    }

    fn split_region(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::SplitRegionRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::SplitRegionResponse>,
    ) {
        todo!()
    }

    fn read_index(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::ReadIndexRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::ReadIndexResponse>,
    ) {
        todo!()
    }

    fn mvcc_get_by_key(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::MvccGetByKeyRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::MvccGetByKeyResponse>,
    ) {
        todo!()
    }

    fn mvcc_get_by_start_ts(
        &mut self,
        _ctx: grpcio::RpcContext,
        _req: kvproto::kvrpcpb::MvccGetByStartTsRequest,
        _sink: grpcio::UnarySink<kvproto::kvrpcpb::MvccGetByStartTsResponse>,
    ) {
        todo!()
    }

    fn batch_commands(
        &mut self,
        _ctx: grpcio::RpcContext,
        _stream: grpcio::RequestStream<kvproto::tikvpb::BatchCommandsRequest>,
        _sink: grpcio::DuplexSink<kvproto::tikvpb::BatchCommandsResponse>,
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
