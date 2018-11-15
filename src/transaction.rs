use std::ops::RangeBounds;
use Error;

use futures::{Poll, Stream};

use {Config, Key, KvFuture, KvPair, Value};

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

pub trait Retriever {
    fn get(&self, key: impl AsRef<Key>) -> KvFuture<Value>;

    fn batch_get(&self, keys: impl AsRef<[Key]>) -> KvFuture<Vec<KvPair>>;

    fn scan(&self, range: impl RangeBounds<Key>) -> Scanner;

    fn scan_reverse(&self, range: impl RangeBounds<Key>) -> Scanner;
}

pub trait Mutator {
    fn set(&mut self, pair: impl Into<KvPair>) -> KvFuture<()>;

    fn delete(&mut self, key: impl AsRef<Key>) -> KvFuture<()>;
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum IsolationLevel {
    SnapshotIsolation,
    ReadCommitted,
}

pub struct Transaction;

impl Transaction {
    pub fn commit(self) -> KvFuture<()> {
        unimplemented!()
    }

    pub fn rollback(self) -> KvFuture<()> {
        unimplemented!()
    }

    pub fn lock_keys(&mut self, keys: impl AsRef<[Key]>) -> KvFuture<()> {
        drop(keys);
        unimplemented!()
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
}

impl Retriever for Transaction {
    fn get(&self, key: impl AsRef<Key>) -> KvFuture<Value> {
        drop(key);
        unimplemented!()
    }

    fn batch_get(&self, keys: impl AsRef<[Key]>) -> KvFuture<Vec<KvPair>> {
        drop(keys);
        unimplemented!()
    }

    fn scan(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
        unimplemented!()
    }

    fn scan_reverse(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
        unimplemented!()
    }
}

impl Mutator for Transaction {
    fn set(&mut self, pair: impl Into<KvPair>) -> KvFuture<()> {
        drop(pair);
        unimplemented!()
    }

    fn delete(&mut self, key: impl AsRef<Key>) -> KvFuture<()> {
        drop(key);
        unimplemented!()
    }
}

pub struct Snapshot;

impl Retriever for Snapshot {
    fn get(&self, key: impl AsRef<Key>) -> KvFuture<Value> {
        drop(key);
        unimplemented!()
    }

    fn batch_get(&self, keys: impl AsRef<[Key]>) -> KvFuture<Vec<KvPair>> {
        drop(keys);
        unimplemented!()
    }

    fn scan(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
        unimplemented!()
    }

    fn scan_reverse(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
        unimplemented!()
    }
}

pub trait Client {
    fn begin(&self) -> Transaction;

    fn begin_with_timestamp(&self, _timestamp: Timestamp) -> Transaction;

    fn snapshot(&self) -> Snapshot;

    fn current_timestamp(&self) -> Timestamp;
}

pub struct TxnClient {}

impl TxnClient {
    pub fn new(_config: &Config) -> KvFuture<Self> {
        unimplemented!()
    }
}

impl Client for TxnClient {
    fn begin(&self) -> Transaction {
        unimplemented!()
    }

    fn begin_with_timestamp(&self, _timestamp: Timestamp) -> Transaction {
        unimplemented!()
    }

    fn snapshot(&self) -> Snapshot {
        unimplemented!()
    }

    fn current_timestamp(&self) -> Timestamp {
        unimplemented!()
    }
}
