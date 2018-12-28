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

/*! Raw related functionality.

Using the [`raw::Client`](struct.Client.html) you can utilize TiKV's raw interface.

This interface offers optimal performance as it does not require coordination with a timestamp
oracle, while the transactional interface does.

**Warning:** It is not advisible to use the both raw and transactional functionality in the same keyspace.
 */

use crate::{rpc::RpcClient, Config, Error, Key, KeyRange, KvFuture, KvPair, Result, Value};
use futures::{future, Future, Poll, Async};
use std::{u32, sync::Arc, ops::{Bound, Deref, RangeBounds, DerefMut}};


/// The TiKV raw [`Client`](struct.Client.html) is used to issue requests to the TiKV server and PD cluster.
pub struct Client {
    rpc: Arc<RpcClient>
}

impl Client {
    /// Create a new [`Client`](struct.Client.html) once the [`Connect`](struct.Connect.html) resolves.
    ///
    /// ```rust,no_run
    /// use tikv_client::{Config, raw::Client};
    /// use futures::Future;
    /// let connect = Client::new(&Config::default());
    /// let client = connect.wait();
    /// ```
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    pub fn new(config: &Config) -> Connect {
        Connect::new(config.clone())
    }

    #[inline]
    fn rpc(&self) -> Arc<RpcClient> {
        Arc::clone(&self.rpc)
    }

    /// Create a new [`Get`](struct.Get.html) request.
    ///
    /// Once resolved this request will result in the fetching of the value associated with the
    /// given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, raw::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(&Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let key = "TiKV";
    /// let req = connected_client.get(key);
    /// let result: Value = req.wait().unwrap();
    /// ```
    pub fn get(&self, key: impl Into<Key>) -> Get {
        Get::new(self.rpc(), GetInner::new(key.into()))
    }

