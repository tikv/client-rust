// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

// TODO: Remove this when txn is done.
#![allow(dead_code)]

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::{Arc, RwLock},
    time::Duration,
};

use futures::future::{ready, Either};
use futures::prelude::*;
use grpcio::{EnvBuilder, Environment};
use kvproto::metapb;

use crate::{
    compat::{stream_fn, ClientFutureExt},
    kv::BoundRange,
    raw::ColumnFamily,
    rpc::{
        pd::{PdClient, Region, RegionId, RetryClient, StoreId, Timestamp},
        security::SecurityManager,
        tikv::KvClient,
        Address, RawContext, Store, TxnContext,
    },
    Config, Error, Key, KvPair, Result, Value,
};

const CQ_COUNT: usize = 1;
const CLIENT_PREFIX: &str = "tikv-client";

pub struct RpcClient<PdC: PdClient = RetryClient> {
    pd: Arc<PdC>,
    tikv: Arc<RwLock<HashMap<String, Arc<KvClient>>>>,
    env: Arc<Environment>,
    security_mgr: Arc<SecurityManager>,
    timeout: Duration,
}

impl RpcClient<RetryClient> {
    pub fn connect(config: &Config) -> Result<RpcClient<RetryClient>> {
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
        let tikv = Default::default();
        Ok(RpcClient {
            pd,
            tikv,
            env,
            security_mgr,
            timeout: config.timeout,
        })
    }
}

