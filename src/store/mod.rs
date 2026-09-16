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
use crate::compat::stream_fn;
use crate::pd::PdClient;
use crate::proto::kvrpcpb;
use crate::region::RegionWithLeader;
use crate::request::{KeyMode, Keyspace};
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

pub fn region_stream_for_keys_with_keyspace<K, KOut, PdC>(
    key_data: impl Iterator<Item = K> + Send + Sync + 'static,
    pd_client: Arc<PdC>,
    keyspace: Keyspace,
    key_mode: KeyMode,
) -> BoxStream<'static, Result<(Vec<KOut>, RegionWithLeader)>>
where
    PdC: PdClient,
    K: AsRef<Key> + Into<KOut> + Send + Sync + 'static,
    KOut: Send + Sync + 'static,
{
    if !matches!(keyspace, Keyspace::ApiV3 { .. }) {
        return region_stream_for_keys(key_data, pd_client);
    }

    let mut route_pairs: Vec<(Key, K)> = key_data
        .map(|key| (keyspace.encode_route_key(key.as_ref(), key_mode), key))
        .collect();
    route_pairs.sort_by(|left, right| left.0.cmp(&right.0));

    let route_pairs = route_pairs.into_iter().peekable();
    stream_fn(Some(route_pairs), move |route_pairs| {
        let this = pd_client.clone();
        async move {
            let mut route_pairs = match route_pairs {
                None => return Ok(None),
                Some(route_pairs) => route_pairs,
            };
            let Some((route_key, key)) = route_pairs.next() else {
                return Ok(None);
            };
            let region = this.region_for_key(&route_key).await?;
            let mut grouped = vec![key.into()];
            while let Some((next_route_key, _)) = route_pairs.peek() {
                if !region.contains(next_route_key) {
                    break;
                }
                grouped.push(route_pairs.next().unwrap().1.into());
            }
            Ok(Some((Some(route_pairs), (grouped, region))))
        }
    })
    .boxed()
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

#[allow(clippy::type_complexity)]
pub fn region_stream_for_range_with_keyspace<PdC: PdClient>(
    range: (Vec<u8>, Vec<u8>),
    pd_client: Arc<PdC>,
    keyspace: Keyspace,
    key_mode: KeyMode,
) -> BoxStream<'static, Result<((Vec<u8>, Vec<u8>), RegionWithLeader)>> {
    if !matches!(keyspace, Keyspace::ApiV3 { .. }) {
        return region_stream_for_range(range, pd_client);
    }

    let user_range = (Key::from(range.0.clone()), Key::from(range.1.clone()));
    let route_range = keyspace.encode_route_range(user_range.0, user_range.1, key_mode);
    let route_range_vec: (Vec<u8>, Vec<u8>) =
        (route_range.0.clone().into(), route_range.1.clone().into());
    let bnd_range = if route_range.1.is_empty() {
        BoundRange::range_from(route_range.0.clone())
    } else {
        BoundRange::from(route_range_vec.clone())
    };
    pd_client
        .regions_for_range(bnd_range)
        .map_ok(move |region| {
            let region_range = region.range();
            let route_result_range = range_intersection(
                region_range,
                (
                    route_range_vec.0.clone().into(),
                    route_range_vec.1.clone().into(),
                ),
            );
            let user_result_range =
                keyspace.decode_route_range(route_result_range.0, route_result_range.1, key_mode);
            (
                (user_result_range.0.into(), user_result_range.1.into()),
                region,
            )
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

pub fn region_stream_for_ranges_with_keyspace<PdC: PdClient>(
    ranges: Vec<kvrpcpb::KeyRange>,
    pd_client: Arc<PdC>,
    keyspace: Keyspace,
    key_mode: KeyMode,
) -> BoxStream<'static, Result<(Vec<kvrpcpb::KeyRange>, RegionWithLeader)>> {
    if !matches!(keyspace, Keyspace::ApiV3 { .. }) {
        return region_stream_for_ranges(ranges, pd_client);
    }

    let mut route_ranges: Vec<kvrpcpb::KeyRange> = ranges
        .into_iter()
        .map(|range| {
            let (start_key, end_key) = keyspace.encode_route_range(
                Key::from(range.start_key),
                Key::from(range.end_key),
                key_mode,
            );
            make_key_range(start_key.into(), end_key.into())
        })
        .collect();
    route_ranges.reverse();
    stream_fn(Some(route_ranges), move |ranges| {
        let this = pd_client.clone();
        async move {
            let mut ranges = match ranges {
                None => return Ok(None),
                Some(r) => r,
            };

            if let Some(route_range) = ranges.pop() {
                let start_key: Key = route_range.start_key.clone().into();
                let end_key: Key = route_range.end_key.clone().into();
                let region = this.region_for_key(&start_key).await?;
                let region_start = region.start_key();
                let region_end = region.end_key();
                let mut grouped = vec![];
                if !region_end.is_empty() && (end_key > region_end || end_key.is_empty()) {
                    grouped.push(make_user_key_range(
                        keyspace,
                        key_mode,
                        start_key,
                        region_end.clone(),
                    ));
                    ranges.push(make_key_range(region_end.into(), end_key.into()));
                    return Ok(Some((Some(ranges), (grouped, region))));
                }
                grouped.push(make_user_key_range(keyspace, key_mode, start_key, end_key));

                while let Some(route_range) = ranges.pop() {
                    let start_key: Key = route_range.start_key.clone().into();
                    let end_key: Key = route_range.end_key.clone().into();
                    if start_key < region_start
                        || (!region_end.is_empty() && start_key >= region_end)
                    {
                        ranges.push(route_range);
                        break;
                    }
                    if !region_end.is_empty() && (end_key > region_end || end_key.is_empty()) {
                        grouped.push(make_user_key_range(
                            keyspace,
                            key_mode,
                            start_key,
                            region_end.clone(),
                        ));
                        ranges.push(make_key_range(region_end.into(), end_key.into()));
                        return Ok(Some((Some(ranges), (grouped, region))));
                    }
                    grouped.push(make_user_key_range(keyspace, key_mode, start_key, end_key));
                }
                Ok(Some((Some(ranges), (grouped, region))))
            } else {
                Ok(None)
            }
        }
    })
    .boxed()
}

fn make_user_key_range(
    keyspace: Keyspace,
    key_mode: KeyMode,
    start: Key,
    end: Key,
) -> kvrpcpb::KeyRange {
    let (start, end) = keyspace.decode_route_range(start, end, key_mode);
    make_key_range(start.into(), end.into())
}

fn make_key_range(start_key: Vec<u8>, end_key: Vec<u8>) -> kvrpcpb::KeyRange {
    kvrpcpb::KeyRange { start_key, end_key }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::executor;

    use super::*;
    use crate::mock::MockPdClient;

    #[test]
    fn test_api_v3_key_routing_uses_physical_key_and_keeps_user_keys() {
        let keyspace = Keyspace::api_v3(1, 7).unwrap();
        let user_keys: Vec<Key> = vec![vec![1].into(), vec![2].into()];
        let pd_client = Arc::new(MockPdClient::default());

        let mut stream =
            executor::block_on_stream(
                region_stream_for_keys_with_keyspace::<Key, Key, MockPdClient>(
                    user_keys.clone().into_iter(),
                    pd_client,
                    keyspace,
                    KeyMode::Txn,
                ),
            );

        let (keys, region) = stream.next().unwrap().unwrap();
        assert_eq!(region.id(), 2);
        assert_eq!(keys, user_keys);
        assert!(stream.next().is_none());
    }

    #[test]
    fn test_api_v3_range_routing_uses_physical_key_and_returns_user_range() {
        let keyspace = Keyspace::api_v3(1, 7).unwrap();
        let pd_client = Arc::new(MockPdClient::default());

        let mut stream = executor::block_on_stream(region_stream_for_range_with_keyspace(
            (vec![1], vec![2]),
            pd_client,
            keyspace,
            KeyMode::Txn,
        ));

        let (range, region) = stream.next().unwrap().unwrap();
        assert_eq!(region.id(), 2);
        assert_eq!(range, (vec![1], vec![2]));
        assert!(stream.next().is_none());
    }

    #[test]
    fn test_api_v3_multi_range_routing_returns_user_ranges() {
        let keyspace = Keyspace::api_v3(1, 7).unwrap();
        let pd_client = Arc::new(MockPdClient::default());
        let user_range = make_key_range(vec![1], vec![2]);

        let mut stream = executor::block_on_stream(region_stream_for_ranges_with_keyspace(
            vec![user_range.clone()],
            pd_client,
            keyspace,
            KeyMode::Txn,
        ));

        let (ranges, region) = stream.next().unwrap().unwrap();
        assert_eq!(region.id(), 2);
        assert_eq!(ranges, vec![user_range]);
        assert!(stream.next().is_none());
    }
}
