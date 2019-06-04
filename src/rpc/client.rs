// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

// TODO: Remove this when txn is done.
#![allow(dead_code)]

use std::{
    collections::hash_map::{self, HashMap},
    fmt,
    sync::{Arc, RwLock},
    time::Duration,
};

use futures::future::{self, ready, Either, Future};
use futures::prelude::TryFutureExt;
use grpcio::{EnvBuilder, Environment};
use kvproto::kvrpcpb;
use log::*;

use crate::{
    compat::{loop_fn, Loop},
    raw::ColumnFamily,
    rpc::{
        pd::{PdClient, PdTimestamp, Region, RegionId, RegionVerId, Store, StoreId},
        security::SecurityManager,
        tikv::KvClient,
        util::HandyRwLock,
    },
    Config, Error, Key, KvPair, Result, Value,
};

const CQ_COUNT: usize = 1;
const CLIENT_PREFIX: &str = "tikv-client";

struct RpcClientInner {
    pd: Arc<PdClient>,
    tikv: Arc<RwLock<HashMap<String, Arc<KvClient>>>>,
    env: Arc<Environment>,
    security_mgr: Arc<SecurityManager>,
    timeout: Duration,
}

impl RpcClientInner {
    fn connect(config: &Config) -> Result<RpcClientInner> {
        let env = Arc::new(
            EnvBuilder::new()
                .cq_count(CQ_COUNT)
                .name_prefix(thd_name!(CLIENT_PREFIX))
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
        Ok(RpcClientInner {
            pd,
            tikv,
            env,
            security_mgr,
            timeout: config.timeout,
        })
    }

    fn get_all_stores(&self) -> impl Future<Output = Result<Vec<Store>>> {
        self.pd.get_all_stores()
    }

    fn get_store_by_id(&self, id: StoreId) -> impl Future<Output = Result<Store>> {
        self.pd.get_store(id)
    }

    fn get_region(&self, key: &[u8]) -> impl Future<Output = Result<Region>> {
        self.pd.get_region(key)
    }

    fn get_region_by_id(&self, id: RegionId) -> impl Future<Output = Result<Region>> {
        self.pd.get_region_by_id(id)
    }

    fn get_ts(&self) -> impl Future<Output = Result<PdTimestamp>> {
        self.pd.get_ts()
    }

    fn load_store(&self, id: StoreId) -> impl Future<Output = Result<Store>> {
        info!("reload info for store {}", id);
        self.pd.get_store(id).map_ok(Into::into)
    }

    fn load_region(&self, key: &Key) -> impl Future<Output = Result<Region>> {
        self.pd.get_region(key.as_ref())
    }

    fn load_region_by_id(&self, id: RegionId) -> impl Future<Output = Result<Region>> {
        self.pd.get_region_by_id(id)
    }

    fn locate_key(&self, key: &Key) -> impl Future<Output = Result<KeyLocation>> {
        self.load_region(key)
    }

    fn kv_client(&self, context: RegionContext) -> Result<(RegionContext, Arc<KvClient>)> {
        if let Some(conn) = self.tikv.rl().get(context.address()) {
            return Ok((context, Arc::clone(conn)));
        };
        info!("connect to tikv endpoint: {:?}", context.address());
        let tikv = Arc::clone(&self.tikv);
        KvClient::connect(
            Arc::clone(&self.env),
            context.address(),
            &self.security_mgr,
            self.timeout,
        )
        .map(Arc::new)
        .map(|c| {
            tikv.wl()
                .insert(context.address().to_owned(), Arc::clone(&c));
            (context, c)
        })
    }
}

pub struct RpcClient {
    inner: Arc<RpcClientInner>,
}

impl RpcClient {
    pub fn connect(config: &Config) -> Result<RpcClient> {
        Ok(RpcClient {
            inner: Arc::new(RpcClientInner::connect(config)?),
        })
    }

