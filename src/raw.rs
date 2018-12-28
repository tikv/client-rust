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

use std::{
    ops::{Bound, Deref, DerefMut, RangeBounds},
    sync::Arc,
};

use futures::{future, Async, Future, Poll};

use crate::{rpc::RpcClient, Config, Error, Key, KvFuture, KvPair, Result, Value};

#[derive(Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ColumnFamily(String);

impl<T> From<T> for ColumnFamily
where
    T: ToString,
{
    fn from(i: T) -> ColumnFamily {
        ColumnFamily(i.to_string())
    }
}

impl ColumnFamily {
    pub fn into_inner(self) -> String {
        self.0
    }
}

pub trait RequestInner: Sized {
    type Resp;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp>;
}

pub struct Request<Inner>
where
    Inner: RequestInner,
{
    inner: Option<(Arc<RpcClient>, Inner)>,
    future: Option<KvFuture<Inner::Resp>>,
    cf: Option<ColumnFamily>,
}

impl<Inner> Request<Inner>
where
    Inner: RequestInner,
{
    fn new(client: Arc<RpcClient>, inner: Inner) -> Self {
        Request {
            inner: Some((client, inner)),
            future: None,
            cf: None,
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<Inner> Deref for Request<Inner>
where
    Inner: RequestInner,
{
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.inner.as_ref().unwrap().1
    }
}

impl<Inner> DerefMut for Request<Inner>
where
    Inner: RequestInner,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.as_mut().unwrap().1
    }
}

impl<Inner> Future for Request<Inner>
where
    Inner: RequestInner,
{
    type Item = Inner::Resp;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if self.inner.is_some() {
                let (client, inner) = self.inner.take().unwrap();
                self.future = Some(inner.execute(client, self.cf.take()));
            } else {
                break self.future.as_mut().map(|x| x.poll()).unwrap();
            }
        }
    }
}

pub struct GetInner {
    key: Key,
}

impl GetInner {
    fn new(key: Key) -> Self {
        GetInner { key }
    }
}

impl RequestInner for GetInner {
    type Resp = Value;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp> {
        Box::new(client.raw_get(self.key, cf))
    }
}

pub type Get = Request<GetInner>;

pub struct BatchGetInner {
    keys: Vec<Key>,
}

impl RequestInner for BatchGetInner {
    type Resp = Vec<KvPair>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp> {
        Box::new(client.raw_batch_get(self.keys, cf))
    }
}

impl BatchGetInner {
    fn new(keys: Vec<Key>) -> Self {
        BatchGetInner { keys }
    }
}

pub type BatchGet = Request<BatchGetInner>;

pub struct PutInner {
    key: Key,
    value: Value,
}

impl PutInner {
    fn new(key: Key, value: Value) -> Self {
        PutInner { key, value }
    }
}

impl RequestInner for PutInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp> {
        let (key, value) = (self.key, self.value);
        Box::new(client.raw_put(key, value, cf))
    }
}

pub type Put = Request<PutInner>;

pub struct BatchPutInner {
    pairs: Vec<KvPair>,
}

impl BatchPutInner {
    fn new(pairs: Vec<KvPair>) -> Self {
        BatchPutInner { pairs }
    }
}

impl RequestInner for BatchPutInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp> {
        Box::new(client.raw_batch_put(self.pairs, cf))
    }
}

pub type BatchPut = Request<BatchPutInner>;

pub struct DeleteInner {
    key: Key,
}

impl DeleteInner {
    fn new(key: Key) -> Self {
        DeleteInner { key }
    }
}

impl RequestInner for DeleteInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp> {
        Box::new(client.raw_delete(self.key, cf))
    }
}

pub type Delete = Request<DeleteInner>;

pub struct BatchDeleteInner {
    keys: Vec<Key>,
}

impl BatchDeleteInner {
    fn new(keys: Vec<Key>) -> Self {
        BatchDeleteInner { keys }
    }
}

impl RequestInner for BatchDeleteInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp> {
        Box::new(client.raw_batch_delete(self.keys, cf))
    }
}

pub type BatchDelete = Request<BatchDeleteInner>;

pub struct ScanInner {
    range: Result<(Key, Option<Key>)>,
    limit: u32,
    key_only: bool,
}

impl ScanInner {
    fn new(range: Result<(Key, Option<Key>)>, limit: u32) -> Self {
        ScanInner {
            range,
            limit,
            key_only: false,
        }
    }
}

impl RequestInner for ScanInner {
    type Resp = Vec<KvPair>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp> {
        match self.range {
            Ok(range) => Box::new(client.raw_scan(range, self.limit, self.key_only, cf)),
            Err(e) => Box::new(future::err(e)),
        }
    }
}

pub struct Scan {
    request: Request<ScanInner>,
}

impl Scan {
    fn new(client: Arc<RpcClient>, inner: ScanInner) -> Self {
        Scan {
            request: Request::new(client, inner),
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.request = self.request.cf(cf);
        self
    }

    pub fn key_only(mut self) -> Self {
        self.request.inner = self.request.inner.map(|mut x| {
            x.1.key_only = true;
            x
        });
        self
    }
}

impl Future for Scan {
    type Item = Vec<KvPair>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.request.poll()
    }
}

pub struct BatchScanInner {
    ranges: Vec<Result<(Key, Option<Key>)>>,
    each_limit: u32,
    key_only: bool,
}

