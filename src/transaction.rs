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
use futures::{Future, Poll, Stream};
use std::ops::RangeBounds;

#[derive(Copy, Clone)]
pub struct Timestamp(u64);

impl Into<Timestamp> for u64 {
    fn into(self) -> Timestamp {
        Timestamp(self)
    }
}

impl Timestamp {
    pub fn timestamp(self) -> u64 {
        self.0
    }

    pub fn physical(self) -> i64 {
        (self.0 >> 16) as i64
    }

    pub fn logical(self) -> i64 {
        (self.0 & 0xFFFF as u64) as i64
    }
}

pub struct Scanner;

impl Stream for Scanner {
    type Item = KvPair;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        unimplemented!()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum IsolationLevel {
    SnapshotIsolation,
    ReadCommitted,
}

pub struct Get {
    key: Key,
}

impl Get {
    fn new(key: Key) -> Self {
        Get { key }
    }
}

impl Future for Get {
    type Item = Value;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _key = &self.key;
        unimplemented!()
    }
}

pub struct BatchGet {
    keys: Vec<Key>,
}

impl BatchGet {
    fn new(keys: Vec<Key>) -> Self {
        BatchGet { keys }
    }
}

impl Future for BatchGet {
    type Item = Value;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _keys = &self.keys;
        unimplemented!()
    }
}

pub struct Commit {
    txn: Transaction,
}

impl Commit {
    fn new(txn: Transaction) -> Self {
        Commit { txn }
    }
}

impl Future for Commit {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _txn = &self.txn;
        unimplemented!()
    }
}

pub struct Rollback {
    txn: Transaction,
}

impl Rollback {
    fn new(txn: Transaction) -> Self {
        Rollback { txn }
    }
}

impl Future for Rollback {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _txn = &self.txn;
        unimplemented!()
    }
}

pub struct LockKeys {
    keys: Vec<Key>,
}

impl LockKeys {
    fn new(keys: Vec<Key>) -> Self {
        LockKeys { keys }
    }
}

impl Future for LockKeys {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _keys = &self.keys;
        unimplemented!()
    }
}

pub struct Set {
    key: Key,
    value: Value,
}

impl Set {
    fn new(key: Key, value: Value) -> Self {
        Set { key, value }
    }
}

impl Future for Set {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _key = &self.key;
        let _value = &self.value;
        unimplemented!()
    }
}

pub struct Delete {
    key: Key,
}

impl Delete {
    fn new(key: Key) -> Self {
        Delete { key }
    }
}

impl Future for Delete {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _key = &self.key;
        unimplemented!()
    }
}

pub struct Transaction {
    snapshot: Snapshot,
}

impl Transaction {
    pub fn commit(self) -> Commit {
        Commit::new(self)
    }

    pub fn rollback(self) -> Rollback {
        Rollback::new(self)
    }

    pub fn lock_keys(&mut self, keys: impl AsRef<[Key]>) -> LockKeys {
        LockKeys::new(keys.as_ref().to_vec().clone())
    }

    pub fn is_readonly(&self) -> bool {
        unimplemented!()
    }

    pub fn start_ts(&self) -> Timestamp {
        unimplemented!()
    }

    pub fn snapshot(&self) -> Snapshot {
        unimplemented!()
    }

    pub fn set_isolation_level(&mut self, _level: IsolationLevel) {
        unimplemented!()
    }

    pub fn get(&self, key: impl AsRef<Key>) -> Get {
        self.snapshot.get(key)
    }

    pub fn batch_get(&self, keys: impl AsRef<[Key]>) -> BatchGet {
        self.snapshot.batch_get(keys)
    }

    pub fn scan(&self, range: impl RangeBounds<Key>) -> Scanner {
        self.snapshot.scan(range)
    }

    pub fn scan_reverse(&self, range: impl RangeBounds<Key>) -> Scanner {
        self.snapshot.scan_reverse(range)
    }

    pub fn set(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Set {
        Set::new(key.into(), value.into())
    }

    pub fn delete(&mut self, key: impl AsRef<Key>) -> Delete {
        Delete::new(key.as_ref().clone())
    }
}

pub struct Snapshot;

impl Snapshot {
    pub fn get(&self, key: impl AsRef<Key>) -> Get {
        Get::new(key.as_ref().clone())
    }

    pub fn batch_get(&self, keys: impl AsRef<[Key]>) -> BatchGet {
        BatchGet::new(keys.as_ref().to_vec().clone())
    }

    pub fn scan(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
        unimplemented!()
    }

    pub fn scan_reverse(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
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

pub struct Client {}

impl Client {
    #![allow(clippy::new_ret_no_self)]
    pub fn new(config: &Config) -> Connect {
        Connect::new(config.clone())
    }

    pub fn begin(&self) -> Transaction {
        unimplemented!()
    }

    pub fn begin_with_timestamp(&self, _timestamp: Timestamp) -> Transaction {
        unimplemented!()
    }

    pub fn snapshot(&self) -> Snapshot {
        unimplemented!()
    }

    pub fn current_timestamp(&self) -> Timestamp {
        unimplemented!()
    }
}