    fn group_tasks_by_region<Task>(
        &self,
        tasks: Vec<Task>,
    ) -> impl Future<Output = Result<GroupedTasks<Task>>>
    where
        Task: GroupingTask,
    {
        let result: Option<GroupedTasks<Task>> = None;
        let inner = self.inner();
        loop_fn((0, tasks, result), move |(mut index, tasks, mut result)| {
            if index == tasks.len() {
                Either::Left(future::ok(Loop::Break(result)))
            } else {
                let inner = Arc::clone(&inner);
                Either::Right(
                    inner
                        .locate_key(tasks[index].key())
                        .map_ok(move |location| {
                            while let Some(item) = tasks.get(index) {
                                if !location.contains(item.key()) {
                                    break;
                                }
                                let ver_id = location.ver_id();
                                let item = item.clone();
                                if let Some(ref mut grouped) = result {
                                    grouped.add(ver_id, item);
                                } else {
                                    result = Some(GroupedTasks::new(ver_id, item));
                                }
                                index += 1;
                            }
                            if index == tasks.len() {
                                Loop::Break(result)
                            } else {
                                Loop::Continue((index, tasks, result))
                            }
                        }),
                )
            }
        })
        .map_ok(|r| r.unwrap_or_default())
    }

    fn region_context(
        inner: Arc<RpcClientInner>,
        key: &Key,
    ) -> impl Future<Output = Result<(RegionContext, Arc<KvClient>)>> {
        let inner2 = Arc::clone(&inner);
        inner
            .locate_key(key)
            .and_then(move |location| {
                let peer = location.peer().expect("leader must exist");
                let store_id = peer.get_store_id();
                inner.load_store(store_id).map_ok(|store| RegionContext {
                    region: location,
                    store,
                })
            })
            .and_then(move |region| ready(inner2.kv_client(region)))
    }

    fn region_context_by_id(
        inner: Arc<RpcClientInner>,
        id: RegionId,
    ) -> impl Future<Output = Result<(RegionContext, Arc<KvClient>)>> {
        let inner2 = Arc::clone(&inner);
        inner
            .load_region_by_id(id)
            .and_then(move |region| {
                let peer = region.peer().expect("leader must exist");
                let store_id = peer.get_store_id();
                inner
                    .load_store(store_id)
                    .map_ok(|store| RegionContext { region, store })
            })
            .and_then(move |region| ready(inner2.kv_client(region)))
    }

