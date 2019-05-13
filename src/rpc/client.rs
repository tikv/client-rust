// Copyright 2018 The TiKV Project Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

// TODO: Remove this when txn is done.
#![allow(dead_code)]

use std::{
    collections::HashMap,
    fmt,
    ops::Deref,
    sync::{Arc, RwLock},
    time::Duration,
};

use futures::{
    future::{self, loop_fn, Either, Loop},
    Future,
};
use grpcio::{EnvBuilder, Environment};
use kvproto::kvrpcpb;
use log::*;

use crate::{
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

    fn get_all_stores(&self) -> impl Future<Item = Vec<Store>, Error = Error> {
        self.pd.get_all_stores()
    }

    fn get_store_by_id(&self, id: StoreId) -> impl Future<Item = Store, Error = Error> {
        self.pd.get_store(id)
    }

    fn get_region(&self, key: &[u8]) -> impl Future<Item = Region, Error = Error> {
        self.pd.get_region(key)
    }

    fn get_region_by_id(&self, id: RegionId) -> impl Future<Item = Region, Error = Error> {
        self.pd.get_region_by_id(id)
    }

    fn get_ts(&self) -> impl Future<Item = PdTimestamp, Error = Error> {
        self.pd.get_ts()
    }

    fn load_store(&self, id: StoreId) -> impl Future<Item = Store, Error = Error> {
        info!("reload info for store {}", id);
        self.pd.get_store(id).map(Into::into)
    }

    fn load_region(&self, key: &Key) -> impl Future<Item = Region, Error = Error> {
        self.pd.get_region(key.as_ref())
    }

    fn load_region_by_id(&self, id: RegionId) -> impl Future<Item = Region, Error = Error> {
        self.pd.get_region_by_id(id)
    }

    fn locate_key(&self, key: &Key) -> impl Future<Item = KeyLocation, Error = Error> {
        self.load_region(key).map(KeyLocation::new)
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
    ) -> impl Future<Item = GroupedTasks<Task>, Error = Error>
    where
        Task: GroupingTask,
    {
        let result: Option<GroupedTasks<Task>> = None;
        let inner = self.inner();
        loop_fn((0, tasks, result), move |(mut index, tasks, mut result)| {
            if index == tasks.len() {
                Either::A(future::ok(Loop::Break(result)))
            } else {
                let inner = Arc::clone(&inner);
                Either::B(inner.locate_key(tasks[index].key()).map(move |location| {
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
                }))
            }
        })
        .map(|r| r.unwrap_or_default())
    }

    fn region_context(
        inner: Arc<RpcClientInner>,
        key: &Key,
    ) -> impl Future<Item = (RegionContext, Arc<KvClient>), Error = Error> {
        let inner2 = Arc::clone(&inner);
        inner
            .locate_key(key)
            .and_then(move |location| {
                let peer = location.peer().expect("leader must exist");
                let store_id = peer.get_store_id();
                inner.load_store(store_id).map(|store| RegionContext {
                    region: location.into_inner(),
                    store,
                })
            })
            .and_then(move |region| inner2.kv_client(region))
    }

    fn region_context_by_id(
        inner: Arc<RpcClientInner>,
        id: RegionId,
    ) -> impl Future<Item = (RegionContext, Arc<KvClient>), Error = Error> {
        let inner2 = Arc::clone(&inner);
        inner
            .load_region_by_id(id)
            .and_then(move |region| {
                let peer = region.peer().expect("leader must exist");
                let store_id = peer.get_store_id();
                inner
                    .load_store(store_id)
                    .map(|store| RegionContext { region, store })
            })
            .and_then(move |region| inner2.kv_client(region))
    }

    fn raw(
        inner: Arc<RpcClientInner>,
        key: &Key,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Item = RawContext, Error = Error> {
        Self::region_context(inner, key).map(|(region, client)| RawContext::new(region, client, cf))
    }

    fn txn(inner: Arc<RpcClientInner>, key: &Key) -> impl Future<Item = TxnContext, Error = Error> {
        Self::region_context(inner, key).map(|(region, _client)| TxnContext::new(region))
    }

    #[inline]
    fn inner(&self) -> Arc<RpcClientInner> {
        Arc::clone(&self.inner)
    }

    pub fn raw_get(
        &self,
        key: Key,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Item = Option<Value>, Error = Error> {
        Self::raw(self.inner(), &key, cf)
            .and_then(|context| context.client().raw_get(context, key))
            .map(|value| if value.is_empty() { None } else { Some(value) })
    }

    pub fn raw_batch_get(
        &self,
        keys: Vec<Key>,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Item = Vec<KvPair>, Error = Error> {
        let inner = self.inner();
        self.group_tasks_by_region(keys)
            .and_then(move |task_groups| {
                let task_groups = task_groups.into_inner();
                let mut tasks = Vec::with_capacity(task_groups.len());
                for (region, keys) in task_groups.into_iter() {
                    let inner = Arc::clone(&inner);
                    let cf = cf.clone();
                    let task = Self::region_context_by_id(inner, region.id)
                        .map(|(region, client)| RawContext::new(region, client, cf))
                        .and_then(|context| {
                            context.client().raw_batch_get(context, keys.into_iter())
                        });
                    tasks.push(task);
                }
                future::join_all(tasks)
            })
            .map(|r| r.into_iter().flat_map(|a| a.into_iter()).collect())
    }

    pub fn raw_put(
        &self,
        key: Key,
        value: Value,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Item = (), Error = Error> {
        if value.is_empty() {
            Either::A(future::err(Error::empty_value()))
        } else {
            Either::B(
                Self::raw(self.inner(), &key, cf)
                    .and_then(|context| context.client().raw_put(context, key, value)),
            )
        }
    }

    pub fn raw_batch_put(
        &self,
        pairs: Vec<KvPair>,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Item = (), Error = Error> {
        if pairs.iter().any(|p| p.value().is_empty()) {
            Either::A(future::err(Error::empty_value()))
        } else {
            let inner = self.inner();
            Either::B(
                self.group_tasks_by_region(pairs)
                    .and_then(move |task_groups| {
                        let task_groups = task_groups.into_inner();
                        let mut tasks = Vec::with_capacity(task_groups.len());
                        for (region, pairs) in task_groups.into_iter() {
                            let inner = Arc::clone(&inner);
                            let cf = cf.clone();
                            let task = Self::region_context_by_id(inner, region.id)
                                .map(|(region, client)| RawContext::new(region, client, cf))
                                .and_then(|context| context.client().raw_batch_put(context, pairs));
                            tasks.push(task);
                        }
                        future::join_all(tasks)
                    })
                    .map(|_| ()),
            )
        }
    }

    pub fn raw_delete(
        &self,
        key: Key,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Item = (), Error = Error> {
        Self::raw(self.inner(), &key, cf)
            .and_then(|context| context.client().raw_delete(context, key))
    }

    pub fn raw_batch_delete(
        &self,
        keys: Vec<Key>,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Item = (), Error = Error> {
        let inner = self.inner();
        self.group_tasks_by_region(keys)
            .and_then(move |task_groups| {
                let task_groups = task_groups.into_inner();
                let mut tasks = Vec::with_capacity(task_groups.len());
                for (region, keys) in task_groups.into_iter() {
                    let inner = Arc::clone(&inner);
                    let cf = cf.clone();
                    let task = Self::region_context_by_id(inner, region.id)
                        .map(|(region, client)| RawContext::new(region, client, cf))
                        .and_then(|context| context.client().raw_batch_delete(context, keys));
                    tasks.push(task);
                }
                future::join_all(tasks)
            })
            .map(|_| ())
    }

    pub fn raw_scan(
        &self,
        range: (Key, Option<Key>),
        limit: u32,
        key_only: bool,
        cf: Option<ColumnFamily>,
    ) -> impl Future<Item = Vec<KvPair>, Error = Error> {
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
                let region = location.into_inner();
                let cf = scan.cf.clone();
                Self::region_context_by_id(Arc::clone(&inner), region.id)
                    .map(|(region, client)| {
                        (scan, region.range(), RawContext::new(region, client, cf))
                    })
                    .and_then(|(mut scan, region_range, context)| {
                        let (start_key, end_key) = scan.range();
                        context
                            .client()
                            .raw_scan(context, start_key, end_key, scan.limit, scan.key_only)
                            .map(|pairs| (scan, region_range, pairs))
                    })
                    .map(|(mut scan, region_range, mut pairs)| {
                        let limit = scan.limit;
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
    ) -> impl Future<Item = Vec<KvPair>, Error = Error> {
        drop(ranges);
        drop(cf);
        future::err(Error::unimplemented())
    }

    pub fn raw_delete_range(
        &self,
        range: (Key, Option<Key>),
        cf: Option<ColumnFamily>,
    ) -> impl Future<Item = (), Error = Error> {
        let scan: ScanRegionsContext<(), Option<ColumnFamily>> = ScanRegionsContext::new(range, cf);
        let inner = Arc::clone(&self.inner);
        loop_fn((inner, scan), |(inner, scan)| {
            inner.locate_key(scan.start_key()).and_then(|location| {
                let region = location.into_inner();
                let cf = scan.clone();
                Self::region_context_by_id(Arc::clone(&inner), region.id)
                    .map(|(region, client)| {
                        (scan, region.range(), RawContext::new(region, client, cf))
                    })
                    .and_then(|(mut scan, region_range, context)| {
                        let (start_key, end_key) = scan.range();
                        let start_key = start_key.expect("start key must be specified");
                        let end_key = end_key.expect("end key must be specified");
                        context
                            .client()
                            .raw_delete_range(context, start_key, end_key)
                            .map(|_| (scan, region_range))
                    })
                    .map(|(mut scan, region_range)| match scan.next(region_range) {
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
        let mut kvctx = kvrpcpb::Context::new();
        kvctx.set_region_id(ctx.region.id);
        kvctx.set_region_epoch(ctx.region.take_region_epoch());
        kvctx.set_peer(ctx.region.peer().expect("leader must exist").into_inner());
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

struct KeyLocation(Region);

impl KeyLocation {
    fn new(region: Region) -> Self {
        KeyLocation(region)
    }

    fn into_inner(self) -> Region {
        self.0
    }
}

impl Deref for KeyLocation {
    type Target = Region;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

trait GroupingTask: Clone + Default + Sized {
    fn key(&self) -> &Key;
}

#[derive(Default)]
struct GroupedTasks<Task: GroupingTask>(HashMap<RegionVerId, Vec<Task>>, RegionVerId);

impl<Task> GroupedTasks<Task>
where
    Task: GroupingTask,
{
    fn new(ver_id: RegionVerId, task: Task) -> Self {
        let mut map = HashMap::with_capacity(1);
        map.insert(ver_id.clone(), vec![task]);
        GroupedTasks(map, ver_id)
    }

    fn add(&mut self, ver_id: RegionVerId, task: Task) {
        self.0
            .entry(ver_id)
            .or_insert_with(|| Vec::with_capacity(1))
            .push(task)
    }

    fn into_inner(self) -> HashMap<RegionVerId, Vec<Task>> {
        self.0
    }
}

impl<Task> Deref for GroupedTasks<Task>
where
    Task: GroupingTask,
{
    type Target = HashMap<RegionVerId, Vec<Task>>;

    fn deref(&self) -> &Self::Target {
        &self.0
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

impl<Res, State> Deref for ScanRegionsContext<Res, State>
where
    Res: Default,
    State: Sized,
{
    type Target = State;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
