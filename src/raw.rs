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

use crate::{transmute_bound, Config, Error, Key, KvPair, Value};
use futures::{Future, Poll};
use std::{u32, ops::{Deref, RangeBounds, Bound}};

/// The TiKV raw [`Client`](struct.Client.html) is used to issue requests to the TiKV server and PD cluster.
pub struct Client;

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

    /// Create a new [`Get`](struct.Get.html) request.
    ///
    /// Once resolved this request will result in the fetching of the value associated with the given key.
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
        Get::new(self, key.into())
    }

    /// Create a new [`BatchGet`](struct.BatchGet.html) request.
    ///
    /// Once resolved this request will result in the fetching of the values associated with the given keys.
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
        BatchGet::new(self, keys.into_iter().map(|v| v.into()).collect())
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
        Put::new(self, key.into(), value.into())
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
        BatchPut::new(self, pairs.into_iter().map(Into::into).collect())
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
        Delete::new(self, key.into())
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
        BatchDelete::new(self, keys.into_iter().map(Into::into).collect())
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
    pub fn scan<K>(&self, range: impl RangeBounds<K>, limit: impl Into<Option<u32>>) -> Scan
    where
        K: Into<Key> + Clone,
    {
        Scan::new(
            self,
            (
                transmute_bound(range.start_bound()),
                transmute_bound(range.end_bound()),
            ),
            limit.into().unwrap_or(u32::MAX),
        )
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
    pub fn batch_scan<K>(
        &self,
        ranges: impl IntoIterator<Item = impl RangeBounds<K>>,
        each_limit: impl Into<Option<u32>>,
    ) -> BatchScan
    where
        K: Into<Key> + Clone,
    {
        BatchScan::new(
            self,
            ranges
                .into_iter()
                .map(|v| {
                    (
                        transmute_bound(v.start_bound()),
                        transmute_bound(v.end_bound()),
                    )
                })
                .collect(),
            each_limit.into().unwrap_or(u32::MAX),
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
    pub fn delete_range<K>(&self, range: impl RangeBounds<K>) -> DeleteRange
    where
        K: Into<Key> + Clone,
    {
        DeleteRange::new(
            self,
            (
                transmute_bound(range.start_bound()),
                transmute_bound(range.end_bound()),
            ),
        )
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
        let _config = &self.config;
        unimplemented!()
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

impl Deref for ColumnFamily {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// An unresolved [`Client::get`](struct.Client.html#method.get) request.
/// 
/// Once resolved this request will result in the fetching of the value associated with the given key.
pub struct Get<'a> {
    client: &'a Client,
    key: Key,
    cf: Option<ColumnFamily>,
}

impl<'a> Get<'a> {
    fn new(client: &'a Client, key: Key) -> Self {
        Get {
            client,
            key,
            cf: None,
        }
    }

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for Get<'a> {
    type Item = Value;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.key;
        let _ = &self.cf;
        unimplemented!()
    }
}

/// An unresolved [`Client::batch_get`](struct.Client.html#method.batch_get) request.
///
/// Once resolved this request will result in the fetching of the values associated with the given keys.
pub struct BatchGet<'a> {
    client: &'a Client,
    keys: Vec<Key>,
    cf: Option<ColumnFamily>,
}

impl<'a> BatchGet<'a> {
    fn new(client: &'a Client, keys: Vec<Key>) -> Self {
        BatchGet {
            client,
            keys,
            cf: None,
        }
    }
    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for BatchGet<'a> {
    type Item = Vec<KvPair>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.keys;
        let _ = &self.cf;
        unimplemented!()
    }
}

/// An unresolved [`Client::put`](struct.Client.html#method.put) request.
/// 
/// Once resolved this request will result in the setting of the value associated with the given key.
pub struct Put<'a> {
    client: &'a Client,
    key: Key,
    value: Value,
    cf: Option<ColumnFamily>,
}

impl<'a> Put<'a> {
    fn new(client: &'a Client, key: Key, value: Value) -> Self {
        Put {
            client,
            key,
            value,
            cf: None,
        }
    }

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for Put<'a> {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.key;
        let _ = &self.value;
        let _ = &self.cf;
        unimplemented!()
    }
}

