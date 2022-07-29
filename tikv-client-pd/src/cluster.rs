// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{timestamp::TimestampOracle, Error, Result, SecurityManager};
use async_trait::async_trait;
use grpcio::{CallOption, Environment, RpcStatusCode};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use tikv_client_common::internal_err;
use tikv_client_proto::{
    keyspacepb,
    pdpb::{self, Timestamp},
};

/// A PD cluster.
pub struct Cluster {
    id: u64,
    client: pdpb::PdClient,
    members: pdpb::GetMembersResponse,
    tso: TimestampOracle,
    keyspace_mgr: Option<keyspacepb::KeyspaceClient>,
}

macro_rules! pd_request {
    ($cluster_id:expr, $type:ty) => {{
        let mut request = <$type>::default();
        let mut header = ::tikv_client_proto::pdpb::RequestHeader::default();
        header.set_cluster_id($cluster_id);
        request.set_header(header);
        request
    }};
}

// These methods make a single attempt to make a request.
impl Cluster {
    pub async fn get_region(
        &self,
        key: Vec<u8>,
        timeout: Duration,
    ) -> Result<pdpb::GetRegionResponse> {
        let mut req = pd_request!(self.id, pdpb::GetRegionRequest);
        req.set_region_key(key.clone());
        req.send(&self.client, timeout).await
    }

    pub async fn get_region_by_id(
        &self,
        id: u64,
        timeout: Duration,
    ) -> Result<pdpb::GetRegionResponse> {
        let mut req = pd_request!(self.id, pdpb::GetRegionByIdRequest);
        req.set_region_id(id);
        req.send(&self.client, timeout).await
    }

    pub async fn get_store(&self, id: u64, timeout: Duration) -> Result<pdpb::GetStoreResponse> {
        let mut req = pd_request!(self.id, pdpb::GetStoreRequest);
        req.set_store_id(id);
        req.send(&self.client, timeout).await
    }

    pub async fn get_all_stores(&self, timeout: Duration) -> Result<pdpb::GetAllStoresResponse> {
        let req = pd_request!(self.id, pdpb::GetAllStoresRequest);
        req.send(&self.client, timeout).await
    }

    pub async fn get_timestamp(&self) -> Result<Timestamp> {
        self.tso.clone().get_timestamp().await
    }

    pub async fn update_safepoint(
        &self,
        safepoint: u64,
        timeout: Duration,
    ) -> Result<pdpb::UpdateGcSafePointResponse> {
        let mut req = pd_request!(self.id, pdpb::UpdateGcSafePointRequest);
        req.set_safe_point(safepoint);
        req.send(&self.client, timeout).await
    }

    pub async fn load_keyspace(
        &self,
        name: String,
        timeout: Duration,
    ) -> Result<keyspacepb::LoadKeyspaceResponse> {
        let mut req = pd_request!(self.id, keyspacepb::LoadKeyspaceRequest);
        let mgr = self
            .keyspace_mgr
            .as_ref()
            .ok_or(Error::KeyspaceUnsupported)?;
        req.set_name(name);
        req.send(mgr, timeout).await
    }
}

struct ConnectResponse {
    client: pdpb::PdClient,
    members: pdpb::GetMembersResponse,
    keyspace_mgr: Option<keyspacepb::KeyspaceClient>,
}

/// An object for connecting and reconnecting to a PD cluster.
pub struct Connection {
    env: Arc<Environment>,
    security_mgr: Arc<SecurityManager>,
}

impl Connection {
    pub fn new(env: Arc<Environment>, security_mgr: Arc<SecurityManager>) -> Connection {
        Connection { env, security_mgr }
    }

    pub async fn connect_cluster(
        &self,
        endpoints: &[String],
        timeout: Duration,
    ) -> Result<Cluster> {
        let members = self.validate_endpoints(endpoints, timeout).await?;
        let ConnectResponse {
            client,
            members,
            keyspace_mgr,
        } = self.try_connect_leader(&members, timeout).await?;
        let id = members.get_header().get_cluster_id();
        let tso = TimestampOracle::new(id, &client)?;
        let cluster = Cluster {
            id,
            client,
            members,
            tso,
            keyspace_mgr,
        };
        Ok(cluster)
    }

