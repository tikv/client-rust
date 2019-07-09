// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::{
    fmt,
    sync::{Arc, RwLock},
    time::Duration,
};

use futures::compat::Compat01As03;
use futures::prelude::*;
use grpcio::{CallOption, Environment};
use kvproto::{pdpb, pdpb::PdClient as RpcClient};

use crate::{
    rpc::{
        context::RequestContext,
        pd::{
            context::request_context, leader::LeaderClient, request::Request, Region,
            RegionId, StoreId, Timestamp
        },
        security::SecurityManager,
    },
    Error, ErrorKind, Result,
};

const LEADER_CHANGE_RETRY: usize = 10;

trait PdResponse {
    fn header(&self) -> &pdpb::ResponseHeader;
}

impl PdResponse for pdpb::GetStoreResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.get_header()
    }
}

impl PdResponse for pdpb::GetRegionResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.get_header()
    }
}

impl PdResponse for pdpb::GetAllStoresResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.get_header()
    }
}

pub struct PdClient {
    cluster_id: u64,
    leader: Arc<RwLock<LeaderClient>>,
    timeout: Duration,
}

impl PdClient {
    pub fn connect(
        env: Arc<Environment>,
        endpoints: &[String],
        security_mgr: Arc<SecurityManager>,
        timeout: Duration,
    ) -> Result<PdClient> {
        let leader = LeaderClient::connect(env, endpoints, security_mgr, timeout)?;
        let cluster_id = leader.read().unwrap().cluster_id();

        Ok(PdClient {
            cluster_id,
            leader,
            timeout,
        })
    }

    fn get_leader(&self) -> pdpb::Member {
        self.leader.read().unwrap().members.get_leader().clone()
    }

    pub fn get_region(&self, key: &[u8]) -> impl Future<Output = Result<Region>> {
        let mut req = pd_request!(self.cluster_id, pdpb::GetRegionRequest);
        req.set_region_key(key.to_owned());
        let key = req.get_region_key().to_owned();

        self.execute(request_context(
            "get_region",
            move |cli: &RpcClient, opt: _| {
                cli.get_region_async_opt(&req, opt).map(Compat01As03::new)
            },
        ))
        .and_then(move |mut resp| {
            let region = if resp.has_region() {
                resp.take_region()
            } else {
                return future::ready(Err(Error::region_for_key_not_found(key)));
            };
            let leader = if resp.has_leader() {
                Some(resp.take_leader())
            } else {
                None
            };
            future::ready(Ok(Region::new(region, leader)))
        })
    }

    pub fn get_region_by_id(&self, region_id: u64) -> impl Future<Output = Result<Region>> {
        let mut req = pd_request!(self.cluster_id, pdpb::GetRegionByIdRequest);
        req.set_region_id(region_id);

        self.execute(request_context(
            "get_region_by_id",
            move |cli: &RpcClient, opt: _| {
                cli.get_region_by_id_async_opt(&req, opt)
                    .map(Compat01As03::new)
            },
        ))
        .and_then(move |mut resp| {
            let region = if resp.has_region() {
                resp.take_region()
            } else {
                return future::ready(Err(Error::region_not_found(region_id, None)));
            };
            let leader = if resp.has_leader() {
                Some(resp.take_leader())
            } else {
                None
            };
            future::ready(Ok(Region::new(region, leader)))
        })
    }

    fn execute<Executor, Resp, RpcFuture>(
        &self,
        mut context: RequestContext<Executor>,
    ) -> impl Future<Output = Result<Resp>>
    where
        Executor: FnMut(&RpcClient, CallOption) -> ::grpcio::Result<RpcFuture> + Send + 'static,
        RpcFuture: Future<Output = std::result::Result<Resp, ::grpcio::Error>> + Send + 'static,
        Resp: PdResponse + Send + fmt::Debug + 'static,
    {
        let timeout = self.timeout;
        let mut executor = context.executor();
        let wrapper = move |cli: &RwLock<LeaderClient>| {
            let option = CallOption::default().timeout(timeout);
            let cli = &cli.read().unwrap().client;
            executor(cli, option).unwrap().map(|r| match r {
                Err(e) => Err(ErrorKind::Grpc(e).into()),
                Ok(r) => {
                    {
                        let header = r.header();
                        if header.has_error() {
                            return Err(internal_err!(header.get_error().get_message()));
                        }
                    }
                    Ok(r)
                }
            })
        };
        Request::new(
            wrapper,
            Arc::clone(&self.leader),
            LeaderClient::reconnect,
            LEADER_CHANGE_RETRY,
        )
        .execute()
        .map(move |r| context.done(r))
    }

    pub fn get_all_stores(&self) -> impl Future<Output = Result<Vec<metapb::Store>>> {
        let req = pd_request!(self.cluster_id, pdpb::GetAllStoresRequest);

        self.execute(request_context(
            "get_all_stores",
            move |cli: &RpcClient, opt: _| {
                cli.get_all_stores_async_opt(&req, opt)
                    .map(Compat01As03::new)
            },
        ))
        .map_ok(|mut resp| resp.take_stores().into_iter().map(Into::into).collect())
    }

    pub fn get_store(&self, store_id: StoreId) -> impl Future<Output = Result<metapb::Store>> {
        let mut req = pd_request!(self.cluster_id, pdpb::GetStoreRequest);
        req.set_store_id(store_id);

        self.execute(request_context(
            "get_store",
            move |cli: &RpcClient, opt: _| {
                cli.get_store_async_opt(&req, opt).map(Compat01As03::new)
            },
        ))
        .map_ok(|mut resp| resp.take_store())
    }

    pub fn get_ts(&self) -> impl Future<Output = Result<Timestamp>> {
        self.leader.write().unwrap().get_ts()
    }
}

impl fmt::Debug for PdClient {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("PdClient")
            .field("cluster_id", &self.cluster_id)
            .field("leader", &self.get_leader())
            .field("timeout", &self.timeout)
            .finish()
    }
}
