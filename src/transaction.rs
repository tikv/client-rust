use std::ops::RangeBounds;

use futures::{future, Future, Poll, Stream};

use {Config, Error, Key, KvPair, Value};

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

pub struct Transaction;

impl Transaction {
    pub fn commit(self) -> impl Future<Item = (), Error = Error> + Send {
        future::ok(())
    }

    pub fn rollback(self) -> impl Future<Item = (), Error = Error> + Send {
        future::ok(())
    }

    pub fn lock_keys(
        &mut self,
        keys: impl AsRef<[Key]>,
    ) -> impl Future<Item = (), Error = Error> + Send {
        drop(keys);
        future::ok(())
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

    pub fn get(&self, key: impl AsRef<Key>) -> impl Future<Item = Value, Error = Error> + Send {
        drop(key);
        future::ok(b"".to_vec().into())
    }

    pub fn batch_get(
        &self,
        keys: impl AsRef<[Key]>,
    ) -> impl Future<Item = Vec<KvPair>, Error = Error> + Send {
        drop(keys);
        future::ok(Vec::new())
    }

    pub fn scan(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
        unimplemented!()
    }

    pub fn scan_reverse(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
        unimplemented!()
    }

    pub fn set(&mut self, pair: impl Into<KvPair>) -> impl Future<Item = (), Error = Error> + Send {
        drop(pair);
        future::ok(())
    }

    pub fn delete(&mut self, key: impl AsRef<Key>) -> impl Future<Item = (), Error = Error> + Send {
        drop(key);
        future::ok(())
    }
}

pub struct Snapshot;

impl Snapshot {
    pub fn get(&self, key: impl AsRef<Key>) -> impl Future<Item = Value, Error = Error> + Send {
        drop(key);
        future::ok(b"".to_vec().into())
    }

    pub fn batch_get(
        &self,
        keys: impl AsRef<[Key]>,
    ) -> impl Future<Item = Vec<KvPair>, Error = Error> + Send {
        drop(keys);
        future::ok(Vec::new())
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

pub struct Client {}

impl Client {
    #![allow(new_ret_no_self)]
    pub fn new(_config: &Config) -> impl Future<Item = Self, Error = Error> + Send {
        future::ok(Client {})
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