    /// Create a new [`BatchGet`](struct.BatchGet.html) request.
    ///
    /// Once resolved this request will result in the fetching of the values associated with the 
    /// given keys.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{KvPair, Config, raw::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(&Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let keys = vec!["TiKV", "TiDB"];
    /// let req = connected_client.batch_get(keys);
    /// let result: Vec<KvPair> = req.wait().unwrap();
    /// ```
    pub fn batch_get(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> BatchGet {
        BatchGet::new(self.rpc(), BatchGetInner::new(keys.into_iter().map(Into::into).collect()))
    }

    /// Create a new [`Put`](struct.Put.html) request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, raw::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(&Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let key = "TiKV";
    /// let val = "TiKV";
    /// let req = connected_client.put(key, val);
    /// let result: () = req.wait().unwrap();
    /// ```
    pub fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Put {
        Put::new(self.rpc(), PutInner::new(key.into(), value.into()))
    }

    /// Create a new [`BatchPut`](struct.BatchPut.html) request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Error, Result, KvPair, Key, Value, Config, raw::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(&Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let kvpair1 = ("PD", "Go");
    /// let kvpair2 = ("TiKV", "Rust");
    /// let iterable = vec![kvpair1, kvpair2];
    /// let req = connected_client.batch_put(iterable);
    /// let result: () = req.wait().unwrap();
    /// ```
    pub fn batch_put(&self, pairs: impl IntoIterator<Item = impl Into<KvPair>>) -> BatchPut {
        BatchPut::new(self.rpc(), BatchPutInner::new(pairs.into_iter().map(Into::into).collect()))
    }

    /// Create a new [`Delete`](struct.Delete.html) request.
    ///
    /// Once resolved this request will result in the deletion of the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, raw::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(&Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let key = "TiKV";
    /// let req = connected_client.delete(key);
    /// let result: () = req.wait().unwrap();
    /// ```
    pub fn delete(&self, key: impl Into<Key>) -> Delete {
        Delete::new(self.rpc(), DeleteInner::new(key.into()))
    }

    /// Create a new [`BatchDelete`](struct.BatchDelete.html) request.
    ///
    /// Once resolved this request will result in the deletion of the given keys.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, raw::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(&Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let keys = vec!["TiKV", "TiDB"];
    /// let req = connected_client.batch_delete(keys);
    /// let result: () = req.wait().unwrap();
    /// ```
    pub fn batch_delete(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> BatchDelete {
        BatchDelete::new(self.rpc(), BatchDeleteInner::new(keys.into_iter().map(Into::into).collect()))
    }

    /// Create a new [`Scan`](struct.Scan.html) request.
    ///
    /// Once resolved this request will result in a scanner over the given keys.
    ///
    /// If not passed a `limit` parameter, it will default to `u32::MAX`.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{KvPair, Config, raw::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(&Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = connected_client.scan(inclusive_range, 2);
    /// let result: Vec<KvPair> = req.wait().unwrap();
    /// ```
    pub fn scan(&self, range: impl KeyRange, limit: impl Into<Option<u32>>) -> Scan {
        Scan::new(self.rpc(), ScanInner::new(range.into_bounds(), limit.into().unwrap_or(u32::MAX)))
    }

    /// Create a new [`BatchScan`](struct.BatchScan.html) request.
    ///
    /// Once resolved this request will result in a set of scanners over the given keys.
    ///
    /// If not passed a `limit` parameter, it will default to `u32::MAX`.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, raw::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(&Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let inclusive_range1 = "TiDB"..="TiKV";
    /// let inclusive_range2 = "TiKV"..="TiSpark";
    /// let iterable = vec![inclusive_range1, inclusive_range2];
    /// let req = connected_client.batch_scan(iterable, 2);
    /// let result = req.wait();
    /// ```
    pub fn batch_scan(
        &self,
        ranges: impl IntoIterator<Item = impl KeyRange>,
        each_limit: impl Into<Option<u32>>,
    ) -> BatchScan {
        BatchScan::new(
            self.rpc(),
            BatchScanInner::new(
                ranges.into_iter().map(KeyRange::into_keys).collect(),
                each_limit.into().unwrap_or(u32::MAX),
            ),
        )
    }

    /// Create a new [`DeleteRange`](struct.DeleteRange.html) request.
    ///
    /// Once resolved this request will result in the deletion of all keys over the given range.
    ///
    /// If not passed a `limit` parameter, it will default to `u32::MAX`.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, raw::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(&Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = connected_client.delete_range(inclusive_range);
    /// let result: () = req.wait().unwrap();
    /// ```
    pub fn delete_range(&self, range: impl KeyRange) -> DeleteRange {
        DeleteRange::new(self.rpc(), DeleteRangeInner::new(range.into_keys()))
    }
}

/// An unresolved [`Client`](struct.Client.html) connection to a TiKV cluster.
/// 
/// Once resolved it will result in a connected [`Client`](struct.Client.html).
///
/// ```rust,no_run
/// use tikv_client::{Config, raw::{Client, Connect}};
/// use futures::Future;
/// 
/// let connect: Connect = Client::new(&Config::default());
/// let client: Client = connect.wait().unwrap();
/// ```
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


/// A [`ColumnFamily`](struct.ColumnFamily.html) is an optional parameter for [`raw::Client`](struct.Client.html) requests.
/// 
/// TiKV uses RocksDB's `ColumnFamily` support. You can learn more about RocksDB's `ColumnFamily`s [on their wiki](https://github.com/facebook/rocksdb/wiki/Column-Families).
/// 
/// By default in TiKV data is stored in three different `ColumnFamily` values, configurable in the TiKV server's configuration:
/// 
/// * Default: Where real user data is stored. Set by `[rocksdb.defaultcf]`.
/// * Write: Where MVCC and index related data are stored. Set by `[rocksdb.writecf]`.
/// * Lock: Where lock information is stored. Set by `[rocksdb.lockcf]`.
/// 
/// Not providing a call a `ColumnFamily` means it will use the default value of `default`.
/// 
/// The best (and only) way to create a [`ColumnFamily`](struct.ColumnFamily.html) is via the `From` implementation:
/// 
/// ```rust
/// # use tikv_client::raw::ColumnFamily;
/// 
/// let cf = ColumnFamily::from("write");
/// let cf = ColumnFamily::from(String::from("write"));
/// let cf = ColumnFamily::from(&String::from("write"));
/// ```
/// 
/// This is a *wrapper type* that implements `Deref<Target=String>` so it can be used like one transparently.
/// 
/// **But, you should not need to worry about all this:** Many functions which accept a
/// `ColumnFamily` accept an `Into<ColumnFamily>`, which means all of the above types can be passed
/// directly to those functions.
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

impl Deref for ColumnFamily {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
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

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
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

/// An unresolved [`Client::get`](struct.Client.html#method.get) request.
/// 
/// Once resolved this request will result in the fetching of the value associated with the given
/// key.
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

/// An unresolved [`Client::batch_get`](struct.Client.html#method.batch_get) request.
///
/// Once resolved this request will result in the fetching of the values associated with the given
/// keys.
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

/// An unresolved [`Client::put`](struct.Client.html#method.put) request.
/// 
/// Once resolved this request will result in the putting of the value associated with the given
/// key.
pub type Put = Request<PutInner>;

/// An unresolved [`Client::batch_put`](struct.Client.html#method.batch_put) request.
/// 
/// Once resolved this request will result in the setting of the value associated with the given key.
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


/// An unresolved [`Client::batch_put`](struct.Client.html#method.batch_put) request.
/// 
/// Once resolved this request will result in the setting of the value associated with the given key.
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

/// An unresolved [`Client::delete`](struct.Client.html#method.delete) request.
///
/// Once resolved this request will result in the deletion of the given key.
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
    range: (Bound<Key>, Bound<Key>),
    limit: u32,
    key_only: bool,
}

impl ScanInner {
    fn new(range: (Bound<Key>, Bound<Key>), limit: u32) -> Self {
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
        let keys = match self.range.into_keys() {
            Err(e) => return Box::new(future::err(e)),
            Ok(v) => v,
        };
        Box::new(client.raw_scan(keys, self.limit, self.key_only, cf))
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

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
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

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
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

pub type DeleteRange = Request<DeleteRangeInner>;

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
