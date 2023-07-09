// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use log::error;
use log::info;
use log::warn;
use tonic::transport::Channel;
use tonic::IntoRequest;
use tonic::Request;

use super::timestamp::TimestampOracle;
use crate::internal_err;
use crate::proto::pdpb;
use crate::Result;
use crate::SecurityManager;
use crate::Timestamp;

/// A PD cluster.
pub struct Cluster {
    id: u64,
    client: pdpb::pd_client::PdClient<Channel>,
    members: pdpb::GetMembersResponse,
    tso: TimestampOracle,
}

macro_rules! pd_request {
    ($cluster_id:expr, $type:ty) => {{
        let mut request = <$type>::default();
        let mut header = pdpb::RequestHeader::default();
        header.cluster_id = $cluster_id;
        request.header = Some(header);
        request
    }};
}

// These methods make a single attempt to make a request.
impl Cluster {
    pub async fn get_region(
        &mut self,
        key: Vec<u8>,
        timeout: Duration,
    ) -> Result<pdpb::GetRegionResponse> {
        let mut req = pd_request!(self.id, pdpb::GetRegionRequest);
        req.region_key = key.clone();
        req.send(&mut self.client, timeout).await
    }

    pub async fn get_region_by_id(
        &mut self,
        id: u64,
        timeout: Duration,
    ) -> Result<pdpb::GetRegionResponse> {
        let mut req = pd_request!(self.id, pdpb::GetRegionByIdRequest);
        req.region_id = id;
        req.send(&mut self.client, timeout).await
    }

    pub async fn get_store(
        &mut self,
        id: u64,
        timeout: Duration,
    ) -> Result<pdpb::GetStoreResponse> {
        let mut req = pd_request!(self.id, pdpb::GetStoreRequest);
        req.store_id = id;
        req.send(&mut self.client, timeout).await
    }

    pub async fn get_all_stores(
        &mut self,
        timeout: Duration,
    ) -> Result<pdpb::GetAllStoresResponse> {
        let req = pd_request!(self.id, pdpb::GetAllStoresRequest);
        req.send(&mut self.client, timeout).await
    }

    pub async fn get_timestamp(&self) -> Result<Timestamp> {
        self.tso.clone().get_timestamp().await
    }

    pub async fn update_safepoint(
        &mut self,
        safepoint: u64,
        timeout: Duration,
    ) -> Result<pdpb::UpdateGcSafePointResponse> {
        let mut req = pd_request!(self.id, pdpb::UpdateGcSafePointRequest);
        req.safe_point = safepoint;
        req.send(&mut self.client, timeout).await
    }
}

/// An object for connecting and reconnecting to a PD cluster.
pub struct Connection {
    security_mgr: Arc<SecurityManager>,
}

impl Connection {
    pub fn new(security_mgr: Arc<SecurityManager>) -> Connection {
        Connection { security_mgr }
    }

    pub async fn connect_cluster(
        &self,
        endpoints: &[String],
        timeout: Duration,
    ) -> Result<Cluster> {
        let members = self.validate_endpoints(endpoints, timeout).await?;
        let (client, members) = self.try_connect_leader(&members, timeout).await?;
        let id = members.header.as_ref().unwrap().cluster_id;
        let tso = TimestampOracle::new(id, &client)?;
        let cluster = Cluster {
            id,
            client,
            members,
            tso,
        };
        Ok(cluster)
    }

