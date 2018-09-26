// Copyright 2017 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use futures::sync::oneshot;
use futures::Future;
use grpc::{CallOption, EnvBuilder};
use kvproto::metapb;
use kvproto::pdpb::{self, Member};

use super::util::{check_resp_header, validate_endpoints, LeaderClient, Request};
use super::{Error, PdClient, RegionInfo, Result, PD_REQUEST_HISTOGRAM_VEC, REQUEST_TIMEOUT};
use pd::{PdFuture, PdTimestamp};
use util::security::SecurityManager;
use util::time::duration_to_sec;
use util::HandyRwLock;

const CQ_COUNT: usize = 1;
const CLIENT_PREFIX: &str = "pd";
const LEADER_CHANGE_RETRY: usize = 10;

macro_rules! thd_name {
    ($name:expr) => {{
        $crate::util::get_tag_from_thread_name()
            .map(|tag| format!("{}::{}", $name, tag))
            .unwrap_or_else(|| $name.to_owned())
    }};
}

pub struct PdRpcClient {
    cluster_id: u64,
    leader: Arc<RwLock<LeaderClient>>,
}

macro_rules! request {
    ($cluster_id:expr, $type:ty) => {{
        let mut request = <$type>::new();
        let mut header = pdpb::RequestHeader::new();
        header.set_cluster_id($cluster_id);
        request.set_header(header);
        request
    }};
}

impl PdRpcClient {
    pub fn new(endpoints: &[&str], security_mgr: Arc<SecurityManager>) -> Result<PdRpcClient> {
        let env = Arc::new(
            EnvBuilder::new()
                .cq_count(CQ_COUNT)
                .name_prefix(thd_name!(CLIENT_PREFIX))
                .build(),
        );
        let (client, members) = validate_endpoints(&env, endpoints, &security_mgr)?;

        Ok(PdRpcClient {
            cluster_id: members.get_header().get_cluster_id(),
            leader: Arc::new(RwLock::new(LeaderClient::new(
                env,
                security_mgr,
                client,
                members,
            ))),
        })
    }

    #[inline]
    fn call_option() -> CallOption {
        CallOption::default().timeout(Duration::from_secs(REQUEST_TIMEOUT))
    }

    pub fn get_leader(&self) -> Member {
        self.leader.rl().members.get_leader().clone()
    }

    fn get_region_and_leader_async(
        &self,
        key: &[u8],
    ) -> impl Future<Item = (metapb::Region, Option<metapb::Peer>), Error = Error> {
        let timer = Instant::now();

        let mut req = request!(self.cluster_id, pdpb::GetRegionRequest);
        req.set_region_key(key.to_owned());

        let executor = move |client: &RwLock<LeaderClient>, req: pdpb::GetRegionRequest| {
            let receiver = client
                .rl()
                .client
                .get_region_async_opt(&req, Self::call_option())
                .unwrap();
            let key = req.get_region_key().to_owned();
            Box::new(receiver.map_err(Error::Grpc).and_then(move |mut resp| {
                PD_REQUEST_HISTOGRAM_VEC
                    .with_label_values(&["get_region"])
                    .observe(duration_to_sec(timer.elapsed()));
                check_resp_header(resp.get_header())?;
                let region = if resp.has_region() {
                    resp.take_region()
                } else {
                    return Err(Error::RegionNotFound(key));
                };
                let leader = if resp.has_leader() {
                    Some(resp.take_leader())
                } else {
                    None
                };
                Ok((region, leader))
            })) as PdFuture<_>
        };
        self.request(req, executor, LEADER_CHANGE_RETRY).execute()
    }

