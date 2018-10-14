extern crate futures;

use futures::Future;
use std::io::Error;

pub mod api;

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

pub trait Request: Sized {
    type Response: Sized;
    fn execute(self, kv: &Client) -> KvFuture<Self::Response>;
}

pub struct Client {}
