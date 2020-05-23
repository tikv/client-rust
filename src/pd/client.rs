// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use futures::future::BoxFuture;
use futures::future::{ready, Either};
use futures::prelude::*;
use futures::stream::BoxStream;
use grpcio::{EnvBuilder, Environment};

use crate::{
    compat::{stream_fn, ClientFutureExt},
    kv::BoundRange,
    kv_client::{KvClient, KvConnect, Store, TikvConnect},
    pd::{cluster::Cluster, Region, RegionId, RetryClient},
    security::SecurityManager,
    transaction::Timestamp,
    Config, Key, Result,
};

const CQ_COUNT: usize = 1;
const CLIENT_PREFIX: &str = "tikv-client";

pub trait PdClient: Send + Sync + 'static {
    type KvClient: KvClient + Send + Sync + 'static;

    fn map_region_to_store(
        self: Arc<Self>,
        region: Region,
    ) -> BoxFuture<'static, Result<Store<Self::KvClient>>>;

    fn region_for_key(&self, key: &Key) -> BoxFuture<'static, Result<Region>>;

    fn region_for_id(&self, id: RegionId) -> BoxFuture<'static, Result<Region>>;

    fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>>;

    fn store_for_key(
        self: Arc<Self>,
        key: &Key,
    ) -> BoxFuture<'static, Result<Store<Self::KvClient>>> {
        self.region_for_key(key)
            .and_then(move |region| self.map_region_to_store(region))
            .boxed()
    }

    fn store_for_id(
        self: Arc<Self>,
        id: RegionId,
    ) -> BoxFuture<'static, Result<Store<Self::KvClient>>> {
        self.region_for_id(id)
            .and_then(move |region| self.map_region_to_store(region).boxed())
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
    ) -> BoxStream<'static, Result<Store<Self::KvClient>>> {
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
                this.map_region_to_store(region).map_ok(move |store| {
                    if end_key.map(|x| x < region_end).unwrap_or(false) || region_end.is_empty() {
                        return Some((None, store));
                    }
                    Some((Some(region_end), store))
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
pub struct PdRpcClient<KvC: KvConnect + Send + Sync + 'static = TikvConnect, Cl = Cluster> {
    pd: Arc<RetryClient<Cl>>,
    kv_connect: KvC,
    kv_client_cache: Arc<RwLock<HashMap<String, KvC::KvClient>>>,
    timeout: Duration,
}

impl<KvC: KvConnect + Send + Sync + 'static> PdClient for PdRpcClient<KvC> {
    type KvClient = KvC::KvClient;

    fn map_region_to_store(
        self: Arc<Self>,
        region: Region,
    ) -> BoxFuture<'static, Result<Store<KvC::KvClient>>> {
        let timeout = self.timeout;
        // FIXME propagate this error instead of using `unwrap`.
        let store_id = region.get_store_id().unwrap();
        self.pd
            .clone()
            .get_store(store_id)
            .ok_and_then(move |store| self.kv_client(store.get_address()))
            .map_ok(move |kv_client| Store::new(region, kv_client, timeout))
            .boxed()
    }

    fn region_for_id(&self, id: RegionId) -> BoxFuture<'static, Result<Region>> {
        self.pd.clone().get_region_by_id(id).boxed()
    }

    fn region_for_key(&self, key: &Key) -> BoxFuture<'static, Result<Region>> {
        let key: &[u8] = key.into();
        self.pd.clone().get_region(key.to_owned()).boxed()
    }

    fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>> {
        self.pd.clone().get_timestamp().boxed()
    }
}

impl PdRpcClient<TikvConnect, Cluster> {
    pub async fn connect(config: &Config) -> Result<PdRpcClient> {
        PdRpcClient::new(
            config,
            |env, security_mgr| TikvConnect::new(env, security_mgr),
            |env, security_mgr| {
                RetryClient::connect(env, &config.pd_endpoints, security_mgr, config.timeout)
            },
        )
        .await
    }
}

