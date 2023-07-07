// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{Error, Key, Result};
use derive_new::new;
use tikv_client_proto::{kvrpcpb, metapb};

/// The ID of a region
pub type RegionId = u64;
/// The ID of a store
pub type StoreId = u64;

/// The ID and version information of a region.
#[derive(Eq, PartialEq, Hash, Clone, Default, Debug)]
pub struct RegionVerId {
    /// The ID of the region
    pub id: RegionId,
    /// Conf change version, auto increment when add or remove peer
    pub conf_ver: u64,
    /// Region version, auto increment when split or merge
    pub ver: u64,
}

/// Information about a TiKV region and its leader.
///
/// In TiKV all data is partitioned by range. Each partition is called a region.
#[derive(new, Clone, Default, Debug, PartialEq)]
pub struct RegionWithLeader {
    pub region: metapb::Region,
    pub leader: Option<metapb::Peer>,
}

impl Eq for RegionWithLeader {}

impl RegionWithLeader {
    pub fn contains(&self, key: &Key) -> bool {
        let key: &[u8] = key.into();
        let start_key = &self.region.start_key;
        let end_key = &self.region.end_key;
        key >= start_key.as_slice() && (key < end_key.as_slice() || end_key.is_empty())
    }

    pub fn context(&self) -> Result<kvrpcpb::Context> {
        self.leader
            .as_ref()
            .ok_or(Error::LeaderNotFound {
                region_id: self.region.id,
            })
            .map(|l| {
                let mut ctx = kvrpcpb::Context::default();
                ctx.region_id = self.region.id;
                ctx.region_epoch = self.region.region_epoch.clone();
                ctx.peer = Some(l.clone());
                ctx
            })
    }

    pub fn start_key(&self) -> Key {
        self.region.start_key.to_vec().into()
    }

    pub fn end_key(&self) -> Key {
        self.region.end_key.to_vec().into()
    }

    pub fn range(&self) -> (Key, Key) {
        (self.start_key(), self.end_key())
    }

    pub fn ver_id(&self) -> RegionVerId {
        let region = &self.region;
        let epoch = region.region_epoch.as_ref().unwrap();
        RegionVerId {
            id: region.id,
            conf_ver: epoch.conf_ver,
            ver: epoch.version,
        }
    }

    pub fn id(&self) -> RegionId {
        self.region.id
    }

    pub fn get_store_id(&self) -> Result<StoreId> {
        self.leader
            .as_ref()
            .cloned()
            .ok_or_else(|| Error::LeaderNotFound {
                region_id: self.id(),
            })
            .map(|s| s.store_id)
    }
}
