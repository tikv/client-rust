use std::io::Error;

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

impl Scanner {
    pub fn set_limit(&mut self, _limit: u32) {
        unimplemented!()
    }

    pub fn set_key_only(&mut self, _key_only: bool) {
        unimplemented!()
    }
}

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

    fn seek(&self, key: impl AsRef<Key>) -> KvFuture<Scanner>;

    fn seek_reverse(&self, key: impl AsRef<Key>) -> KvFuture<Scanner>;
}

pub trait Mutator {
    fn set(&mut self, pair: impl Into<KvPair>) -> KvFuture<()>;

    fn delete(&mut self, key: impl AsRef<Key>) -> KvFuture<()>;
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

    pub fn snapshot(&self) -> KvFuture<Snapshot> {
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

    fn seek(&self, key: impl AsRef<Key>) -> KvFuture<Scanner> {
        drop(key);
        unimplemented!()
    }

    fn seek_reverse(&self, key: impl AsRef<Key>) -> KvFuture<Scanner> {
        drop(key);
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

    fn seek(&self, key: impl AsRef<Key>) -> KvFuture<Scanner> {
        drop(key);
        unimplemented!()
    }

    fn seek_reverse(&self, key: impl AsRef<Key>) -> KvFuture<Scanner> {
        drop(key);
        unimplemented!()
    }
}

pub trait Client {
    fn begin(&self) -> KvFuture<Transaction>;

    fn begin_with_timestamp(&self, _timestamp: Timestamp) -> KvFuture<Transaction>;

    fn snapshot(&self) -> KvFuture<Snapshot>;

    fn current_timestamp(&self) -> Timestamp;
}

pub struct TxnClient {}

impl TxnClient {
    pub fn new(_config: &Config) -> KvFuture<Self> {
        unimplemented!()
    }
}

impl Client for TxnClient {
    fn begin(&self) -> KvFuture<Transaction> {
        unimplemented!()
    }

    fn begin_with_timestamp(&self, _timestamp: Timestamp) -> KvFuture<Transaction> {
        unimplemented!()
    }

    fn snapshot(&self) -> KvFuture<Snapshot> {
        unimplemented!()
    }

    fn current_timestamp(&self) -> Timestamp {
        unimplemented!()
    }
}
