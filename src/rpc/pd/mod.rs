// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

// TODO: Remove this when txn is done.
#![allow(dead_code)]

use derive_new::new;
pub use kvproto::metapb::{Peer, Store};
use kvproto::{kvrpcpb, metapb};

pub use crate::rpc::pd::client::PdClient;
use crate::{Error, Key, Result};

#[macro_use]
mod leader;
mod client;
mod context;
mod request;

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
    pub leader: Option<Peer>,
}

impl Region {
    pub fn switch_peer(&mut self, _to: StoreId) -> Result<()> {
        unimplemented!()
    }

    pub fn contains(&self, key: &Key) -> bool {
        let key: &[u8] = key.into();
        let start_key = self.region.get_start_key();
        let end_key = self.region.get_end_key();
        start_key <= key && (end_key > key || end_key.is_empty())
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

    pub fn start_key(&self) -> &[u8] {
        self.region.get_start_key()
    }

    pub fn end_key(&self) -> &[u8] {
        self.region.get_end_key()
    }

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

    pub fn peer(&self) -> Result<Peer> {
        self.leader
            .as_ref()
            .map(Clone::clone)
            .map(Into::into)
            .ok_or_else(|| Error::stale_epoch(None))
    }

    pub fn meta(&self) -> metapb::Region {
        self.region.clone()
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Timestamp {
    pub physical: i64,
    pub logical: i64,
}
