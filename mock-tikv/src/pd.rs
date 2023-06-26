// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{spawn_unary_success, MOCK_TIKV_PORT};
use futures::{FutureExt, StreamExt, TryFutureExt};
use grpcio::{Environment, Server, ServerBuilder, WriteFlags};
use std::sync::Arc;
use tikv_client_proto::pdpb::*;

pub const MOCK_PD_PORT: u16 = 50021;
/// This is mock pd server, used with mock tikv server.
#[derive(Debug, Clone)]
pub struct MockPd {}

impl MockPd {
    fn new() -> MockPd {
        MockPd {}
    }

    fn region() -> tikv_client_proto::metapb::Region {
        tikv_client_proto::metapb::Region {
            start_key: vec![],
            end_key: vec![],
            peers: vec![Self::leader()].into(),
            ..Default::default()
        }
    }

    fn leader() -> tikv_client_proto::metapb::Peer {
        tikv_client_proto::metapb::Peer::default()
    }

    fn store() -> tikv_client_proto::metapb::Store {
        // TODO: start_timestamp?
        tikv_client_proto::metapb::Store {
            address: format!("localhost:{MOCK_TIKV_PORT}"),
            ..Default::default()
        }
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
        let member = Member {
            name: "mock tikv".to_owned(),
            client_urls: vec![format!("localhost:{MOCK_PD_PORT}")].into(),
            ..Default::default()
        };
        let resp = GetMembersResponse {
            members: vec![member.clone()].into(),
            leader: Some(member).into(),
            ..Default::default()
        };
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
                let resp = TsoResponse::default();
                // TODO: make ts monotonic
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
        let resp = GetStoreResponse {
            store: Some(Self::store()).into(),
            ..Default::default()
        };
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
        let resp = GetRegionResponse {
            region: Some(Self::region()).into(),
            leader: Some(Self::leader()).into(),
            ..Default::default()
        };
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
        let resp = GetRegionResponse {
            region: Some(Self::region()).into(),
            leader: Some(Self::leader()).into(),
            ..Default::default()
        };
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

    fn sync_max_ts(
        &mut self,
        _ctx: ::grpcio::RpcContext,
        _req: SyncMaxTsRequest,
        _sink: ::grpcio::UnarySink<SyncMaxTsResponse>,
    ) {
        todo!()
    }

    fn split_regions(
        &mut self,
        _: ::grpcio::RpcContext<'_>,
        _: SplitRegionsRequest,
        _: ::grpcio::UnarySink<SplitRegionsResponse>,
    ) {
        todo!()
    }

    fn get_dc_location_info(
        &mut self,
        _: grpcio::RpcContext,
        _: GetDcLocationInfoRequest,
        _: grpcio::UnarySink<GetDcLocationInfoResponse>,
    ) {
        todo!()
    }
}