    // Re-establish connection with PD leader in asynchronous fashion.
    pub async fn reconnect(&self, cluster: &mut Cluster, timeout: Duration) -> Result<()> {
        warn!("updating pd client");
        let start = Instant::now();
        let (client, members) = self.try_connect_leader(&cluster.members, timeout).await?;
        let tso = TimestampOracle::new(cluster.id, &client)?;
        *cluster = Cluster {
            id: cluster.id,
            client,
            members,
            tso,
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

            let (_, resp) = match self.connect(ep, timeout).await {
                Ok(resp) => resp,
                // Ignore failed PD node.
                Err(e) => {
                    warn!("PD endpoint {} failed to respond: {:?}", ep, e);
                    continue;
                }
            };

            // Check cluster ID.
            let cid = resp.header.as_ref().unwrap().cluster_id;
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

    async fn connect(
        &self,
        addr: &str,
        _timeout: Duration,
    ) -> Result<(pdpb::pd_client::PdClient<Channel>, pdpb::GetMembersResponse)> {
        let mut client = self
            .security_mgr
            .connect(addr, pdpb::pd_client::PdClient::<Channel>::new)
            .await?;
        let resp: pdpb::GetMembersResponse = client
            .get_members(pdpb::GetMembersRequest::default())
            .await?
            .into_inner();
        Ok((client, resp))
    }

    async fn try_connect(
        &self,
        addr: &str,
        cluster_id: u64,
        timeout: Duration,
    ) -> Result<(pdpb::pd_client::PdClient<Channel>, pdpb::GetMembersResponse)> {
        let (client, r) = self.connect(addr, timeout).await?;
        Connection::validate_cluster_id(addr, &r, cluster_id)?;
        Ok((client, r))
    }

    fn validate_cluster_id(
        addr: &str,
        members: &pdpb::GetMembersResponse,
        cluster_id: u64,
    ) -> Result<()> {
        let new_cluster_id = members.header.as_ref().unwrap().cluster_id;
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
    ) -> Result<(pdpb::pd_client::PdClient<Channel>, pdpb::GetMembersResponse)> {
        let previous_leader = previous.leader.as_ref().unwrap();
        let members = &previous.members;
        let cluster_id = previous.header.as_ref().unwrap().cluster_id;

        let mut resp = None;
        // Try to connect to other members, then the previous leader.
        'outer: for m in members
            .iter()
            .filter(|m| *m != previous_leader)
            .chain(Some(previous_leader))
        {
            for ep in &m.client_urls {
                match self.try_connect(ep.as_str(), cluster_id, timeout).await {
                    Ok((_, r)) => {
                        resp = Some(r);
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
            let leader = resp.leader.as_ref().unwrap();
            for ep in &leader.client_urls {
                let r = self.try_connect(ep.as_str(), cluster_id, timeout).await;
                if r.is_ok() {
                    return r;
                }
            }
        }

        Err(internal_err!("failed to connect to {:?}", members))
    }
}

type GrpcResult<T> = std::result::Result<T, tonic::Status>;

#[async_trait]
trait PdMessage: Sized {
    type Response: PdResponse;

    async fn rpc(
        req: Request<Self>,
        client: &mut pdpb::pd_client::PdClient<Channel>,
    ) -> GrpcResult<Self::Response>;

    async fn send(
        self,
        client: &mut pdpb::pd_client::PdClient<Channel>,
        timeout: Duration,
    ) -> Result<Self::Response> {
        let mut req = self.into_request();
        req.set_timeout(timeout);
        let response = Self::rpc(req, client).await?;

        if let Some(err) = &response.header().error {
            Err(internal_err!(err.message))
        } else {
            Ok(response)
        }
    }
}

#[async_trait]
impl PdMessage for pdpb::GetRegionRequest {
    type Response = pdpb::GetRegionResponse;

    async fn rpc(
        req: Request<Self>,
        client: &mut pdpb::pd_client::PdClient<Channel>,
    ) -> GrpcResult<Self::Response> {
        Ok(client.get_region(req).await?.into_inner())
    }
}

#[async_trait]
impl PdMessage for pdpb::GetRegionByIdRequest {
    type Response = pdpb::GetRegionResponse;

    async fn rpc(
        req: Request<Self>,
        client: &mut pdpb::pd_client::PdClient<Channel>,
    ) -> GrpcResult<Self::Response> {
        Ok(client.get_region_by_id(req).await?.into_inner())
    }
}

#[async_trait]
impl PdMessage for pdpb::GetStoreRequest {
    type Response = pdpb::GetStoreResponse;

    async fn rpc(
        req: Request<Self>,
        client: &mut pdpb::pd_client::PdClient<Channel>,
    ) -> GrpcResult<Self::Response> {
        Ok(client.get_store(req).await?.into_inner())
    }
}

#[async_trait]
impl PdMessage for pdpb::GetAllStoresRequest {
    type Response = pdpb::GetAllStoresResponse;

    async fn rpc(
        req: Request<Self>,
        client: &mut pdpb::pd_client::PdClient<Channel>,
    ) -> GrpcResult<Self::Response> {
        Ok(client.get_all_stores(req).await?.into_inner())
    }
}

#[async_trait]
impl PdMessage for pdpb::UpdateGcSafePointRequest {
    type Response = pdpb::UpdateGcSafePointResponse;

    async fn rpc(
        req: Request<Self>,
        client: &mut pdpb::pd_client::PdClient<Channel>,
    ) -> GrpcResult<Self::Response> {
        Ok(client.update_gc_safe_point(req).await?.into_inner())
    }
}

trait PdResponse {
    fn header(&self) -> &pdpb::ResponseHeader;
}

impl PdResponse for pdpb::GetStoreResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.header.as_ref().unwrap()
    }
}

impl PdResponse for pdpb::GetRegionResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.header.as_ref().unwrap()
    }
}

impl PdResponse for pdpb::GetAllStoresResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.header.as_ref().unwrap()
    }
}

impl PdResponse for pdpb::UpdateGcSafePointResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.header.as_ref().unwrap()
    }
}