    fn get_store_async(&self, store_id: u64) -> impl Future<Item = metapb::Store, Error = Error> {
        let timer = Instant::now();

        let mut req = request!(self.cluster_id, pdpb::GetStoreRequest);
        req.set_store_id(store_id);

        let executor = move |client: &RwLock<LeaderClient>, req: pdpb::GetStoreRequest| {
            let receiver = client
                .rl()
                .client
                .get_store_async_opt(&req, Self::call_option())
                .unwrap();
            Box::new(receiver.map_err(Error::Grpc).and_then(move |mut resp| {
                PD_REQUEST_HISTOGRAM_VEC
                    .with_label_values(&["get_store"])
                    .observe(duration_to_sec(timer.elapsed()));
                check_resp_header(resp.get_header())?;
                Ok(resp.take_store())
            })) as PdFuture<_>
        };

        self.request(req, executor, LEADER_CHANGE_RETRY).execute()
    }

    pub fn get_cluster_id(&self) -> Result<u64> {
        Ok(self.cluster_id)
    }

    pub fn get_ts(&self) -> Result<PdTimestamp> {
        self.get_ts_async().wait()
    }

    pub fn get_ts_async(&self) -> PdFuture<PdTimestamp> {
        let timer = Instant::now();

        let mut req = request!(self.cluster_id, pdpb::TsoRequest);
        req.set_count(1);

        let (tx, rx) = oneshot::channel::<PdTimestamp>();
        let leader = self.leader.wl();
        leader.tso_requests_sender.unbounded_send(tx).unwrap();
        Box::new(rx.map_err(Error::Canceled).and_then(move |ts| {
            PD_REQUEST_HISTOGRAM_VEC
                .with_label_values(&["get_ts"])
                .observe(duration_to_sec(timer.elapsed()));
            Ok(ts)
        }))
    }

    pub fn on_reconnect(&self, f: Box<Fn() + Sync + Send + 'static>) {
        let mut leader = self.leader.wl();
        leader.on_reconnect = Some(f);
    }

    pub fn request<Req, Resp, F>(&self, req: Req, func: F, retry: usize) -> Request<Req, Resp, F>
    where
        Req: Clone + Send + 'static,
        Resp: Send + 'static,
        F: FnMut(&RwLock<LeaderClient>, Req) -> PdFuture<Resp> + Send + 'static,
    {
        Request::new(req, func, Arc::clone(&self.leader), retry)
    }
}

impl PdClient for PdRpcClient {
    fn get_cluster_id(&self) -> Result<u64> {
        Ok(self.cluster_id)
    }

    fn get_cluster_config(&self) -> PdFuture<metapb::Cluster> {
        unimplemented!()
    }

    fn get_store(&self, store_id: u64) -> PdFuture<metapb::Store> {
        Box::new(self.get_store_async(store_id).and_then(Ok))
    }

    fn get_region(&self, key: &[u8]) -> PdFuture<metapb::Region> {
        Box::new(self.get_region_and_leader_async(key).and_then(|x| Ok(x.0)))
    }

    fn get_region_info(&self, key: &[u8]) -> PdFuture<RegionInfo> {
        Box::new(
            self.get_region_and_leader_async(key)
                .and_then(|x| Ok(RegionInfo::new(x.0, x.1))),
        )
    }

    fn get_region_by_id(&self, region_id: u64) -> PdFuture<Option<metapb::Region>> {
        let timer = Instant::now();

        let mut req = request!(self.cluster_id, pdpb::GetRegionByIDRequest);
        req.set_region_id(region_id);

        let executor = move |client: &RwLock<LeaderClient>, req: pdpb::GetRegionByIDRequest| {
            let handler = client
                .rl()
                .client
                .get_region_by_id_async_opt(&req, Self::call_option())
                .unwrap();
            Box::new(handler.map_err(Error::Grpc).and_then(move |mut resp| {
                PD_REQUEST_HISTOGRAM_VEC
                    .with_label_values(&["get_region_by_id"])
                    .observe(duration_to_sec(timer.elapsed()));
                check_resp_header(resp.get_header())?;
                if resp.has_region() {
                    Ok(Some(resp.take_region()))
                } else {
                    Ok(None)
                }
            })) as PdFuture<_>
        };

        self.request(req, executor, LEADER_CHANGE_RETRY).execute()
    }
}

impl fmt::Debug for PdRpcClient {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("PdRpcClient")
            .field("cluster_id", &self.cluster_id)
            .field("leader", &self.get_leader())
            .finish()
    }
}