impl<PdC: PdClient> RpcClient<PdC> {
    pub fn raw_get(
        self: Arc<Self>,
        key: Key,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<Option<Value>>> {
        self.clone()
            .raw(&key)
            .and_then(|context| context.client.raw_get(context.store, cf, key))
            .map_ok(|value| if value.is_empty() { None } else { Some(value) })
    }

    pub fn raw_batch_get(
        self: Arc<Self>,
        keys: Vec<Key>,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<Vec<KvPair>>> {
        self.clone().group_tasks_by_region(keys).try_fold(
            Vec::new(),
            move |mut result, (region_id, keys)| {
                let cf = cf.clone();
                self.clone()
                    .raw_from_id(region_id)
                    .and_then(|context| {
                        context
                            .client
                            .raw_batch_get(context.store, cf, keys.into_iter())
                    })
                    .map_ok(move |mut pairs| {
                        result.append(&mut pairs);
                        result
                    })
            },
        )
    }

    pub fn raw_put(
        self: Arc<Self>,
        key: Key,
        value: Value,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        if value.is_empty() {
            future::Either::Left(future::err(Error::empty_value()))
        } else {
            future::Either::Right(
                self.raw(&key)
                    .and_then(|context| context.client.raw_put(context.store, cf, key, value)),
            )
        }
    }

    pub fn raw_batch_put(
        self: Arc<Self>,
        pairs: Vec<KvPair>,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        if pairs.iter().any(|p| p.value().is_empty()) {
            future::Either::Left(future::err(Error::empty_value()))
        } else {
            future::Either::Right(self.clone().group_tasks_by_region(pairs).try_for_each(
                move |(region_id, keys)| {
                    let cf = cf.clone();
                    self.clone()
                        .raw_from_id(region_id)
                        .and_then(|context| context.client.raw_batch_put(context.store, cf, keys))
                },
            ))
        }
    }

    pub fn raw_delete(
        self: Arc<Self>,
        key: Key,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        self.raw(&key)
            .and_then(|context| context.client.raw_delete(context.store, cf, key))
    }

    pub fn raw_batch_delete(
        self: Arc<Self>,
        keys: Vec<Key>,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        self.clone()
            .group_tasks_by_region(keys)
            .try_for_each(move |(region_id, keys)| {
                let cf = cf.clone();
                self.clone()
                    .raw_from_id(region_id)
                    .and_then(|context| context.client.raw_batch_delete(context.store, cf, keys))
            })
    }

    pub fn raw_scan(
        self: Arc<Self>,
        range: BoundRange,
        limit: u32,
        key_only: bool,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<Vec<KvPair>>> {
        self.regions_for_range(range)
            .try_fold(Vec::new(), move |mut result, context| {
                if result.len() as u32 >= limit {
                    // Skip any more scans if we've hit the limit already.
                    return Either::Left(ready(Ok(result)));
                }
                let (start_key, end_key) = context.store.range();
                Either::Right(
                    context
                        .client
                        .raw_scan(
                            context.store,
                            cf.clone(),
                            start_key,
                            Some(end_key),
                            limit,
                            key_only,
                        )
                        .map_ok(move |mut pairs| {
                            result.append(&mut pairs);
                            result
                        }),
                )
            })
    }

    pub fn raw_delete_range(
        self: Arc<Self>,
        range: BoundRange,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        self.regions_for_range(range).try_for_each(move |context| {
            let (start_key, end_key) = context.store.range();
            context
                .client
                .raw_delete_range(context.store, cf.clone(), start_key, end_key)
        })
    }

    pub fn raw_batch_scan(
        self: Arc<Self>,
        _ranges: Vec<BoundRange>,
        _each_limit: u32,
        _key_only: bool,
        _cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<Vec<KvPair>>> {
        future::err(Error::unimplemented())
    }

    pub fn get_timestamp(self: Arc<Self>) -> impl Future<Output = Result<Timestamp>> {
        Arc::clone(&self.pd).get_timestamp()
    }

    // Returns a Steam which iterates over the contexts for each region covered by range.
    fn regions_for_range(
        self: Arc<Self>,
        range: BoundRange,
    ) -> impl Stream<Item = Result<RawContext>> {
        let (start_key, end_key) = range.into_keys();
        stream_fn(Some(start_key), move |start_key| {
            let start_key = match start_key {
                None => return Either::Right(ready(Ok(None))),
                Some(sk) => sk,
            };
            let end_key = end_key.clone();
            let this = self.clone();
            Either::Left(self.get_region(&start_key).and_then(move |location| {
                this.raw_from_id(location.id()).map_ok(move |context| {
                    let region_end = context.store.end_key();
                    if end_key.map(|x| x < region_end).unwrap_or(false) || region_end.is_empty() {
                        return Some((None, context));
                    }
                    Some((Some(region_end), context))
                })
            }))
        })
    }

    fn group_tasks_by_region<Task>(
        self: Arc<Self>,
        tasks: Vec<Task>,
    ) -> impl Stream<Item = Result<(RegionId, Vec<Task>)>>
    where
        Task: GroupingTask,
    {
        let tasks: VecDeque<Task> = tasks.into();

        stream_fn(tasks, move |mut tasks| {
            if tasks.is_empty() {
                Either::Right(ready(Ok(None)))
            } else {
                Either::Left(self.get_region(tasks[0].key()).map_ok(move |region| {
                    let id = region.id();
                    let mut grouped = Vec::new();
                    while let Some(task) = tasks.pop_front() {
                        if !region.contains(task.key()) {
                            tasks.push_front(task);
                            break;
                        }
                        grouped.push(task);
                    }
                    Some((tasks, (id, grouped)))
                }))
            }
        })
    }

    fn load_store(&self, id: StoreId) -> impl Future<Output = Result<metapb::Store>> {
        info!("reload info for store {}", id);
        self.pd.clone().get_store(id)
    }

    fn load_region_by_id(&self, id: RegionId) -> impl Future<Output = Result<Region>> {
        self.pd.clone().get_region_by_id(id)
    }

    fn get_region(&self, key: &Key) -> impl Future<Output = Result<Region>> {
        self.pd.clone().get_region(key.into())
    }

    fn kv_client(&self, a: &impl Address) -> Result<Arc<KvClient>> {
        let address = a.address();
        if let Some(client) = self.tikv.read().unwrap().get(address) {
            return Ok(client.clone());
        };
        info!("connect to tikv endpoint: {:?}", address);
        let tikv = self.tikv.clone();
        KvClient::connect(self.env.clone(), address, &self.security_mgr, self.timeout)
            .map(Arc::new)
            .map(|c| {
                tikv.write().unwrap().insert(address.to_owned(), c.clone());
                c
            })
    }

    fn store_for_key(self: Arc<Self>, key: &Key) -> impl Future<Output = Result<Store>> {
        let region = self.get_region(key);
        self.map_region_to_store(region)
    }

    fn map_store_to_raw_context(
        self: Arc<Self>,
        region_ctx: impl Future<Output = Result<Store>>,
    ) -> impl Future<Output = Result<RawContext>> {
        region_ctx.ok_and_then(move |region_ctx| {
            self.kv_client(&region_ctx)
                .map(|client| RawContext::new(region_ctx, client))
        })
    }

    fn map_region_to_store(
        self: Arc<Self>,
        region: impl Future<Output = Result<Region>>,
    ) -> impl Future<Output = Result<Store>> {
        region.and_then(move |region| {
            let peer = region.peer().expect("leader must exist");
            let store_id = peer.get_store_id();
            self.load_store(store_id)
                .map_ok(|store| Store { region, store })
        })
    }

    fn raw_from_id(self: Arc<Self>, id: RegionId) -> impl Future<Output = Result<RawContext>> {
        let region = self.clone().load_region_by_id(id);
        let store = self.clone().map_region_to_store(region);
        self.map_store_to_raw_context(store)
    }

    fn raw(self: Arc<Self>, key: &Key) -> impl Future<Output = Result<RawContext>> {
        let store = self.clone().store_for_key(key);
        self.map_store_to_raw_context(store)
    }

    fn txn(self: Arc<Self>, key: &Key) -> impl Future<Output = Result<TxnContext>> {
        self.store_for_key(key)
            .map_ok(|region_ctx| TxnContext::new(region_ctx))
    }
}

impl fmt::Debug for RpcClient {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("tikv-client")
            .field("pd", &self.pd)
            .finish()
    }
}

trait GroupingTask: Clone + Default + Sized {
    fn key(&self) -> &Key;
}

impl GroupingTask for Key {
    fn key(&self) -> &Key {
        self
    }
}

impl GroupingTask for KvPair {
    fn key(&self) -> &Key {
        self.key()
    }
}

impl GroupingTask for (Key, Option<Key>) {
    fn key(&self) -> &Key {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rpc::pd::Timestamp;
    use futures::executor;
    use futures::future::{ready, BoxFuture};

    struct MockPdClient {}

    impl PdClient for MockPdClient {
        fn connect(
            _env: Arc<Environment>,
            _endpoints: &[String],
            _security_mgr: Arc<SecurityManager>,
            _timeout: Duration,
        ) -> Result<Self> {
            Ok(MockPdClient {})
        }

        fn get_region(self: Arc<Self>, key: &[u8]) -> BoxFuture<'static, Result<Region>> {
            let region = if key.len() <= 1 {
                let mut region = Region::default();
                region.region.set_start_key(vec![0]);
                region.region.set_end_key(vec![4]);
                region
            } else {
                let mut region = Region::default();
                region.region.set_start_key(vec![4]);
                region.region.set_end_key(vec![8]);
                region
            };

            Box::pin(ready(Ok(region)))
        }

        fn get_region_by_id(self: Arc<Self>, _id: RegionId) -> BoxFuture<'static, Result<Region>> {
            unimplemented!();
        }

        fn get_store(self: Arc<Self>, _id: StoreId) -> BoxFuture<'static, Result<metapb::Store>> {
            unimplemented!();
        }

        fn get_all_stores(self: Arc<Self>) -> BoxFuture<'static, Result<Vec<metapb::Store>>> {
            unimplemented!();
        }

        fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>> {
            unimplemented!();
        }
    }

