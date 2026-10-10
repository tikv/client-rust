// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

mod client;
mod errors;
mod request;

use std::cmp::max;
use std::cmp::min;
use std::sync::Arc;

use futures::prelude::*;
use futures::stream::BoxStream;

pub use self::client::KvClient;
pub use self::client::KvConnect;
pub use self::client::TikvConnect;
pub use self::errors::HasKeyErrors;
pub use self::errors::HasRegionError;
pub use self::errors::HasRegionErrors;
pub(crate) use self::request::reload_txn_protocol_range;
pub use self::request::Request;
use crate::pd::PdClient;
use crate::proto::kvrpcpb;
use crate::region::RegionWithLeader;
use crate::BoundRange;
use crate::Key;
use crate::Result;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TxnProtocolVersionRange {
    pub min: u32,
    pub max: u32,
}

impl TxnProtocolVersionRange {
    /// The version the client would send before validating the store's range.
    pub(crate) fn preferred_version(self, default_version: u32) -> u32 {
        default_version.min(self.max)
    }

    /// Select a version supported by both the request and the store.
    pub(crate) fn select(self, default_version: u32, required_version: u32) -> Option<u32> {
        let selected = self.preferred_version(default_version);
        (self.min <= self.max && selected >= self.min && selected >= required_version)
            .then_some(selected)
    }
}

impl From<&crate::proto::metapb::Store> for TxnProtocolVersionRange {
    fn from(store: &crate::proto::metapb::Store) -> Self {
        store
            .txn_protocol_version_range
            .as_ref()
            .map(|range| Self {
                min: range.min,
                max: range.max,
            })
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TxnProtocolRequirement {
    NotTransaction,
    Transaction { required_version: u32 },
}

impl TxnProtocolRequirement {
    pub(crate) fn select(
        self,
        range: TxnProtocolVersionRange,
        default_version: u32,
    ) -> Option<u32> {
        match self {
            Self::NotTransaction => None,
            Self::Transaction { required_version } => {
                range.select(default_version, required_version)
            }
        }
    }
}

#[derive(Clone)]
pub struct RegionStore {
    pub region_with_leader: RegionWithLeader,
    pub store_id: u64,
    pub txn_protocol_version_range: TxnProtocolVersionRange,
    pub client: Arc<dyn KvClient + Send + Sync>,
}

impl RegionStore {
    pub fn new(
        region_with_leader: RegionWithLeader,
        client: Arc<dyn KvClient + Send + Sync>,
    ) -> Self {
        let store_id = region_with_leader.get_store_id().unwrap_or_default();
        Self::with_metadata(
            region_with_leader,
            store_id,
            TxnProtocolVersionRange::default(),
            client,
        )
    }

    pub fn with_metadata(
        region_with_leader: RegionWithLeader,
        store_id: u64,
        txn_protocol_version_range: TxnProtocolVersionRange,
        client: Arc<dyn KvClient + Send + Sync>,
    ) -> Self {
        Self {
            region_with_leader,
            store_id,
            txn_protocol_version_range,
            client,
        }
    }
}

#[derive(Clone)]
pub struct Store {
    pub store_id: u64,
    pub txn_protocol_version_range: TxnProtocolVersionRange,
    pub client: Arc<dyn KvClient + Send + Sync>,
}

impl Store {
    pub fn new(client: Arc<dyn KvClient + Send + Sync>) -> Self {
        Self::with_metadata(0, TxnProtocolVersionRange::default(), client)
    }

    pub fn with_metadata(
        store_id: u64,
        txn_protocol_version_range: TxnProtocolVersionRange,
        client: Arc<dyn KvClient + Send + Sync>,
    ) -> Self {
        Self {
            store_id,
            txn_protocol_version_range,
            client,
        }
    }
}

/// Maps keys to a stream of stores. `key_data` must be sorted in increasing order
pub fn region_stream_for_keys<K, KOut, PdC>(
    key_data: impl Iterator<Item = K> + Send + Sync + 'static,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(Vec<KOut>, RegionWithLeader)>>
where
    PdC: PdClient,
    K: AsRef<Key> + Into<KOut> + Send + Sync + 'static,
    KOut: Send + Sync + 'static,
{
    pd_client.clone().group_keys_by_region(key_data)
}

#[allow(clippy::type_complexity)]
pub fn region_stream_for_range<PdC: PdClient>(
    range: (Vec<u8>, Vec<u8>),
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<((Vec<u8>, Vec<u8>), RegionWithLeader)>> {
    let bnd_range = if range.1.is_empty() {
        BoundRange::range_from(range.0.clone().into())
    } else {
        BoundRange::from(range.clone())
    };
    pd_client
        .regions_for_range(bnd_range)
        .map_ok(move |region| {
            let region_range = region.range();
            let result_range = range_intersection(
                region_range,
                (range.0.clone().into(), range.1.clone().into()),
            );
            ((result_range.0.into(), result_range.1.into()), region)
        })
        .boxed()
}

/// The range used for request should be the intersection of `region_range` and `range`.
fn range_intersection(region_range: (Key, Key), range: (Key, Key)) -> (Key, Key) {
    let (lower, upper) = region_range;
    let up = if upper.is_empty() {
        range.1
    } else if range.1.is_empty() {
        upper
    } else {
        min(upper, range.1)
    };
    (max(lower, range.0), up)
}

pub fn region_stream_for_ranges<PdC: PdClient>(
    ranges: Vec<kvrpcpb::KeyRange>,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(Vec<kvrpcpb::KeyRange>, RegionWithLeader)>> {
    pd_client.clone().group_ranges_by_region(ranges)
}

#[cfg(test)]
mod txn_protocol_tests {
    use super::*;

    #[rstest::rstest]
    #[case(None, TxnProtocolVersionRange { min: 0, max: 0 })]
    #[case(Some(crate::proto::metapb::TxnProtocolVersionRange { min: 2, max: 1 }), TxnProtocolVersionRange { min: 2, max: 1 })]
    fn store_range_conversion_preserves_invalid_metadata(
        #[case] range: Option<crate::proto::metapb::TxnProtocolVersionRange>,
        #[case] expected: TxnProtocolVersionRange,
    ) {
        let store = crate::proto::metapb::Store {
            txn_protocol_version_range: range,
            ..Default::default()
        };
        assert_eq!(TxnProtocolVersionRange::from(&store), expected);
    }

    #[test]
    fn select_txn_protocol_version_validates_the_whole_range() {
        let range = TxnProtocolVersionRange { min: 0, max: 2 };
        assert_eq!(range.select(1, 0), Some(1));
        assert_eq!(range.select(1, 2), None);
        assert_eq!(range.select(2, 2), Some(2));
        assert_eq!(
            TxnProtocolVersionRange { min: 2, max: 1 }.select(2, 0),
            None
        );
        assert_eq!(
            TxnProtocolVersionRange { min: 2, max: 3 }.select(1, 0),
            None
        );
    }
}