    // Re-establish connection with PD leader in asynchronous fashion.
    pub async fn reconnect(&self, cluster: &mut Cluster, timeout: Duration) -> Result<()> {
        warn!("updating pd client");
        let start = Instant::now();
        let ConnectResponse {
            client,
            members,
            keyspace_mgr,
        } = self.try_connect_leader(&cluster.members, timeout).await?;
        let tso = TimestampOracle::new(cluster.id, &client)?;
        *cluster = Cluster {
            id: cluster.id,
            client,
            members,
            tso,
            keyspace_mgr,
        };

        info!("updating PD client done, spent {:?}", start.elapsed());
        Ok(())
    }

    async fn validate_endpoints(
        &self,
        endpoints: &[String],
        timeout: Duration,
    ) -> Result<pdpb::GetMembersResponse> {
        let mut endpoints_set = HashSet::with_capacity(endpoints.len());

        let mut members = None;
        let mut cluster_id = None;
        for ep in endpoints {
            if !endpoints_set.insert(ep) {
                return Err(internal_err!("duplicated PD endpoint {}", ep));
            }

            let resp = match self.connect(ep, timeout).await {
                Ok(ConnectResponse { members, .. }) => members,
                // Ignore failed PD node.
                Err(e) => {
                    warn!("PD endpoint {} failed to respond: {:?}", ep, e);
                    continue;
                }
            };

            // Check cluster ID.
            let cid = resp.get_header().get_cluster_id();
            if let Some(sample) = cluster_id {
                if sample != cid {
                    return Err(internal_err!(
                        "PD response cluster_id mismatch, want {}, got {}",
                        sample,
                        cid
                    ));
                }
            } else {
                cluster_id = Some(cid);
            }
            // TODO: check all fields later?

            if members.is_none() {
                members = Some(resp);
            }
        }

        match members {
            Some(members) => {
                info!("All PD endpoints are consistent: {:?}", endpoints);
                Ok(members)
            }
            _ => Err(internal_err!("PD cluster failed to respond")),
        }
    }

    async fn connect(&self, addr: &str, timeout: Duration) -> Result<ConnectResponse> {
        let client = self
            .security_mgr
            .connect(self.env.clone(), addr, pdpb::PdClient::new)?;

        let keyspace_mgr = self.connect_keyspace_mgr(addr)?;

        let option = CallOption::default().timeout(timeout);
        let members = client
            .get_members_async_opt(&pdpb::GetMembersRequest::default(), option)
            .map_err(Error::from)?
            .await?;

        Ok(ConnectResponse {
            client,
            members,
            keyspace_mgr,
        })
    }

