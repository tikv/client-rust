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

use serde_derive::*;
use std::ops::Deref;
use std::path::PathBuf;

pub mod errors;
pub mod raw;
pub mod transaction;

pub use crate::errors::Error;
pub use crate::errors::Result;

/// The key part of a key/value pair.
/// 
/// In TiKV, keys are an ordered sequence of bytes. This has an advantage over choosing `String` as valid `UTF-8` is not required.
/// This means that the user is permitted to store any data they wish, as long as it can be represented by bytes.
/// 
/// This type implements `Deref<Target=Vec<u8>>` so it can be used like one transparently.
/// 
/// This type contains an owned value, so it should be treated it like `String` or `Vec<u8>` over a `&str` or `&[u8]`.
/// 
/// ```rust
/// # use tikv_client::Key;
/// let from_bytes = Key::from(b"TiKV".to_vec());
/// let from_string = Key::from(String::from("TiKV"));
/// 
/// assert_eq!(from_bytes, from_string);
/// assert_eq!(*from_bytes, b"TiKV".to_vec());
/// ```
#[derive(Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Key(Vec<u8>);

/// The value part of a key/value pair.
/// 
/// In TiKV, values are an ordered sequence of bytes. This has an advantage over choosing `String` as valid `UTF-8` is not required.
/// This means that the user is permitted to store any data they wish, as long as it can be represented by bytes.
/// 
/// This type implements `Deref<Target=Vec<u8>>` so it can be used like one transparently.
/// 
/// This type contains an owned value, so it should be treated it like `String` or `Vec<u8>` over a `&str` or `&[u8]`.
/// 
/// ```rust
/// # use tikv_client::Value;
/// let from_bytes = Value::from(b"TiKV".to_vec());
/// let from_string = Value::from(String::from("TiKV"));
/// 
/// assert_eq!(from_bytes, from_string);
/// assert_eq!(*from_bytes, b"TiKV".to_vec());
/// ```
#[derive(Default, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Value(Vec<u8>);
/// A key/value pair.
/// 
/// Used primarily in batch and scan requests.
/// 
/// ```rust
/// # use tikv_client::{Key, Value, KvPair};
/// let key = b"key".to_vec();
/// let value = b"value".to_vec();
/// let constructed = KvPair::new(key.clone(), value.clone());
/// let from_tuple = KvPair::from((key.into(), value.into()));
/// assert_eq!(constructed, from_tuple);
#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct KvPair(Key, Value);

impl From<Vec<u8>> for Key {
    fn from(v: Vec<u8>) -> Self {
        Key(v)
    }
}

impl From<String> for Key {
    fn from(v: String) -> Key {
        Key(v.into_bytes())
    }
}

impl AsRef<Key> for Key {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl Deref for Key {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Value {
        Value(v.into_bytes())
    }
}

impl Deref for Value {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl KvPair {
    pub fn new(key: impl Into<Key>, value: impl Into<Value>) -> Self {
        KvPair(key.into(), value.into())
    }

    pub fn key(&self) -> &Key {
        &self.0
    }

    pub fn value(&self) -> &Value {
        &self.1
    }
}

impl From<(Key, Value)> for KvPair {
    fn from((k, v): (Key, Value)) -> KvPair {
        KvPair(k, v)
    }
}

/// The configuration of either a [`raw::Client`](raw/struct.Client.html) or a [`transaction::Client`](transaction/struct.Client.html).
/// 
/// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster the endpoints for PD must be provided, **not** the TiKV nodes.
///
/// It's important to **include more than one PD endpoint** (include all, if possible!) This helps avoid having a *single point of failure*.
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
    /// Create a new [`Config`](struct.Config.html) which coordinates with the given PD endpoints.
    /// 
    /// ```rust
    /// # use tikv_client::Config;
    /// let config = Config::new(vec!["192.168.0.100:2379", "192.168.0.101:2379"]);
    /// ```
    pub fn new(pd_endpoints: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Config {
            pd_endpoints: pd_endpoints.into_iter().map(Into::into).collect(),
            ca_path: None,
            cert_path: None,
            key_path: None,
        }
    }

    /// Set the certificate authority, certificate, and key locations for the [`Config`](struct.Config.html).
    /// 
    /// By default, TiKV connections do not have utilize transport layer security. Enable it by setting these values.
    /// 
    /// ```rust
    /// # use tikv_client::Config;
    /// let config = Config::new(vec!["192.168.0.100:2379", "192.168.0.101:2379"])
    ///     .with_security("root.ca", "internal.cert", "internal.key");
    /// ```
    pub fn with_security(
        mut self,
        ca_path: impl Into<PathBuf>,
        cert_path: impl Into<PathBuf>,
        key_path: impl Into<PathBuf>,
    ) -> Self {
        self.ca_path = Some(ca_path.into());
        self.cert_path = Some(cert_path.into());
        self.key_path = Some(key_path.into());
        self
    }
}
