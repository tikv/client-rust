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

use crate::{Config, Error, Key, KvPair, Value};
use futures::{Future, Poll};
use std::ops::RangeBounds;

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

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for BatchGet<'a> {
    type Item = Vec<KvPair>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.keys;
        let _ = &self.cf;
        unimplemented!()
    }
}

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

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for Put<'a> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.key;
        let _ = &self.value;
        let _ = &self.cf;
        unimplemented!()
    }
}

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

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for BatchPut<'a> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.pairs;
        let _ = &self.cf;
        unimplemented!()
    }
}

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

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for Delete<'a> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.key;
        let _ = &self.cf;
        unimplemented!()
    }
}

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

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for BatchDelete<'a> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.keys;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct Scan<'a> {
    client: &'a Client,
    range: (Key, Key),
    limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
    reverse: bool,
}

impl<'a> Scan<'a> {
    fn new(client: &'a Client, range: (Key, Key), limit: u32) -> Self {
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
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.range;
        let _ = &self.limit;
        let _ = &self.key_only;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct BatchScan<'a> {
    client: &'a Client,
    ranges: Vec<(Key, Key)>,
    each_limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
    reverse: bool,
}

impl<'a> BatchScan<'a> {
    fn new(client: &'a Client, ranges: Vec<(Key, Key)>, each_limit: u32) -> Self {
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
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.ranges;
        let _ = &self.each_limit;
        let _ = &self.key_only;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct DeleteRange<'a> {
    client: &'a Client,
    range: (Key, Key),
    cf: Option<ColumnFamily>,
}

impl<'a> DeleteRange<'a> {
    fn new(client: &'a Client, range: (Key, Key)) -> Self {
        DeleteRange {
            client,
            range,
            cf: None,
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a> Future for DeleteRange<'a> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.range;
        let _ = &self.cf;
        unimplemented!()
    }
}

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

pub struct Client;

impl Client {
    #![allow(clippy::new_ret_no_self)]
    pub fn new(config: &Config) -> Connect {
        Connect::new(config.clone())
    }

    pub fn get(&self, key: impl AsRef<Key>) -> Get {
        Get::new(self, key.as_ref().clone())
    }

    pub fn batch_get(&self, keys: impl AsRef<[Key]>) -> BatchGet {
        BatchGet::new(self, keys.as_ref().to_vec())
    }

    pub fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Put {
        Put::new(self, key.into(), value.into())
    }

    pub fn batch_put(&self, pairs: impl IntoIterator<Item = impl Into<KvPair>>) -> BatchPut {
        BatchPut::new(self, pairs.into_iter().map(Into::into).collect())
    }

    pub fn delete(&self, key: impl AsRef<Key>) -> Delete {
        Delete::new(self, key.as_ref().clone())
    }

    pub fn batch_delete(&self, keys: impl AsRef<[Key]>) -> BatchDelete {
        BatchDelete::new(self, keys.as_ref().to_vec())
    }

    pub fn scan(&self, range: impl RangeBounds<Key>, limit: u32) -> Scan {
        Scan::new(self, Self::extract_range(&range), limit)
    }

    pub fn batch_scan<Ranges, Bounds>(&self, ranges: Ranges, each_limit: u32) -> BatchScan
    where
        Ranges: AsRef<[Bounds]>,
        Bounds: RangeBounds<Key>,
    {
        BatchScan::new(
            self,
            ranges.as_ref().iter().map(Self::extract_range).collect(),
            each_limit,
        )
    }

    pub fn delete_range(&self, range: impl RangeBounds<Key>) -> DeleteRange {
        DeleteRange::new(self, Self::extract_range(&range))
    }

    fn extract_range(_range: &impl RangeBounds<Key>) -> (Key, Key) {
        unimplemented!()
    }
}