impl BatchScanInner {
    fn new(ranges: Vec<Result<(Key, Option<Key>)>>, each_limit: u32) -> Self {
        BatchScanInner {
            ranges,
            each_limit,
            key_only: false,
        }
    }
}

impl RequestInner for BatchScanInner {
    type Resp = Vec<KvPair>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp> {
        let (mut errors, ranges): (Vec<_>, Vec<_>) =
            self.ranges.into_iter().partition(Result::is_err);
        if !errors.is_empty() {
            Box::new(future::err(errors.pop().unwrap().unwrap_err()))
        } else {
            Box::new(client.raw_batch_scan(
                ranges.into_iter().map(Result::unwrap).collect(),
                self.each_limit,
                self.key_only,
                cf,
            ))
        }
    }
}

pub struct BatchScan {
    request: Request<BatchScanInner>,
}

impl BatchScan {
    fn new(client: Arc<RpcClient>, inner: BatchScanInner) -> Self {
        BatchScan {
            request: Request::new(client, inner),
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.request = self.request.cf(cf);
        self
    }

    pub fn key_only(mut self) -> Self {
        self.request.inner = self.request.inner.map(|mut x| {
            x.1.key_only = true;
            x
        });
        self
    }
}

impl Future for BatchScan {
    type Item = Vec<KvPair>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.request.poll()
    }
}

pub struct DeleteRangeInner {
    range: Result<(Key, Option<Key>)>,
}

impl DeleteRangeInner {
    fn new(range: Result<(Key, Option<Key>)>) -> Self {
        DeleteRangeInner { range }
    }
}

impl RequestInner for DeleteRangeInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> KvFuture<Self::Resp> {
        match self.range {
            Ok(range) => Box::new(client.raw_delete_range(range, cf)),
            Err(e) => Box::new(future::err(e)),
        }
    }
}

pub type DeleteRange = Request<DeleteRangeInner>;

pub struct Connect {
    config: Config,
}

impl Connect {
    fn new(config: Config) -> Self {
        Connect { config }
    }
}

impl Future for Connect {
    type Item = Client;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let config = &self.config;
        let rpc = Arc::new(RpcClient::connect(config)?);
        Ok(Async::Ready(Client { rpc }))
    }
}

pub struct Client {
    rpc: Arc<RpcClient>,
}

impl Client {
    #![allow(clippy::new_ret_no_self)]
    pub fn new(config: &Config) -> Connect {
        Connect::new(config.clone())
    }

    #[inline]
    fn rpc(&self) -> Arc<RpcClient> {
        Arc::clone(&self.rpc)
    }

    pub fn get(&self, key: impl AsRef<Key>) -> Get {
        Get::new(self.rpc(), GetInner::new(key.as_ref().clone()))
    }

    pub fn batch_get(&self, keys: impl AsRef<[Key]>) -> BatchGet {
        BatchGet::new(self.rpc(), BatchGetInner::new(keys.as_ref().to_vec()))
    }

    pub fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Put {
        Put::new(self.rpc(), PutInner::new(key.into(), value.into()))
    }

    pub fn batch_put(&self, pairs: impl IntoIterator<Item = impl Into<KvPair>>) -> BatchPut {
        BatchPut::new(
            self.rpc(),
            BatchPutInner::new(pairs.into_iter().map(Into::into).collect()),
        )
    }

    pub fn delete(&self, key: impl AsRef<Key>) -> Delete {
        Delete::new(self.rpc(), DeleteInner::new(key.as_ref().clone()))
    }

    pub fn batch_delete(&self, keys: impl AsRef<[Key]>) -> BatchDelete {
        BatchDelete::new(self.rpc(), BatchDeleteInner::new(keys.as_ref().to_vec()))
    }

    pub fn scan(&self, range: impl RangeBounds<Key>, limit: u32) -> Scan {
        Scan::new(
            self.rpc(),
            ScanInner::new(Self::range_bounds(&range), limit),
        )
    }

    pub fn batch_scan<Ranges, Bounds>(&self, ranges: Ranges, each_limit: u32) -> BatchScan
    where
        Ranges: AsRef<[Bounds]>,
        Bounds: RangeBounds<Key>,
    {
        BatchScan::new(
            self.rpc(),
            BatchScanInner::new(
                ranges.as_ref().iter().map(Self::range_bounds).collect(),
                each_limit,
            ),
        )
    }

    pub fn delete_range(&self, range: impl RangeBounds<Key>) -> DeleteRange {
        DeleteRange::new(
            self.rpc(),
            DeleteRangeInner::new(Self::range_bounds(&range)),
        )
    }

    fn bound(bound: Bound<&Key>) -> Option<Key> {
        match bound {
            Bound::Included(k) => Some(k.clone()),
            Bound::Excluded(k) => Some(k.clone()),
            Bound::Unbounded => None,
        }
    }

    fn range_bounds(range: &impl RangeBounds<Key>) -> Result<(Key, Option<Key>)> {
        if let Bound::Included(_) = range.end_bound() {
            return Err(Error::InvalidKeyRange);
        }
        if let Bound::Excluded(_) = range.start_bound() {
            return Err(Error::InvalidKeyRange);
        }

        Ok((
            Self::bound(range.start_bound()).unwrap_or_else(Key::default),
            Self::bound(range.end_bound()),
        ))
    }
}
