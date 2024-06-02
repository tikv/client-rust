// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

mod client;
mod errors;
mod request;

use std::cmp::max;
use std::cmp::min;
use std::sync::Arc;

use derive_new::new;
use futures::prelude::*;
use futures::stream::BoxStream;

pub use self::client::KvClient;
pub use self::client::KvConnect;
pub use self::client::TikvConnect;
pub use self::errors::HasKeyErrors;
pub use self::errors::HasRegionError;
pub use self::errors::HasRegionErrors;
pub use self::request::Request;
use crate::pd::PdClient;
use crate::proto::kvrpcpb;
use crate::region::RegionWithLeader;
use crate::BoundRange;
use crate::Key;
use crate::Result;

#[derive(new, Clone)]
pub struct RegionStore {
    pub region_with_leader: RegionWithLeader,
    pub client: Arc<dyn KvClient + Send + Sync>,
}

#[derive(new, Clone)]
pub struct Store {
    pub client: Arc<dyn KvClient + Send + Sync>,
}

/// Maps keys to a stream of stores. `key_data` must be sorted in increasing order
pub fn store_stream_for_keys<K, KOut, PdC>(
    key_data: impl Iterator<Item = K> + Send + Sync + 'static,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(Vec<KOut>, RegionStore)>>
where
    PdC: PdClient,
    K: AsRef<Key> + Into<KOut> + Send + Sync + 'static,
    KOut: Send + Sync + 'static,
{
    pd_client
        .clone()
        .group_keys_by_region(key_data)
        .and_then(move |(region, key)| {
            pd_client
                .clone()
                .map_region_to_store(region)
                .map_ok(move |store| (key, store))
        })
        .boxed()
}

#[allow(clippy::type_complexity)]
pub fn store_stream_for_range<PdC: PdClient>(
    range: (Vec<u8>, Vec<u8>),
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<((Vec<u8>, Vec<u8>), RegionStore)>> {
    let bnd_range = if range.1.is_empty() {
        BoundRange::range_from(range.0.clone().into())
    } else {
        BoundRange::from(range.clone())
    };
    pd_client
        .stores_for_range(bnd_range)
        .map_ok(move |store| {
            let region_range = store.region_with_leader.range();
            let result_range = range_intersection(
                region_range,
                (range.0.clone().into(), range.1.clone().into()),
            );
            ((result_range.0.into(), result_range.1.into()), store)
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

pub fn store_stream_for_ranges<PdC: PdClient>(
    ranges: Vec<kvrpcpb::KeyRange>,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(Vec<kvrpcpb::KeyRange>, RegionStore)>> {
    pd_client
        .clone()
        .group_ranges_by_region(ranges)
        .and_then(move |(region, range)| {
            pd_client
                .clone()
                .map_region_to_store(region)
                .map_ok(move |store| (range, store))
        })
        .boxed()
}