    fn raw(
        inner: Arc<RpcClientInner>,
        key: &Key,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<RawContext>> {
        Self::region_context(inner, key)
            .map_ok(|(region, client)| RawContext::new(region, client, cf))
    }

    fn txn(inner: Arc<RpcClientInner>, key: &Key) -> impl Future<Output = Result<TxnContext>> {
        Self::region_context(inner, key).map_ok(|(region, _client)| TxnContext::new(region))
    }

    #[inline]
    fn inner(&self) -> Arc<RpcClientInner> {
        Arc::clone(&self.inner)
    }

    pub fn raw_get(
        &self,
        key: Key,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<Option<Value>>> {
        Self::raw(self.inner(), &key, cf)
            .and_then(|context| context.client().raw_get(context, key))
            .map_ok(|value| if value.is_empty() { None } else { Some(value) })
    }

    pub fn raw_batch_get(
        &self,
        keys: Vec<Key>,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<Vec<KvPair>>> {
        let inner = self.inner();
        self.group_tasks_by_region(keys)
            .and_then(move |task_groups| {
                let mut tasks = Vec::with_capacity(task_groups.len());
                for (region, keys) in task_groups.into_iter() {
                    let inner = Arc::clone(&inner);
                    let cf = cf.clone();
                    let task = Self::region_context_by_id(inner, region.id)
                        .map_ok(|(region, client)| RawContext::new(region, client, cf))
                        .and_then(|context| {
                            context.client().raw_batch_get(context, keys.into_iter())
                        });
                    tasks.push(task);
                }
                future::try_join_all(tasks)
            })
            .map_ok(|r| r.into_iter().flat_map(|a| a.into_iter()).collect())
    }

    pub fn raw_put(
        &self,
        key: Key,
        value: Value,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        if value.is_empty() {
            Either::Left(future::err(Error::empty_value()))
        } else {
            Either::Right(
                Self::raw(self.inner(), &key, cf)
                    .and_then(|context| context.client().raw_put(context, key, value)),
            )
        }
    }

    pub fn raw_batch_put(
        &self,
        pairs: Vec<KvPair>,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        if pairs.iter().any(|p| p.value().is_empty()) {
            Either::Left(future::err(Error::empty_value()))
        } else {
            let inner = self.inner();
            Either::Right(
                self.group_tasks_by_region(pairs)
                    .and_then(move |task_groups| {
                        let mut tasks = Vec::with_capacity(task_groups.len());
                        for (region, pairs) in task_groups.into_iter() {
                            let inner = Arc::clone(&inner);
                            let cf = cf.clone();
                            let task = Self::region_context_by_id(inner, region.id)
                                .map_ok(|(region, client)| RawContext::new(region, client, cf))
                                .and_then(|context| context.client().raw_batch_put(context, pairs));
                            tasks.push(task);
                        }
                        future::try_join_all(tasks)
                    })
                    .map_ok(|_| ()),
            )
        }
    }

    pub fn raw_delete(
        &self,
        key: Key,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        Self::raw(self.inner(), &key, cf)
            .and_then(|context| context.client().raw_delete(context, key))
    }

    pub fn raw_batch_delete(
        &self,
        keys: Vec<Key>,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        let inner = self.inner();
        self.group_tasks_by_region(keys)
            .and_then(move |task_groups| {
                let mut tasks = Vec::with_capacity(task_groups.len());
                for (region, keys) in task_groups.into_iter() {
                    let inner = Arc::clone(&inner);
                    let cf = cf.clone();
                    let task = Self::region_context_by_id(inner, region.id)
                        .map_ok(|(region, client)| RawContext::new(region, client, cf))
                        .and_then(|context| context.client().raw_batch_delete(context, keys));
                    tasks.push(task);
                }
                future::try_join_all(tasks)
            })
            .map_ok(|_| ())
    }

    pub fn raw_scan(
        &self,
        range: (Key, Option<Key>),
        limit: u32,
        key_only: bool,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<Vec<KvPair>>> {
        struct State {
            limit: u32,
            key_only: bool,
            cf: Option<ColumnFamily>,
        };
        let scan: ScanRegionsContext<Vec<KvPair>, State> = ScanRegionsContext::new(
            range,
            State {
                limit,
                key_only,
                cf,
            },
        );
        let inner = Arc::clone(&self.inner);
        loop_fn((inner, scan), |(inner, scan)| {
            inner.locate_key(scan.start_key()).and_then(|location| {
                let region = location;
                let cf = scan.state.cf.clone();
                Self::region_context_by_id(Arc::clone(&inner), region.id())
                    .map_ok(|(region, client)| {
                        (scan, region.range(), RawContext::new(region, client, cf))
                    })
                    .and_then(|(mut scan, region_range, context)| {
                        let (start_key, end_key) = scan.range();
                        context
                            .client()
                            .raw_scan(
                                context,
                                start_key,
                                end_key,
                                scan.state.limit,
                                scan.state.key_only,
                            )
                            .map_ok(|pairs| (scan, region_range, pairs))
                    })
                    .map_ok(|(mut scan, region_range, mut pairs)| {
                        let limit = scan.state.limit;
                        scan.result_mut().append(&mut pairs);
                        if scan.result().len() as u32 >= limit {
                            Loop::Break(scan.into_inner())
                        } else {
                            match scan.next(region_range) {
                                ScanRegionsStatus::Continue => Loop::Continue((inner, scan)),
                                ScanRegionsStatus::Break => Loop::Break(scan.into_inner()),
                            }
                        }
                    })
            })
        })
    }

    pub fn raw_batch_scan(
        &self,
        ranges: Vec<(Key, Option<Key>)>,
        _each_limit: u32,
        _key_only: bool,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<Vec<KvPair>>> {
        drop(ranges);
        drop(cf);
        future::err(Error::unimplemented())
    }

    pub fn raw_delete_range(
        &self,
        range: (Key, Option<Key>),
        cf: Option<ColumnFamily>,
    ) -> impl Future<Output = Result<()>> {
        let scan: ScanRegionsContext<(), Option<ColumnFamily>> = ScanRegionsContext::new(range, cf);
        let inner = Arc::clone(&self.inner);
        loop_fn((inner, scan), |(inner, scan)| {
            inner.locate_key(scan.start_key()).and_then(|location| {
                let region = location;
                let cf = scan.state.clone();
                Self::region_context_by_id(Arc::clone(&inner), region.id())
                    .map_ok(|(region, client)| {
                        (scan, region.range(), RawContext::new(region, client, cf))
                    })
                    .and_then(|(mut scan, region_range, context)| {
                        let (start_key, end_key) = scan.range();
                        let start_key = start_key.expect("start key must be specified");
                        let end_key = end_key.expect("end key must be specified");
                        context
                            .client()
                            .raw_delete_range(context, start_key, end_key)
                            .map_ok(|_| (scan, region_range))
                    })
                    .map_ok(|(mut scan, region_range)| match scan.next(region_range) {
                        ScanRegionsStatus::Continue => Loop::Continue((inner, scan)),
                        ScanRegionsStatus::Break => Loop::Break(()),
                    })
            })
        })
    }
}

impl fmt::Debug for RpcClient {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("tikv-client")
            .field("pd", &self.inner.pd)
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

pub struct RawContext {
    region: RegionContext,
    client: Arc<KvClient>,
    cf: Option<ColumnFamily>,
}

impl RawContext {
    fn new(region: RegionContext, client: Arc<KvClient>, cf: Option<ColumnFamily>) -> Self {
        RawContext { region, client, cf }
    }

    fn client(&self) -> Arc<KvClient> {
        Arc::clone(&self.client)
    }

    pub fn into_inner(self) -> (RegionContext, Option<ColumnFamily>) {
        (self.region, self.cf)
    }
}

pub struct TxnContext {
    region: RegionContext,
}

impl TxnContext {
    fn new(region: RegionContext) -> Self {
        TxnContext { region }
    }

    pub fn into_inner(self) -> RegionContext {
        self.region
    }
}

type KeyLocation = Region;

trait GroupingTask: Clone + Default + Sized {
    fn key(&self) -> &Key;
}

#[derive(Default)]
struct GroupedTasks<Task: GroupingTask>(HashMap<RegionVerId, Vec<Task>>, RegionVerId);

impl<Task: GroupingTask> GroupedTasks<Task> {
    fn new(ver_id: RegionVerId, task: Task) -> Self {
        let mut map = HashMap::with_capacity(1);
        map.insert(ver_id.clone(), vec![task]);
        GroupedTasks(map, ver_id)
    }

    #[inline]
    fn add(&mut self, ver_id: RegionVerId, task: Task) {
        self.0
            .entry(ver_id)
            .or_insert_with(|| Vec::with_capacity(1))
            .push(task)
    }

    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<Task: GroupingTask> IntoIterator for GroupedTasks<Task> {
    type Item = (RegionVerId, Vec<Task>);
    type IntoIter = hash_map::IntoIter<RegionVerId, Vec<Task>>;

    fn into_iter(self) -> hash_map::IntoIter<RegionVerId, Vec<Task>> {
        self.0.into_iter()
    }
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

enum ScanRegionsStatus {
    Continue,
    Break,
}

struct ScanRegionsContext<Res, State>
where
    Res: Default,
    State: Sized,
{
    start_key: Option<Key>,
    end_key: Option<Key>,
    result: Res,
    state: State,
}

impl<Res, State> ScanRegionsContext<Res, State>
where
    Res: Default,
    State: Sized,
{
    fn new(range: (Key, Option<Key>), state: State) -> Self {
        ScanRegionsContext {
            start_key: Some(range.0),
            end_key: range.1,
            result: Res::default(),
            state,
        }
    }

    fn range(&mut self) -> (Option<Key>, Option<Key>) {
        (self.start_key.take(), self.end_key.clone())
    }

    fn start_key(&self) -> &Key {
        self.start_key.as_ref().unwrap()
    }

    fn end_key(&self) -> Option<&Key> {
        self.end_key.as_ref()
    }

    fn next(&mut self, region_range: (Key, Key)) -> ScanRegionsStatus {
        {
            let region_end = &region_range.1;
            if self.end_key().map(|x| x < region_end).unwrap_or(false) || region_end.is_empty() {
                return ScanRegionsStatus::Break;
            }
        }
        self.start_key = Some(region_range.1);
        ScanRegionsStatus::Continue
    }

    fn into_inner(self) -> Res {
        self.result
    }

    fn result_mut(&mut self) -> &mut Res {
        &mut self.result
    }

    fn result(&self) -> &Res {
        &self.result
    }
}
