// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::{
    collections::HashSet,
    fmt,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use futures::compat::Compat01As03;
use futures::future::BoxFuture;
use futures::prelude::*;
use grpcio::{CallOption, Environment};
use kvproto::{metapb, pdpb};

use crate::{
    rpc::{
        pd::{
            context::request_context, request::retry_request, timestamp::PdReactor, Region,
            RegionId, StoreId, Timestamp,
        },
        security::SecurityManager,
    },
    Error, Result,
};

macro_rules! pd_request {
    ($cluster_id:expr, $type:ty) => {{
        let mut request = <$type>::default();
        let mut header = ::kvproto::pdpb::RequestHeader::default();
        header.set_cluster_id($cluster_id);
        request.set_header(header);
        request
    }};
}

pub trait PdClient: Sized {
    fn connect(
        env: Arc<Environment>,
        endpoints: &[String],
        security_mgr: Arc<SecurityManager>,
        timeout: Duration,
    ) -> Result<Self>;

    fn get_region(self: Arc<Self>, key: &[u8]) -> BoxFuture<'static, Result<Region>>;

    fn get_region_by_id(self: Arc<Self>, id: RegionId) -> BoxFuture<'static, Result<Region>>;

    fn get_store(self: Arc<Self>, id: StoreId) -> BoxFuture<'static, Result<metapb::Store>>;

    fn get_all_stores(self: Arc<Self>) -> BoxFuture<'static, Result<Vec<metapb::Store>>>;

    /// Request a timestamp from the PD cluster.
    fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>>;
}

/// Client for communication with a PD cluster. Has the facility to reconnect to the cluster.
pub struct RetryClient {
    cluster: RwLock<Cluster>,
    connection: Connection,
    timeout: Duration,
}

impl PdClient for RetryClient {
    fn connect(
        env: Arc<Environment>,
        endpoints: &[String],
        security_mgr: Arc<SecurityManager>,
        timeout: Duration,
    ) -> Result<RetryClient> {
        let connection = Connection::new(env, security_mgr);
        let cluster = RwLock::new(connection.connect_cluster(endpoints, timeout)?);
        Ok(RetryClient {
            cluster,
            connection,
            timeout,
        })
    }

    // These get_* functions will try multiple times to make a request, reconnecting as necessary.
    fn get_region(self: Arc<Self>, key: &[u8]) -> BoxFuture<'static, Result<Region>> {
        let key = key.to_owned();
        let timeout = self.timeout;
        Box::pin(retry_request(self, move |cluster| {
            cluster.get_region(key.clone(), timeout)
        }))
    }

    fn get_region_by_id(self: Arc<Self>, id: RegionId) -> BoxFuture<'static, Result<Region>> {
        let timeout = self.timeout;
        Box::pin(retry_request(self, move |cluster| {
            cluster.get_region_by_id(id, timeout)
        }))
    }

    fn get_store(self: Arc<Self>, id: StoreId) -> BoxFuture<'static, Result<metapb::Store>> {
        let timeout = self.timeout;
        Box::pin(retry_request(self, move |cluster| {
            cluster.get_store(id, timeout)
        }))
    }

    fn get_all_stores(self: Arc<Self>) -> BoxFuture<'static, Result<Vec<metapb::Store>>> {
        let timeout = self.timeout;
        Box::pin(retry_request(self, move |cluster| {
            cluster.get_all_stores(timeout)
        }))
    }

    fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>> {
        Box::pin(self.cluster.read().unwrap().get_timestamp())
    }
}

impl RetryClient {
    pub fn reconnect(&self, interval: u64) -> Result<()> {
        if let Some(cluster) =
            self.connection
                .reconnect(&self.cluster.read().unwrap(), interval, self.timeout)?
        {
            *self.cluster.write().unwrap() = cluster;
        }
        Ok(())
    }

    pub fn with_cluster<T, F: Fn(&Cluster) -> T>(&self, f: F) -> T {
        f(&self.cluster.read().unwrap())
    }
}

