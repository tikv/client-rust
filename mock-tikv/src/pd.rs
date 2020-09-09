// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{spawn_unary_success, MOCK_TIKV_PORT};
use futures::{FutureExt, StreamExt, TryFutureExt};
use grpcio::{Environment, Server, ServerBuilder, WriteFlags};
use kvproto::pdpb::*;
use std::sync::Arc;

pub const MOCK_PD_PORT: u16 = 50021;
/// This is mock pd server, used with mock tikv server.
#[derive(Debug, Clone)]
pub struct MockPd {
    ts: i64,
}

impl MockPd {
    fn new() -> MockPd {
        MockPd { ts: 0 }
    }

    fn region() -> kvproto::metapb::Region {
        let mut meta_region = kvproto::metapb::Region::default();
        meta_region.set_end_key(vec![0xff; 20]);
        meta_region.set_start_key(vec![0x00]);
        meta_region.set_id(0);
        meta_region.set_peers(vec![Self::leader()]);
        meta_region
    }

    fn leader() -> kvproto::metapb::Peer {
        kvproto::metapb::Peer::default()
    }

    fn store() -> kvproto::metapb::Store {
        let mut store = kvproto::metapb::Store::default();
        store.set_address(format!("localhost:{}", MOCK_TIKV_PORT));
        // TODO: start_timestamp?
        store
    }
}

pub fn start_mock_pd_server() -> Server {
    let env = Arc::new(Environment::new(1));
    let mut server = ServerBuilder::new(env)
        .register_service(create_pd(MockPd::new()))
        .bind("localhost", MOCK_PD_PORT)
        .build()
        .unwrap();
    server.start();
    server
}

impl Pd for MockPd {
    fn get_members(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: GetMembersRequest,
        sink: ::grpcio::UnarySink<GetMembersResponse>,
    ) {
        let mut resp = GetMembersResponse::default();
        resp.set_header(ResponseHeader::default());
        let mut member = Member::default();
        member.set_name("mock tikv".to_owned());
        member.set_member_id(0);
        member.set_client_urls(vec![format!("localhost:{}", MOCK_PD_PORT)]);
        // member.set_peer_urls(vec![format!("localhost:{}", MOCK_PD_PORT)]);
        resp.set_members(vec![member.clone()]);
        resp.set_leader(member);
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn tso(
        &mut self,
        ctx: ::grpcio::RpcContext,
        stream: ::grpcio::RequestStream<TsoRequest>,
        sink: ::grpcio::DuplexSink<TsoResponse>,
    ) {
        let f = stream
            .map(|_| {
                let mut resp = TsoResponse::default();
                // TODO: make ts monotonic
                resp.set_timestamp(Timestamp::default());
                Ok((resp, WriteFlags::default()))
            })
            .forward(sink)
            .map(|_| ());
        ctx.spawn(f);
    }

    fn bootstrap(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: BootstrapRequest,
        _sink: ::grpcio::UnarySink<BootstrapResponse>,
    ) {
        todo!()
    }

    fn is_bootstrapped(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: IsBootstrappedRequest,
        _sink: ::grpcio::UnarySink<IsBootstrappedResponse>,
    ) {
        todo!()
    }

    fn alloc_id(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: AllocIdRequest,
        _sink: ::grpcio::UnarySink<AllocIdResponse>,
    ) {
        todo!()
    }

    fn get_store(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: GetStoreRequest,
        sink: ::grpcio::UnarySink<GetStoreResponse>,
    ) {
        let mut resp = GetStoreResponse::default();
        resp.set_store(Self::store());
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn put_store(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: PutStoreRequest,
        _sink: ::grpcio::UnarySink<PutStoreResponse>,
    ) {
        todo!()
    }

    fn get_all_stores(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: GetAllStoresRequest,
        _sink: ::grpcio::UnarySink<GetAllStoresResponse>,
    ) {
        todo!()
    }

    fn store_heartbeat(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: StoreHeartbeatRequest,
        _sink: ::grpcio::UnarySink<StoreHeartbeatResponse>,
    ) {
        todo!()
    }

    fn region_heartbeat(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _stream: ::grpcio::RequestStream<RegionHeartbeatRequest>,
        _sink: ::grpcio::DuplexSink<RegionHeartbeatResponse>,
    ) {
        todo!()
    }

    fn get_region(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: GetRegionRequest,
        sink: ::grpcio::UnarySink<GetRegionResponse>,
    ) {
        let mut resp = GetRegionResponse::default();
        resp.set_region(Self::region());
        resp.set_leader(Self::leader());
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn get_prev_region(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: GetRegionRequest,
        _sink: ::grpcio::UnarySink<GetRegionResponse>,
    ) {
        todo!()
    }

    fn get_region_by_id(
        &mut self,
        ctx: ::grpcio::RpcContext,
        req: GetRegionByIdRequest,
        sink: ::grpcio::UnarySink<GetRegionResponse>,
    ) {
        let mut resp = GetRegionResponse::default();
        resp.set_region(Self::region());
        resp.set_leader(Self::leader());
        spawn_unary_success!(ctx, req, resp, sink);
    }

    fn scan_regions(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: ScanRegionsRequest,
        _sink: ::grpcio::UnarySink<ScanRegionsResponse>,
    ) {
        todo!()
    }

    fn ask_split(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: AskSplitRequest,
        _sink: ::grpcio::UnarySink<AskSplitResponse>,
    ) {
        todo!()
    }

    fn report_split(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: ReportSplitRequest,
        _sink: ::grpcio::UnarySink<ReportSplitResponse>,
    ) {
        todo!()
    }

    fn ask_batch_split(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: AskBatchSplitRequest,
        _sink: ::grpcio::UnarySink<AskBatchSplitResponse>,
    ) {
        todo!()
    }

    fn report_batch_split(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: ReportBatchSplitRequest,
        _sink: ::grpcio::UnarySink<ReportBatchSplitResponse>,
    ) {
        todo!()
    }

    fn get_cluster_config(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: GetClusterConfigRequest,
        _sink: ::grpcio::UnarySink<GetClusterConfigResponse>,
    ) {
        todo!()
    }

    fn put_cluster_config(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: PutClusterConfigRequest,
        _sink: ::grpcio::UnarySink<PutClusterConfigResponse>,
    ) {
        todo!()
    }

    fn scatter_region(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: ScatterRegionRequest,
        _sink: ::grpcio::UnarySink<ScatterRegionResponse>,
    ) {
        todo!()
    }

    fn get_gc_safe_point(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: GetGcSafePointRequest,
        _sink: ::grpcio::UnarySink<GetGcSafePointResponse>,
    ) {
        todo!()
    }

    fn update_gc_safe_point(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: UpdateGcSafePointRequest,
        _sink: ::grpcio::UnarySink<UpdateGcSafePointResponse>,
    ) {
        todo!()
    }

    fn update_service_gc_safe_point(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: UpdateServiceGcSafePointRequest,
        _sink: ::grpcio::UnarySink<UpdateServiceGcSafePointResponse>,
    ) {
        todo!()
    }

    fn sync_regions(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _stream: ::grpcio::RequestStream<SyncRegionRequest>,
        _sink: ::grpcio::DuplexSink<SyncRegionResponse>,
    ) {
        todo!()
    }

    fn get_operator(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: GetOperatorRequest,
        _sink: ::grpcio::UnarySink<GetOperatorResponse>,
    ) {
        todo!()
    }
}