    fn mock_rpc_client() -> RpcClient<MockPdClient> {
        let config = Config::default();
        let env = Arc::new(
            EnvBuilder::new()
                .cq_count(CQ_COUNT)
                .name_prefix(thread_name!(CLIENT_PREFIX))
                .build(),
        );
        RpcClient {
            pd: Arc::new(MockPdClient {}),
            tikv: Default::default(),
            env,
            security_mgr: Arc::new(SecurityManager::default()),
            timeout: config.timeout,
        }
    }

    #[test]
    fn test_kv_client() {
        let client = mock_rpc_client();
        let addr1 = "foo";
        let addr2 = "bar";

        let kv1 = client.kv_client(&addr1).unwrap();
        let kv2 = client.kv_client(&addr2).unwrap();
        let kv3 = client.kv_client(&addr2).unwrap();
        assert!(&*kv1 as *const _ != &*kv2 as *const _);
        assert_eq!(&*kv2 as *const _, &*kv3 as *const _);
    }

    #[test]
    fn test_group_tasks_by_region() {
        let client = mock_rpc_client();

        let tasks: Vec<Key> = vec![
            vec![1].into(),
            vec![2].into(),
            vec![3].into(),
            vec![5, 1].into(),
            vec![5, 2].into(),
        ];

        let stream = Arc::new(client).group_tasks_by_region(tasks);
        let mut stream = executor::block_on_stream(stream);

        assert_eq!(
            stream.next().unwrap().unwrap().1,
            vec![vec![1].into(), vec![2].into(), vec![3].into()]
        );
        assert_eq!(
            stream.next().unwrap().unwrap().1,
            vec![vec![5, 1].into(), vec![5, 2].into()]
        );
        assert!(stream.next().is_none());
    }
}
