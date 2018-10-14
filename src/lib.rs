extern crate futures;

use futures::Future;
use std::io::Error;

mod client;
mod raw;
mod txn;

pub use client::Client;
pub use raw::RawKv;
pub use txn::{Oracle, Snapshot, Timestamp, Transaction, TxnKv};

pub struct Key(Vec<u8>);
pub struct Value(Vec<u8>);
pub struct KvPair(Key, Value);
pub struct KeyRange(Key, Key);

pub type KvFuture<T> = Box<Future<Item = T, Error = Error> + Send>;

impl Into<Key> for Vec<u8> {
    fn into(self) -> Key {
        Key(self)
    }
}

impl Into<Value> for Vec<u8> {
    fn into(self) -> Value {
        Value(self)
    }
}

impl Into<KvPair> for (Key, Value) {
    fn into(self) -> KvPair {
        KvPair(self.0, self.1)
    }
}

impl Into<KeyRange> for (Key, Key) {
    fn into(self) -> KeyRange {
        KeyRange(self.0, self.1)
    }
}
