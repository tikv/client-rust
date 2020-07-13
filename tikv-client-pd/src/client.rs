// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{cluster::Cluster, Region, RegionId, RetryClient};
use futures::{
    future::{ready, BoxFuture, Either},
    prelude::*,
    stream::BoxStream,
};
use grpcio::{EnvBuilder, Environment};
use std::{sync::Arc, time::Duration};
use tikv_client_common::{
    compat::{stream_fn, ClientFutureExt},
    kv::BoundRange,
    security::SecurityManager,
    Config, Key, Result, Timestamp,
};

pub struct StoreBuilder {
    pub region: Region,
    pub address: String,
    pub timeout: Duration,
}

impl StoreBuilder {
    pub fn new(region: Region, address: String, timeout: Duration) -> StoreBuilder {
        StoreBuilder {
            region,
            address,
            timeout,
        }
    }
}

const CQ_COUNT: usize = 1;
const CLIENT_PREFIX: &str = "tikv-client";

pub trait PdClient: Send + Sync + 'static {
    fn map_region_to_store_builder(
        self: Arc<Self>,
        region: Region,
    ) -> BoxFuture<'static, Result<StoreBuilder>>;

    fn region_for_key(&self, key: &Key) -> BoxFuture<'static, Result<Region>>;

    fn region_for_id(&self, id: RegionId) -> BoxFuture<'static, Result<Region>>;

    fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>>;

    fn store_for_key(self: Arc<Self>, key: &Key) -> BoxFuture<'static, Result<StoreBuilder>> {
        self.region_for_key(key)
            .and_then(move |region| self.map_region_to_store_builder(region))
            .boxed()
    }

    fn store_for_id(self: Arc<Self>, id: RegionId) -> BoxFuture<'static, Result<StoreBuilder>> {
        self.region_for_id(id)
            .and_then(move |region| self.map_region_to_store_builder(region).boxed())
            .boxed()
    }

    fn group_keys_by_region<K: AsRef<Key> + Send + Sync + 'static>(
        self: Arc<Self>,
        keys: impl Iterator<Item = K> + Send + Sync + 'static,
    ) -> BoxStream<'static, Result<(RegionId, Vec<K>)>> {
        let keys = keys.peekable();
        stream_fn(keys, move |mut keys| {
            if let Some(key) = keys.next() {
                Either::Left(self.region_for_key(key.as_ref()).map_ok(move |region| {
                    let id = region.id();
                    let mut grouped = vec![key];
                    while let Some(key) = keys.peek() {
                        if !region.contains(key.as_ref()) {
                            break;
                        }
                        grouped.push(keys.next().unwrap());
                    }
                    Some((keys, (id, grouped)))
                }))
            } else {
                Either::Right(ready(Ok(None)))
            }
        })
        .boxed()
    }

    // Returns a Steam which iterates over the contexts for each region covered by range.
    fn stores_for_range(
        self: Arc<Self>,
        range: BoundRange,
    ) -> BoxStream<'static, Result<StoreBuilder>> {
        let (start_key, end_key) = range.into_keys();
        stream_fn(Some(start_key), move |start_key| {
            let start_key = match start_key {
                None => return Either::Right(ready(Ok(None))),
                Some(sk) => sk,
            };
            let end_key = end_key.clone();

            let this = self.clone();
            Either::Left(self.region_for_key(&start_key).and_then(move |region| {
                let region_end = region.end_key();
                this.map_region_to_store_builder(region)
                    .map_ok(move |store_builder| {
                        if end_key.map(|x| x < region_end).unwrap_or(false) || region_end.is_empty()
                        {
                            return Some((None, store_builder));
                        }
                        Some((Some(region_end), store_builder))
                    })
            }))
        })
        .boxed()
    }

    // Returns a Steam which iterates over the contexts for ranges in the same region.
    fn group_ranges_by_region(
        self: Arc<Self>,
        mut ranges: Vec<BoundRange>,
    ) -> BoxStream<'static, Result<(RegionId, Vec<BoundRange>)>> {
        ranges.reverse();
        stream_fn(Some(ranges), move |ranges| {
            let mut ranges = match ranges {
                None => return Either::Right(ready(Ok(None))),
                Some(r) => r,
            };

            if let Some(range) = ranges.pop() {
                let (start_key, end_key) = range.clone().into_keys();
                Either::Left(self.region_for_key(&start_key).map_ok(move |region| {
                    let id = region.id();
                    let region_start = region.start_key();
                    let region_end = region.end_key();
                    let mut grouped = vec![];
                    if !region_end.is_empty()
                        && end_key.clone().map(|x| x > region_end).unwrap_or(true)
                    {
                        grouped.push((start_key, region_end.clone()).into());
                        ranges.push((region_end, end_key).into());
                        return Some((Some(ranges), (id, grouped)));
                    }
                    grouped.push(range);

                    while let Some(range) = ranges.pop() {
                        let (start_key, end_key) = range.clone().into_keys();
                        if start_key < region_start {
                            ranges.push(range);
                            break;
                        }
                        if !region_end.is_empty()
                            && end_key.clone().map(|x| x > region_end).unwrap_or(true)
                        {
                            grouped.push((start_key, region_end.clone()).into());
                            ranges.push((region_end, end_key).into());
                            return Some((Some(ranges), (id, grouped)));
                        }
                        grouped.push(range);
                    }
                    Some((Some(ranges), (id, grouped)))
                }))
            } else {
                Either::Right(ready(Ok(None)))
            }
        })
        .boxed()
    }
}