/// An unresolved [`Client::batch_put`](struct.Client.html#method.batch_put) request.
/// 
/// Once resolved this request will result in the setting of the value associated with the given key.
pub struct BatchPut<'a> {
    client: &'a Client,
    pairs: Vec<KvPair>,
    cf: Option<ColumnFamily>,
}

impl<'a> BatchPut<'a> {
    fn new(client: &'a Client, pairs: Vec<KvPair>) -> Self {
        BatchPut {
            client,
            pairs,
            cf: None,
        }
    }

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for BatchPut<'a> {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.pairs;
        let _ = &self.cf;
        unimplemented!()
    }
}

/// An unresolved [`Client::delete`](struct.Client.html#method.delete) request.
///
/// Once resolved this request will result in the deletion of the given key.
pub struct Delete<'a> {
    client: &'a Client,
    key: Key,
    cf: Option<ColumnFamily>,
}

impl<'a> Delete<'a> {
    fn new(client: &'a Client, key: Key) -> Self {
        Delete {
            client,
            key,
            cf: None,
        }
    }

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for Delete<'a> {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.key;
        let _ = &self.cf;
        unimplemented!()
    }
}

/// An unresolved [`Client::batch_delete`](struct.Client.html#method.batch_delete) request.
pub struct BatchDelete<'a> {
    client: &'a Client,
    keys: Vec<Key>,
    cf: Option<ColumnFamily>,
}

impl<'a> BatchDelete<'a> {
    fn new(client: &'a Client, keys: Vec<Key>) -> Self {
        BatchDelete {
            client,
            keys,
            cf: None,
        }
    }

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for BatchDelete<'a> {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.keys;
        let _ = &self.cf;
        unimplemented!()
    }
}

/// An unresolved [`Client::scan`](struct.Client.html#method.scan) request.
/// 
/// Once resolved this request will result in a scanner over the given keys.
pub struct Scan<'a> {
    client: &'a Client,
    range: (Bound<Key>, Bound<Key>),
    limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
    reverse: bool,
}

impl<'a> Scan<'a> {
    fn new(client: &'a Client, range: (Bound<Key>, Bound<Key>), limit: u32) -> Self {
        Scan {
            client,
            range,
            limit,
            key_only: false,
            cf: None,
            reverse: false,
        }
    }

    pub fn key_only(mut self) -> Self {
        self.key_only = true;
        self
    }

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }

    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
}

impl<'a> Future for Scan<'a> {
    type Item = Vec<KvPair>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.range;
        let _ = &self.limit;
        let _ = &self.key_only;
        let _ = &self.cf;
        unimplemented!()
    }
}

/// An unresolved [`Client::batch_scan`](struct.Client.html#method.batch_scan) request.
/// 
/// Once resolved this request will result in a set of scanners over the given keys.
pub struct BatchScan<'a> {
    client: &'a Client,
    ranges: Vec<(Bound<Key>, Bound<Key>)>,
    each_limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
    reverse: bool,
}

impl<'a> BatchScan<'a> {
    fn new(client: &'a Client, ranges: Vec<(Bound<Key>, Bound<Key>)>, each_limit: u32) -> Self {
        BatchScan {
            client,
            ranges,
            each_limit,
            key_only: false,
            cf: None,
            reverse: false,
        }
    }

    pub fn key_only(mut self) -> Self {
        self.key_only = true;
        self
    }

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }

    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
}

impl<'a> Future for BatchScan<'a> {
    type Item = Vec<KvPair>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.ranges;
        let _ = &self.each_limit;
        let _ = &self.key_only;
        let _ = &self.cf;
        unimplemented!()
    }
}

/// An unresolved [`Client::delete_range`](struct.Client.html#method.delete_range) request.
/// 
/// Once resolved this request will result in the deletion of all keys over the given range.
pub struct DeleteRange<'a> {
    client: &'a Client,
    range: (Bound<Key>, Bound<Key>),
    cf: Option<ColumnFamily>,
}

impl<'a> DeleteRange<'a> {
    fn new(client: &'a Client, range: (Bound<Key>, Bound<Key>)) -> Self {
        DeleteRange {
            client,
            range,
            cf: None,
        }
    }

    /// Set the (optional) [`ColumnFamily`](struct.ColumnFamily.html).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for DeleteRange<'a> {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.range;
        let _ = &self.cf;
        unimplemented!()
    }
}