    fn connect_keyspace_mgr(&self, addr: &str) -> Result<Option<keyspacepb::KeyspaceClient>> {
        let result =
            self.security_mgr
                .connect(self.env.clone(), addr, keyspacepb::KeyspaceClient::new);
        match result {
            Ok(mgr) => Ok(Some(mgr)),
            Err(Error::Grpc(grpcio::Error::RpcFailure(status)))
                if status.code() == RpcStatusCode::UNIMPLEMENTED =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    async fn try_connect(
        &self,
        addr: &str,
        cluster_id: u64,
        timeout: Duration,
    ) -> Result<ConnectResponse> {
        let connect_resp = self.connect(addr, timeout).await?;
        Connection::validate_cluster_id(addr, &connect_resp.members, cluster_id)?;
        Ok(connect_resp)
    }

    fn validate_cluster_id(
        addr: &str,
        members: &pdpb::GetMembersResponse,
        cluster_id: u64,
    ) -> Result<()> {
        let new_cluster_id = members.get_header().get_cluster_id();
        if new_cluster_id != cluster_id {
            Err(internal_err!(
                "{} no longer belongs to cluster {}, it is in {}",
                addr,
                cluster_id,
                new_cluster_id
            ))
        } else {
            Ok(())
        }
    }

    async fn try_connect_leader(
        &self,
        previous: &pdpb::GetMembersResponse,
        timeout: Duration,
    ) -> Result<ConnectResponse> {
        let previous_leader = previous.get_leader();
        let members = previous.get_members();
        let cluster_id = previous.get_header().get_cluster_id();

        let mut resp = None;
        // Try to connect to other members, then the previous leader.
        'outer: for m in members
            .iter()
            .filter(|m| *m != previous_leader)
            .chain(Some(previous_leader))
        {
            for ep in m.get_client_urls() {
                match self.try_connect(ep.as_str(), cluster_id, timeout).await {
                    Ok(ConnectResponse { members, .. }) => {
                        resp = Some(members);
                        break 'outer;
                    }
                    Err(e) => {
                        error!("failed to connect to {}, {:?}", ep, e);
                        continue;
                    }
                }
            }
        }

        // Then try to connect the PD cluster leader.
        if let Some(resp) = resp {
            let leader = resp.get_leader();
            for ep in leader.get_client_urls() {
                let r = self.try_connect(ep.as_str(), cluster_id, timeout).await;
                if r.is_ok() {
                    return r;
                }
            }
        }

        Err(internal_err!("failed to connect to {:?}", members))
    }
}

type GrpcResult<T> = std::result::Result<T, grpcio::Error>;

#[async_trait]
trait PdMessage {
    type Response: PdResponse;
    type Client: Send + Sync;

    async fn rpc(&self, client: &Self::Client, opt: CallOption) -> GrpcResult<Self::Response>;

    async fn send(&self, client: &Self::Client, timeout: Duration) -> Result<Self::Response> {
        let option = CallOption::default().timeout(timeout);
        let response = self.rpc(client, option).await?;

        if response.header().has_error() {
            Err(internal_err!(response.header().get_error().get_message()))
        } else {
            Ok(response)
        }
    }
}

#[async_trait]
impl PdMessage for pdpb::GetRegionRequest {
    type Response = pdpb::GetRegionResponse;
    type Client = pdpb::PdClient;

    async fn rpc(&self, client: &pdpb::PdClient, opt: CallOption) -> GrpcResult<Self::Response> {
        client.get_region_async_opt(self, opt)?.await
    }
}

#[async_trait]
impl PdMessage for pdpb::GetRegionByIdRequest {
    type Response = pdpb::GetRegionResponse;
    type Client = pdpb::PdClient;

    async fn rpc(&self, client: &pdpb::PdClient, opt: CallOption) -> GrpcResult<Self::Response> {
        client.get_region_by_id_async_opt(self, opt)?.await
    }
}

#[async_trait]
impl PdMessage for pdpb::GetStoreRequest {
    type Response = pdpb::GetStoreResponse;
    type Client = pdpb::PdClient;

    async fn rpc(&self, client: &pdpb::PdClient, opt: CallOption) -> GrpcResult<Self::Response> {
        client.get_store_async_opt(self, opt)?.await
    }
}

#[async_trait]
impl PdMessage for pdpb::GetAllStoresRequest {
    type Response = pdpb::GetAllStoresResponse;
    type Client = pdpb::PdClient;

    async fn rpc(&self, client: &pdpb::PdClient, opt: CallOption) -> GrpcResult<Self::Response> {
        client.get_all_stores_async_opt(self, opt)?.await
    }
}

#[async_trait]
impl PdMessage for pdpb::UpdateGcSafePointRequest {
    type Response = pdpb::UpdateGcSafePointResponse;
    type Client = pdpb::PdClient;

    async fn rpc(&self, client: &pdpb::PdClient, opt: CallOption) -> GrpcResult<Self::Response> {
        client.update_gc_safe_point_async_opt(self, opt)?.await
    }
}

#[async_trait]
impl PdMessage for keyspacepb::LoadKeyspaceRequest {
    type Response = keyspacepb::LoadKeyspaceResponse;
    type Client = keyspacepb::KeyspaceClient;

    async fn rpc(
        &self,
        client: &keyspacepb::KeyspaceClient,
        opt: CallOption,
    ) -> GrpcResult<Self::Response> {
        client.load_keyspace_async_opt(self, opt)?.await
    }
}

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

impl PdResponse for pdpb::UpdateGcSafePointResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.get_header()
    }
}

impl PdResponse for keyspacepb::LoadKeyspaceResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.get_header()
    }
}
