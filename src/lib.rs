extern crate futures;

use std::io::Error;
use futures::Future;

pub mod api;

pub struct Key(Vec<u8>);
pub struct Value(Vec<u8>);
pub struct KvPair(Key, Value);
pub struct KeyRange(Key, Key);

pub type TiKvFuture<T> = Box<Future<Item = T, Error = Error> + Send>;

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
    fn execute(self, kv: &TiKv) -> TiKvFuture<Self::Response>;
}

pub struct TiKv {}

impl TiKv {
    pub fn execute<E, R>(&self, request: E) -> TiKvFuture<R>
    where
        E: Request<Response = R>,
        R: Sized,
    {
        request.execute(self)
    }
}
