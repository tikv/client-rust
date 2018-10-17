extern crate futures;
extern crate serde;
#[macro_use]
extern crate serde_derive;

use futures::Future;
use std::io::Error;

pub mod raw;
pub mod transaction;

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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub pd_endpoints: Vec<String>,
    pub ca_path: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

impl Config {
    pub fn new<E>(pd_endpoints: E) -> Self
    where
        E: IntoIterator<Item = String>,
    {
        Config {
            pd_endpoints: pd_endpoints.into_iter().collect(),
            ca_path: None,
            cert_path: None,
            key_path: None,
        }
    }

    pub fn with_security<E>(
        pd_endpoints: E,
        ca_path: String,
        cert_path: String,
        key_path: String,
    ) -> Self
    where
        E: IntoIterator<Item = String>,
    {
        Config {
            pd_endpoints: pd_endpoints.into_iter().collect(),
            ca_path: Some(ca_path),
            cert_path: Some(cert_path),
            key_path: Some(key_path),
        }
    }
}