impl fmt::Debug for RetryClient {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("pd::RetryClient")
            .field("cluster_id", &self.cluster.read().unwrap().id)
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// A PD cluster.
pub struct Cluster {
    pub id: u64,
    pub(super) client: pdpb::PdClient,
    members: pdpb::GetMembersResponse,
    reactor: Arc<RwLock<PdReactor>>,
}

// These methods make a single attempt to make a request.
impl Cluster {
    fn get_region(&self, key: Vec<u8>, timeout: Duration) -> impl Future<Output = Result<Region>> {
        let context = request_context("get_region");
        let option = CallOption::default().timeout(timeout);

        let mut req = pd_request!(self.id, pdpb::GetRegionRequest);
        req.set_region_key(key.clone());

        self.client
            .get_region_async_opt(&req, option)
            .map(Compat01As03::new)
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

    fn get_region_by_id(
        &self,
        id: RegionId,
        timeout: Duration,
    ) -> impl Future<Output = Result<Region>> {
        let context = request_context("get_region_by_id");
        let option = CallOption::default().timeout(timeout);

        let mut req = pd_request!(self.id, pdpb::GetRegionByIdRequest);
        req.set_region_id(id);

        self.client
            .get_region_by_id_async_opt(&req, option)
            .map(Compat01As03::new)
            .unwrap()
            .map(move |r| context.done(r.map_err(|e| e.into())))
            .and_then(move |resp| {
                if resp.get_header().has_error() {
                    return future::ready(Err(internal_err!(resp
                        .get_header()
                        .get_error()
                        .get_message())));
                }
                let region = resp.region.ok_or_else(|| Error::region_not_found(id, None));
                let leader = resp.leader;
                future::ready(region.map(move |r| Region::new(r, leader)))
            })
    }

    fn get_store(
        &self,
        id: StoreId,
        timeout: Duration,
    ) -> impl Future<Output = Result<metapb::Store>> {
        let context = request_context("get_store");
        let option = CallOption::default().timeout(timeout);

        let mut req = pd_request!(self.id, pdpb::GetStoreRequest);
        req.set_store_id(id);

        self.client
            .get_store_async_opt(&req, option)
            .map(Compat01As03::new)
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

    fn get_all_stores(
        &self,
        timeout: Duration,
    ) -> impl Future<Output = Result<Vec<metapb::Store>>> {
        let context = request_context("get_all_stores");
        let option = CallOption::default().timeout(timeout);

        let req = pd_request!(self.id, pdpb::GetAllStoresRequest);

        self.client
            .get_all_stores_async_opt(&req, option)
            .map(Compat01As03::new)
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

    fn get_timestamp(&self) -> impl Future<Output = Result<Timestamp>> {
        self.reactor.write().unwrap().get_timestamp()
    }
}

/// An object for connecting and reconnecting to a PD cluster.
struct Connection {
    env: Arc<Environment>,
    security_mgr: Arc<SecurityManager>,
    last_update: RwLock<Instant>,
}

impl Connection {
    fn new(env: Arc<Environment>, security_mgr: Arc<SecurityManager>) -> Connection {
        Connection {
            env,
            security_mgr,
            last_update: RwLock::new(Instant::now()),
        }
    }

    fn connect_cluster(&self, endpoints: &[String], timeout: Duration) -> Result<Cluster> {
        let members = self.validate_endpoints(endpoints, timeout)?;
        let (client, members) = self.try_connect_leader(&members, timeout)?;

        let id = members.get_header().get_cluster_id();
        let cluster = Cluster {
            id,
            members,
            client,
            reactor: Arc::new(RwLock::new(PdReactor::new())),
        };

        PdReactor::start(cluster.reactor.clone(), &cluster);
        Ok(cluster)
    }

    // Re-establish connection with PD leader in synchronized fashion.
    fn reconnect(
        &self,
        old_cluster: &Cluster,
        interval: u64,
        timeout: Duration,
    ) -> Result<Option<Cluster>> {
        if self.last_update.read().unwrap().elapsed() < Duration::from_secs(interval) {
            // Avoid unnecessary updating.
            return Ok(None);
        }

        warn!("updating pd client, blocking the tokio core");
        let start = Instant::now();
        let (client, members) = self.try_connect_leader(&old_cluster.members, timeout)?;

        let cluster = Cluster {
            id: old_cluster.id,
            client,
            members,
            reactor: old_cluster.reactor.clone(),
        };
        PdReactor::start(cluster.reactor.clone(), &cluster);
        *self.last_update.write().unwrap() = Instant::now();

        warn!("updating PD client done, spent {:?}", start.elapsed());
        Ok(Some(cluster))
    }

    fn validate_endpoints(
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

            let (_, resp) = match self.connect(ep, timeout) {
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

    fn connect(
        &self,
        addr: &str,
        timeout: Duration,
    ) -> Result<(pdpb::PdClient, pdpb::GetMembersResponse)> {
        let client = self
            .security_mgr
            .connect(self.env.clone(), addr, pdpb::PdClient::new)?;
        let option = CallOption::default().timeout(timeout);
        let resp = client
            .get_members_opt(&pdpb::GetMembersRequest::default(), option)
            .map_err(Error::from)?;
        Ok((client, resp))
    }

    fn try_connect(
        &self,
        addr: &str,
        cluster_id: u64,
        timeout: Duration,
    ) -> Result<(pdpb::PdClient, pdpb::GetMembersResponse)> {
        let (client, r) = self.connect(addr, timeout)?;
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

    fn try_connect_leader(
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
                match self.try_connect(ep.as_str(), cluster_id, timeout) {
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
                let r = self.try_connect(ep.as_str(), cluster_id, timeout);
                if r.is_ok() {
                    return r;
                }
            }
        }

        Err(internal_err!("failed to connect to {:?}", members))
    }
}