/// This client converts requests for the logical TiKV cluster into requests
/// for a single TiKV store using PD and internal logic.
pub struct PdRpcClient<Cl = Cluster> {
    pd: Arc<RetryClient<Cl>>,
    timeout: Duration,
}

impl PdClient for PdRpcClient {
    fn map_region_to_store_builder(
        self: Arc<Self>,
        region: Region,
    ) -> BoxFuture<'static, Result<StoreBuilder>> {
        let timeout = self.timeout;
        // FIXME propagate this error instead of using `unwrap`.
        let store_id = region.get_store_id().unwrap();
        self.pd
            .clone()
            .get_store(store_id)
            .ok_and_then(move |store| Ok(store.get_address().to_owned()))
            .map_ok(move |address| StoreBuilder::new(region, address, timeout))
            .boxed()
    }

    fn region_for_key(&self, key: &Key) -> BoxFuture<'static, Result<Region>> {
        let key: &[u8] = key.into();
        self.pd.clone().get_region(key.to_owned()).boxed()
    }

    fn region_for_id(&self, id: RegionId) -> BoxFuture<'static, Result<Region>> {
        self.pd.clone().get_region_by_id(id).boxed()
    }

    fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>> {
        self.pd.clone().get_timestamp().boxed()
    }
}

impl PdRpcClient<Cluster> {
    pub async fn connect(config: &Config) -> Result<PdRpcClient> {
        PdRpcClient::new(config, |env, security_mgr| {
            RetryClient::connect(env, &config.pd_endpoints, security_mgr, config.timeout)
        })
        .await
    }
}

impl<Cl> PdRpcClient<Cl> {
    pub async fn new<PdFut, MakePd>(config: &Config, pd: MakePd) -> Result<PdRpcClient<Cl>>
    where
        PdFut: Future<Output = Result<RetryClient<Cl>>>,
        MakePd: FnOnce(Arc<Environment>, Arc<SecurityManager>) -> PdFut,
    {
        let env = Arc::new(
            EnvBuilder::new()
                .cq_count(CQ_COUNT)
                .name_prefix(thread_name!(CLIENT_PREFIX))
                .build(),
        );
        let security_mgr = Arc::new(
            if let (Some(ca_path), Some(cert_path), Some(key_path)) =
                (&config.ca_path, &config.cert_path, &config.key_path)
            {
                SecurityManager::load(ca_path, cert_path, key_path)?
            } else {
                SecurityManager::default()
            },
        );

        let pd = Arc::new(pd(env.clone(), security_mgr.clone()).await?);
        Ok(PdRpcClient {
            pd,
            timeout: config.timeout,
        })
    }
}
