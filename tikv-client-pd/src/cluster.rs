// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

// FIXME: Remove this when txn is done.
#![allow(dead_code)]

use crate::timestamp::TimestampOracle;
use futures::{compat::Compat01As03, prelude::*};
use grpcio::{CallOption, Environment};
use kvproto::{metapb, pdpb};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use tikv_client_common::{
    security::SecurityManager, stats::pd_stats, Error, Region, RegionId, Result, StoreId, Timestamp,
};
use futures::prelude::*;
use grpcio::{CallOption, Environment};
use kvproto::{metapb, pdpb};
use tokio::sync::RwLockWriteGuard;

macro_rules! pd_request {
    ($cluster_id:expr, $type:ty) => {{
        let mut request = <$type>::default();
        let mut header = ::kvproto::pdpb::RequestHeader::default();
        header.set_cluster_id($cluster_id);
        request.set_header(header);
        request
    }};
}

/// A PD cluster.
pub struct Cluster {
    pub id: u64,
    pub last_connected: Instant,
    pub(super) client: pdpb::PdClient,
    members: pdpb::GetMembersResponse,
    tso: TimestampOracle,
}

// These methods make a single attempt to make a request.
impl Cluster {
    pub fn get_region(
        &self,
        key: Vec<u8>,
        timeout: Duration,
    ) -> impl Future<Output = Result<Region>> {
        let context = pd_stats("get_region");
        let option = CallOption::default().timeout(timeout);

        let mut req = pd_request!(self.id, pdpb::GetRegionRequest);
        req.set_region_key(key.clone());

        self.client
            .get_region_async_opt(&req, option)
            .unwrap()
            .map(move |r| context.done(r.map_err(|e| e.into())))
            .and_then(move |resp| {
                if resp.get_header().has_error() {
                    return future::ready(Err(internal_err!(resp
                        .get_header()
                        .get_error()
                        .get_message())));
                }
                let region = resp
                    .region
                    .ok_or_else(|| Error::region_for_key_not_found(key));
                let leader = resp.leader;
                future::ready(region.map(move |r| Region::new(r, leader)))
            })
    }

    pub fn get_region_by_id(
        &self,
        id: RegionId,
        timeout: Duration,
    ) -> impl Future<Output = Result<Region>> {
        let context = pd_stats("get_region_by_id");
        let option = CallOption::default().timeout(timeout);

        let mut req = pd_request!(self.id, pdpb::GetRegionByIdRequest);
        req.set_region_id(id);

        self.client
            .get_region_by_id_async_opt(&req, option)
            .unwrap()
            .map(move |r| context.done(r.map_err(|e| e.into())))
            .and_then(move |resp| {
                if resp.get_header().has_error() {
                    return future::ready(Err(internal_err!(resp
                        .get_header()
                        .get_error()
                        .get_message())));
                }
                let region = resp.region.ok_or_else(|| Error::region_not_found(id));
                let leader = resp.leader;
                future::ready(region.map(move |r| Region::new(r, leader)))
            })
    }

    pub fn get_store(
        &self,
        id: StoreId,
        timeout: Duration,
    ) -> impl Future<Output = Result<metapb::Store>> {
        let context = pd_stats("get_store");
        let option = CallOption::default().timeout(timeout);

        let mut req = pd_request!(self.id, pdpb::GetStoreRequest);
        req.set_store_id(id);

        self.client
            .get_store_async_opt(&req, option)
            .unwrap()
            .map(move |r| context.done(r.map_err(|e| e.into())))
            .and_then(|mut resp| {
                if resp.get_header().has_error() {
                    return future::ready(Err(internal_err!(resp
                        .get_header()
                        .get_error()
                        .get_message())));
                }
                future::ready(Ok(resp.take_store()))
            })
    }

    pub fn get_all_stores(
        &self,
        timeout: Duration,
    ) -> impl Future<Output = Result<Vec<metapb::Store>>> {
        let context = pd_stats("get_all_stores");
        let option = CallOption::default().timeout(timeout);

        let req = pd_request!(self.id, pdpb::GetAllStoresRequest);

        self.client
            .get_all_stores_async_opt(&req, option)
            .unwrap()
            .map(move |r| context.done(r.map_err(|e| e.into())))
            .and_then(|mut resp| {
                if resp.get_header().has_error() {
                    return future::ready(Err(internal_err!(resp
                        .get_header()
                        .get_error()
                        .get_message())));
                }
                future::ready(Ok(resp.take_stores().into_iter().map(Into::into).collect()))
            })
    }

    pub fn get_timestamp(&self) -> impl Future<Output = Result<Timestamp>> {
        self.tso.clone().get_timestamp()
    }
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
        let (client, members) = self.try_connect_leader(&members, timeout).await?;

        let id = members.get_header().get_cluster_id();
        let tso = TimestampOracle::new(id, &client)?;
        let cluster = Cluster {
            id,
            last_connected: Instant::now(),
            members,
            client,
            tso,
        };
        Ok(cluster)
    }

    // Re-establish connection with PD leader in asynchronous fashion.
    pub async fn reconnect(
        &self,
        mut cluster_guard: RwLockWriteGuard<'_, Cluster>,
        timeout: Duration,
    ) -> Result<()> {
        warn!("updating pd client");
        let start = Instant::now();
        let (client, members) = self
            .try_connect_leader(&cluster_guard.members, timeout)
            .await?;
        let tso = TimestampOracle::new(cluster_guard.id, &client)?;
        let last_connected = Instant::now();
        let cluster = Cluster {
            id: cluster_guard.id,
            last_connected,
            client,
            members,
            tso,
        };

        warn!("updating PD client done, spent {:?}", start.elapsed());
        *cluster_guard = cluster;
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
                    error!("PD endpoint {} failed to respond: {:?}", ep, e);
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

    async fn connect(
        &self,
        addr: &str,
        timeout: Duration,
    ) -> Result<(pdpb::PdClient, pdpb::GetMembersResponse)> {
        let client = self
            .security_mgr
            .connect(self.env.clone(), addr, pdpb::PdClient::new)?;
        let option = CallOption::default().timeout(timeout);
        let resp = client
            .get_members_async_opt(&pdpb::GetMembersRequest::default(), option)
            .map_err(Error::from)?
            .await?;
        Ok((client, resp))
    }

    async fn try_connect(
        &self,
        addr: &str,
        cluster_id: u64,
        timeout: Duration,
    ) -> Result<(pdpb::PdClient, pdpb::GetMembersResponse)> {
        let (client, r) = self.connect(addr, timeout).await?;
        Connection::validate_cluster_id(addr, &r, cluster_id)?;
        Ok((client, r))
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
    ) -> Result<(pdpb::PdClient, pdpb::GetMembersResponse)> {
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
