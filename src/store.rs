// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{pd::PdClient, BoundRange, Key, Region, Result};
use derive_new::new;
use futures::{prelude::*, stream::BoxStream};
use std::{
    any::Any,
    cmp::{max, min},
    sync::Arc,
};
use tikv_client_store::{KvClient, KvConnect, Request, TikvConnect};

#[derive(new)]
pub struct Store {
    pub region: Region,
    pub client: Box<dyn KvClient + Send + Sync>,
}

impl Store {
    pub async fn dispatch<Req: Request, Resp: Any>(&self, request: &Req) -> Result<Box<Resp>> {
        Ok(self
            .client
            .dispatch(request)
            .await?
            .downcast()
            .expect("Downcast failed"))
    }
}

pub trait KvConnectStore: KvConnect {
    fn connect_to_store(&self, region: Region, address: String) -> Result<Store> {
        info!("connect to tikv endpoint: {:?}", &address);
        let client = self.connect(address.as_str())?;
        Ok(Store::new(region, Box::new(client)))
    }
}

impl KvConnectStore for TikvConnect {}

pub fn store_stream_for_key<KeyData, PdC>(
    key_data: KeyData,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(KeyData, Store)>>
where
    KeyData: AsRef<Key> + Send + 'static,
    PdC: PdClient,
{
    pd_client
        .store_for_key(key_data.as_ref().clone())
        .map_ok(move |store| (key_data, store))
        .into_stream()
        .boxed()
}

/// Maps keys to a stream of stores. `key_data` must be sorted in increasing order
pub fn store_stream_for_keys<KeyData, IntoKey, I, PdC>(
    key_data: I,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(Vec<KeyData>, Store)>>
where
    KeyData: AsRef<Key> + Send + Sync + 'static,
    IntoKey: Into<KeyData> + 'static,
    I: IntoIterator<Item = IntoKey>,
    I::IntoIter: Send + Sync + 'static,
    PdC: PdClient,
{
    pd_client
        .clone()
        .group_keys_by_region(key_data.into_iter().map(Into::into))
        .and_then(move |(region_id, key)| {
            pd_client
                .clone()
                .store_for_id(region_id)
                .map_ok(move |store| (key, store))
        })
        .boxed()
}

pub fn store_stream_for_range<PdC: PdClient>(
    range: BoundRange,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<((Key, Key), Store)>> {
    pd_client
        .stores_for_range(range.clone())
        .map_ok(move |store| {
            let region_range = store.region.range();
            (bound_range(region_range, range.clone()), store)
        })
        .into_stream()
        .boxed()
}

/// The range used for request should be the intersection of `region_range` and `range`.
fn bound_range(region_range: (Key, Key), range: BoundRange) -> (Key, Key) {
    let (lower, upper) = region_range;
    let (lower_bound, upper_bound) = range.into_keys();
    let up = match (upper.is_empty(), upper_bound) {
        (_, None) => upper,
        (true, Some(ub)) => ub,
        (_, Some(ub)) if ub.is_empty() => upper,
        (_, Some(ub)) => min(upper, ub),
    };
    (max(lower, lower_bound), up)
}

pub fn store_stream_for_ranges<PdC: PdClient>(
    ranges: Vec<BoundRange>,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(Vec<BoundRange>, Store)>> {
    pd_client
        .clone()
        .group_ranges_by_region(ranges)
        .and_then(move |(region_id, range)| {
            pd_client
                .clone()
                .store_for_id(region_id)
                .map_ok(move |store| (range, store))
        })
        .into_stream()
        .boxed()
}
