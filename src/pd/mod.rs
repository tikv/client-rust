// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use derive_new::new;
use kvproto::{kvrpcpb, metapb, pdpb};

use crate::{Error, Key, Result};
pub use client::{PdClient, PdRpcClient};
pub use cluster::Cluster;
pub use retry::RetryClient;

mod client;
#[macro_use]
mod cluster;
mod retry;
mod timestamp;

pub type RegionId = u64;
pub type StoreId = u64;

#[derive(Eq, PartialEq, Hash, Clone, Default, Debug)]
pub struct RegionVerId {
    pub id: RegionId,
    pub conf_ver: u64,
    pub ver: u64,
}

#[derive(new, Clone, Default, Debug, PartialEq)]
pub struct Region {
    pub region: metapb::Region,
    pub leader: Option<metapb::Peer>,
}

impl Region {
    #[allow(dead_code)]
    pub fn switch_peer(&mut self, _to: StoreId) -> Result<()> {
        unimplemented!()
    }

    pub fn contains(&self, key: &Key) -> bool {
        let key: &[u8] = key.into();
        let start_key = self.region.get_start_key();
        let end_key = self.region.get_end_key();
        key >= start_key && (key < end_key || end_key.is_empty())
    }

    pub fn context(&self) -> Result<kvrpcpb::Context> {
        self.leader
            .as_ref()
            .ok_or_else(|| Error::not_leader(self.region.get_id(), None))
            .map(|l| {
                let mut ctx = kvrpcpb::Context::default();
                ctx.set_region_id(self.region.get_id());
                ctx.set_region_epoch(Clone::clone(self.region.get_region_epoch()));
                ctx.set_peer(Clone::clone(l));
                ctx
            })
    }

    pub fn start_key(&self) -> Key {
        self.region.get_start_key().to_vec().into()
    }

    pub fn end_key(&self) -> Key {
        self.region.get_end_key().to_vec().into()
    }

    pub fn range(&self) -> (Key, Key) {
        (self.start_key(), self.end_key())
    }

    #[allow(dead_code)]
    pub fn ver_id(&self) -> RegionVerId {
        let region = &self.region;
        let epoch = region.get_region_epoch();
        RegionVerId {
            id: region.get_id(),
            conf_ver: epoch.get_conf_ver(),
            ver: epoch.get_version(),
        }
    }

    pub fn id(&self) -> RegionId {
        self.region.get_id()
    }

    pub fn get_store_id(&self) -> Result<StoreId> {
        self.leader
            .as_ref()
            .cloned()
            .ok_or_else(|| Error::stale_epoch(None))
            .map(|s| s.get_store_id())
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