impl<KvC: KvConnect + Send + Sync + 'static, Cl> PdRpcClient<KvC, Cl> {
    pub async fn new<PdFut, MakeKvC, MakePd>(
        config: &Config,
        kv_connect: MakeKvC,
        pd: MakePd,
    ) -> Result<PdRpcClient<KvC, Cl>>
    where
        PdFut: Future<Output = Result<RetryClient<Cl>>>,
        MakeKvC: FnOnce(Arc<Environment>, Arc<SecurityManager>) -> KvC,
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
        let kv_client_cache = Default::default();
        Ok(PdRpcClient {
            pd,
            kv_client_cache,
            kv_connect: kv_connect(env, security_mgr),
            timeout: config.timeout,
        })
    }

    fn kv_client(&self, address: &str) -> Result<KvC::KvClient> {
        if let Some(client) = self.kv_client_cache.read().unwrap().get(address) {
            return Ok(client.clone());
        };
        info!("connect to tikv endpoint: {:?}", address);
        self.kv_connect.connect(address).map(|client| {
            self.kv_client_cache
                .write()
                .unwrap()
                .insert(address.to_owned(), client.clone());
            client
        })
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::mock::*;

    use futures::executor;
    use futures::executor::block_on;

    #[test]
    fn test_kv_client_caching() {
        let client = block_on(pd_rpc_client());

        let addr1 = "foo";
        let addr2 = "bar";

        let kv1 = client.kv_client(&addr1).unwrap();
        let kv2 = client.kv_client(&addr2).unwrap();
        let kv3 = client.kv_client(&addr2).unwrap();
        assert!(kv1 != kv2);
        assert_eq!(kv2, kv3);
    }

    #[test]
    fn test_group_keys_by_region() {
        let client = MockPdClient;

        // FIXME This only works if the keys are in order of regions. Not sure if
        // that is a reasonable constraint.
        let tasks: Vec<Key> = vec![
            vec![1].into(),
            vec![2].into(),
            vec![3].into(),
            vec![5, 2].into(),
            vec![12].into(),
            vec![11, 4].into(),
        ];

        let stream = Arc::new(client).group_keys_by_region(tasks.into_iter());
        let mut stream = executor::block_on_stream(stream);

        assert_eq!(
            stream.next().unwrap().unwrap().1,
            vec![
                vec![1].into(),
                vec![2].into(),
                vec![3].into(),
                vec![5, 2].into()
            ]
        );
        assert_eq!(
            stream.next().unwrap().unwrap().1,
            vec![vec![12].into(), vec![11, 4].into()]
        );
        assert!(stream.next().is_none());
    }

    #[test]
    fn test_stores_for_range() {
        let client = Arc::new(MockPdClient);
        let k1: Key = vec![1].into();
        let k2: Key = vec![5, 2].into();
        let k3: Key = vec![11, 4].into();
        let range1 = (k1, k2.clone()).into();
        let mut stream = executor::block_on_stream(client.clone().stores_for_range(range1));
        assert_eq!(stream.next().unwrap().unwrap().region.id(), 1);
        assert!(stream.next().is_none());

        let range2 = (k2, k3).into();
        let mut stream = executor::block_on_stream(client.stores_for_range(range2));
        assert_eq!(stream.next().unwrap().unwrap().region.id(), 1);
        assert_eq!(stream.next().unwrap().unwrap().region.id(), 2);
        assert!(stream.next().is_none());
    }

    #[test]
    fn test_group_ranges_by_region() {
        let client = Arc::new(MockPdClient);
        let k1: Key = vec![1].into();
        let k2: Key = vec![5, 2].into();
        let k3: Key = vec![11, 4].into();
        let k4: Key = vec![16, 4].into();
        let k_split: Key = vec![10].into();
        let range1 = (k1.clone(), k2.clone()).into();
        let range2 = (k1.clone(), k3.clone()).into();
        let range3 = (k2.clone(), k4.clone()).into();
        let ranges: Vec<BoundRange> = vec![range1, range2, range3];

        let mut stream = executor::block_on_stream(client.group_ranges_by_region(ranges));
        let ranges1 = stream.next().unwrap().unwrap();
        let ranges2 = stream.next().unwrap().unwrap();
        let ranges3 = stream.next().unwrap().unwrap();
        let ranges4 = stream.next().unwrap().unwrap();

        assert_eq!(ranges1.0, 1);
        assert_eq!(
            ranges1.1,
            vec![
                (k1.clone(), k2.clone()).into(),
                (k1.clone(), k_split.clone()).into()
            ] as Vec<BoundRange>
        );
        assert_eq!(ranges2.0, 2);
        assert_eq!(
            ranges2.1,
            vec![(k_split.clone(), k3.clone()).into()] as Vec<BoundRange>
        );
        assert_eq!(ranges3.0, 1);
        assert_eq!(
            ranges3.1,
            vec![(k2.clone(), k_split.clone()).into()] as Vec<BoundRange>
        );
        assert_eq!(ranges4.0, 2);
        assert_eq!(
            ranges4.1,
            vec![(k_split.clone(), k4.clone()).into()] as Vec<BoundRange>
        );
        assert!(stream.next().is_none());
    }
}
