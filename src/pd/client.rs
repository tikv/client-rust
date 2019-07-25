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
use grpcio::EnvBuilder;

use crate::{
    compat::{stream_fn, ClientFutureExt},
    kv::BoundRange,
    kv_client::{KvClient, KvConnect, Store, TikvConnect},
    pd::{Region, RegionId, RetryClient},
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
            .and_then(move |region| self.clone().map_region_to_store(region))
            .boxed()
    }

    fn store_for_id(
        self: Arc<Self>,
        id: RegionId,
    ) -> BoxFuture<'static, Result<Store<Self::KvClient>>> {
        self.region_for_id(id)
            .and_then(move |region| self.clone().map_region_to_store(region).boxed())
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
                    let mut grouped = Vec::new();
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
}

/// This client converts requests for the logical TiKV cluster into requests
/// for a single TiKV store using PD and internal logic.
pub struct PdRpcClient<KvC: KvConnect + Send + Sync + 'static = TikvConnect> {
    pd: Arc<RetryClient>,
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
        self.pd.clone().get_region(key.into()).boxed()
    }

    fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>> {
        self.pd.clone().get_timestamp()
    }
}

impl PdRpcClient<TikvConnect> {
    pub fn connect(config: &Config) -> Result<PdRpcClient> {
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

        let pd = Arc::new(RetryClient::connect(
            env.clone(),
            &config.pd_endpoints,
            security_mgr.clone(),
            config.timeout,
        )?);
        let kv_client_cache = Default::default();
        let kv_connect = TikvConnect::new(env, security_mgr);
        Ok(PdRpcClient {
            pd,
            kv_client_cache,
            kv_connect,
            timeout: config.timeout,
        })
    }
}

impl<KvC: KvConnect + Send + Sync + 'static> PdRpcClient<KvC> {
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
mod test {
    use super::*;
    use crate::raw::RawRequest;
    use crate::Error;

    // use futures::executor;
    use futures::future::{ready, BoxFuture};
    use grpcio::CallOption;
    use kvproto::metapb;

    struct MockKvClient;

    impl KvClient for MockKvClient {
        fn dispatch<T: RawRequest>(
            &self,
            _request: &T::RpcRequest,
            _opt: CallOption,
        ) -> BoxFuture<'static, Result<T::RpcResponse>> {
            unimplemented!();
        }
    }

    struct MockPdClient;

    impl PdClient for MockPdClient {
        type KvClient = MockKvClient;

        fn map_region_to_store(
            self: Arc<Self>,
            _region: Region,
        ) -> BoxFuture<'static, Result<Store<Self::KvClient>>> {
            unimplemented!()
        }

        fn region_for_key(&self, key: &Key) -> BoxFuture<'static, Result<Region>> {
            let bytes: &[_] = key.into();
            let region = if bytes.len() <= 1 {
                Self::region1()
            } else {
                Self::region2()
            };

            Box::pin(ready(Ok(region)))
        }
        fn region_for_id(&self, id: RegionId) -> BoxFuture<'static, Result<Region>> {
            let result = match id {
                1 => Ok(Self::region1()),
                2 => Ok(Self::region2()),
                _ => Err(Error::region_not_found(id, None)),
            };

            Box::pin(ready(result))
        }

        fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>> {
            unimplemented!()
        }
    }

    // fn get_store(self: Arc<Self>, id: StoreId) -> BoxFuture<'static, Result<metapb::Store>> {
    //     let mut result = metapb::Store::default();
    //     result.set_address(format!("store-address-{}", id));
    //     Box::pin(ready(Ok(result)))
    // }

    impl MockPdClient {
        fn region1() -> Region {
            let mut region = Region::default();
            region.region.id = 1;
            region.region.set_start_key(vec![0]);
            region.region.set_end_key(vec![4]);

            let mut leader = metapb::Peer::default();
            leader.store_id = 41;
            region.leader = Some(leader);

            region
        }

        fn region2() -> Region {
            let mut region = Region::default();
            region.region.id = 2;
            region.region.set_start_key(vec![4]);
            region.region.set_end_key(vec![8]);

            let mut leader = metapb::Peer::default();
            leader.store_id = 42;
            region.leader = Some(leader);

            region
        }
    }

    // #[test]
    // fn test_kv_client() {
    //     let client = MockPdClient;
    //     let addr1 = "foo";
    //     let addr2 = "bar";

    //     let kv1 = client.kv_client(&addr1).unwrap();
    //     let kv2 = client.kv_client(&addr2).unwrap();
    //     let kv3 = client.kv_client(&addr2).unwrap();
    //     assert!(&*kv1 as *const _ != &*kv2 as *const _);
    //     assert_eq!(&*kv2 as *const _, &*kv3 as *const _);
    // }

    // #[test]
    // fn test_group_keys_by_region() {
    //     let client = MockPdClient;

    //     let tasks: Vec<Key> = vec![
    //         vec![1].into(),
    //         vec![2].into(),
    //         vec![3].into(),
    //         vec![5, 1].into(),
    //         vec![5, 2].into(),
    //     ];

    //     let stream = Arc::new(client).group_keys_by_region(tasks.into_iter());
    //     let mut stream = executor::block_on_stream(stream);

    //     assert_eq!(
    //         stream.next().unwrap().unwrap().1,
    //         vec![vec![1].into(), vec![2].into(), vec![3].into()]
    //     );
    //     assert_eq!(
    //         stream.next().unwrap().unwrap().1,
    //         vec![vec![5, 1].into(), vec![5, 2].into()]
    //     );
    //     assert!(stream.next().is_none());
    // }

    // #[test]
    // fn test_raw_scan() {
    //     let client = Arc::new(mock_rpc_client());
    //     // TODO two regions with kvs across both
    //     let start: Key = vec![1].into();
    //     let end: Key = vec![3].into();
    //     let scan = client.raw_scan((start..end).into(), 10, true, None);
    //     let scan = executor::block_on(scan).unwrap();

    //     assert_eq!(scan.len(), 10);
    // }
}
