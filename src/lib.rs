extern crate futures;
extern crate serde;
#[macro_use]
extern crate serde_derive;

use std::io::Error;
use std::path::PathBuf;

use futures::Future;

pub mod raw;
pub mod transaction;

#[derive(Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Key(Vec<u8>);
#[derive(Default, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Value(Vec<u8>);
#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct KvPair(Key, Value);

pub type KvFuture<T> = Box<Future<Item = T, Error = Error> + Send>;

impl Into<Key> for Vec<u8> {
    fn into(self) -> Key {
        Key(self)
    }
}

impl AsRef<Key> for Key {
    fn as_ref(&self) -> &Self {
        self
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub pd_endpoints: Vec<String>,
    pub ca_path: Option<PathBuf>,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
}

impl Config {
    pub fn new<E, S>(pd_endpoints: E) -> Self
    where
        E: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Config {
            pd_endpoints: pd_endpoints.into_iter().map(Into::into).collect(),
            ca_path: None,
            cert_path: None,
            key_path: None,
        }
    }

    pub fn with_security<E>(
        pd_endpoints: E,
        ca_path: PathBuf,
        cert_path: PathBuf,
        key_path: PathBuf,
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
