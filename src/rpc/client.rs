// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

// TODO: Remove this when txn is done.
#![allow(dead_code)]

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::{Arc, RwLock},
    time::Duration,
};

use derive_new::new;
use futures::future::{ready, Either};
use futures::prelude::*;
use grpcio::{EnvBuilder, Environment};
use kvproto::kvrpcpb;

use crate::{
    compat::{stream_fn, ClientFutureExt},
    kv::BoundRange,
    raw::ColumnFamily,
    rpc::{
        pd::{PdClient, Region, RegionId, Store, StoreId},
        security::SecurityManager,
        tikv::KvClient,
    },
    Config, Error, Key, KvPair, Result, Value,
};

const CQ_COUNT: usize = 1;
const CLIENT_PREFIX: &str = "tikv-client";

pub struct RpcClient {
    pd: Arc<PdClient>,
    tikv: Arc<RwLock<HashMap<String, Arc<KvClient>>>>,
    env: Arc<Environment>,
    security_mgr: Arc<SecurityManager>,
    timeout: Duration,
}

impl RpcClient {
    pub fn connect(config: &Config) -> Result<RpcClient> {
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

        let pd = Arc::new(PdClient::connect(
            Arc::clone(&env),
            &config.pd_endpoints,
            Arc::clone(&security_mgr),
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

    pub fn raw_get(
        self: Arc<Self>,
        key: Key,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<Option<Value>>> {
        self.clone()
            .raw(&key)
            .and_then(|context| context.client.raw_get(context.region, cf, key))
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
                            .raw_batch_get(context.region, cf, keys.into_iter())
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
                    .and_then(|context| context.client.raw_put(context.region, cf, key, value)),
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
                        .and_then(|context| context.client.raw_batch_put(context.region, cf, keys))
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
            .and_then(|context| context.client.raw_delete(context.region, cf, key))
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
                    .and_then(|context| context.client.raw_batch_delete(context.region, cf, keys))
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
                let (start_key, end_key) = context.region.range();
                Either::Right(
                    context
                        .client
                        .raw_scan(
                            context.region,
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
            let (start_key, end_key) = context.region.range();
            context
                .client
                .raw_delete_range(context.region, cf.clone(), start_key, end_key)
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
            Either::Left(
                self.clone()
                    .get_region(&start_key)
                    .and_then(move |location| {
                        this.raw_from_id(location.id()).map_ok(move |context| {
                            let region_end = context.region.end_key();
                            if end_key.map(|x| x < region_end).unwrap_or(false)
                                || region_end.is_empty()
                            {
                                return Some((None, context));
                            }
                            Some((Some(region_end), context))
                        })
                    }),
            )
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
                Either::Left(
                    self.clone()
                        .get_region(tasks[0].key())
                        .map_ok(move |location| {
                            let ver_id = location.ver_id();
                            let mut grouped = Vec::new();
                            while let Some(task) = tasks.pop_front() {
                                if !location.contains(task.key()) {
                                    break;
                                }
                                grouped.push(task);
                            }
                            Some((tasks, (ver_id.id, grouped)))
                        }),
                )
            }
        })
    }

    fn load_store(&self, id: StoreId) -> impl Future<Output = Result<Store>> {
        info!("reload info for store {}", id);
        self.pd.get_store(id).map_ok(Into::into)
    }

    fn load_region_by_id(&self, id: RegionId) -> impl Future<Output = Result<Region>> {
        self.pd.get_region_by_id(id)
    }

    fn get_region(&self, key: &Key) -> impl Future<Output = Result<Region>> {
        self.pd.get_region(key.into())
    }

    fn kv_client(&self, context: &RegionContext) -> Result<Arc<KvClient>> {
        if let Some(conn) = self.tikv.read().unwrap().get(context.address()) {
            return Ok(Arc::clone(conn));
        };
        info!("connect to tikv endpoint: {:?}", context.address());
        let tikv = self.tikv.clone();
        KvClient::connect(
            Arc::clone(&self.env),
            context.address(),
            &self.security_mgr,
            self.timeout,
        )
        .map(Arc::new)
        .map(|c| {
            tikv.write()
                .unwrap()
                .insert(context.address().to_owned(), Arc::clone(&c));
            c
        })
    }

    fn region_context_for_key(
        self: Arc<Self>,
        key: &Key,
    ) -> impl Future<Output = Result<RegionContext>> {
        let region = self.get_region(key);
        self.map_region_to_context(region)
    }

    fn map_region_context_to_raw(
        self: Arc<Self>,
        region_ctx: impl Future<Output = Result<RegionContext>>,
    ) -> impl Future<Output = Result<RawContext>> {
        region_ctx.ok_and_then(move |region_ctx| {
            self.kv_client(&region_ctx)
                .map(|client| RawContext::new(region_ctx, client))
        })
    }

    fn map_region_to_context(
        self: Arc<Self>,
        region: impl Future<Output = Result<Region>>,
    ) -> impl Future<Output = Result<RegionContext>> {
        region.and_then(move |region| {
            let peer = region.peer().expect("leader must exist");
            let store_id = peer.get_store_id();
            self.load_store(store_id)
                .map_ok(|store| RegionContext { region, store })
        })
    }

    fn raw_from_id(self: Arc<Self>, id: RegionId) -> impl Future<Output = Result<RawContext>> {
        let region = self.clone().load_region_by_id(id);
        let region_ctx = self.clone().map_region_to_context(region);
        self.map_region_context_to_raw(region_ctx)
    }

    fn raw(self: Arc<Self>, key: &Key) -> impl Future<Output = Result<RawContext>> {
        let region_ctx = self.clone().region_context_for_key(key);
        self.map_region_context_to_raw(region_ctx)
    }

    fn txn(self: Arc<Self>, key: &Key) -> impl Future<Output = Result<TxnContext>> {
        self.region_context_for_key(key)
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

pub struct RegionContext {
    region: Region,
    store: Store,
}

impl RegionContext {
    fn address(&self) -> &str {
        self.store.get_address()
    }

    fn start_key(&self) -> Key {
        self.region.start_key().to_vec().into()
    }

    fn end_key(&self) -> Key {
        self.region.end_key().to_vec().into()
    }

    fn range(&self) -> (Key, Key) {
        (self.start_key(), self.end_key())
    }
}

impl From<RegionContext> for kvrpcpb::Context {
    fn from(mut ctx: RegionContext) -> kvrpcpb::Context {
        let mut kvctx = kvrpcpb::Context::default();
        kvctx.set_region_id(ctx.region.id());
        kvctx.set_region_epoch(ctx.region.region.take_region_epoch());
        kvctx.set_peer(ctx.region.peer().expect("leader must exist"));
        kvctx
    }
}

#[derive(new)]
pub struct RawContext {
    pub(super) region: RegionContext,
    pub(super) client: Arc<KvClient>,
}

#[derive(new)]
pub struct TxnContext {
    pub(super) region: RegionContext,
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
