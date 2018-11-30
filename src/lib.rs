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
use std::{ops::{Deref, Bound}, path::PathBuf};

mod errors;
pub mod raw;
pub mod transaction;

#[doc(inline)]
pub use crate::errors::Error;
#[doc(inline)]
pub use crate::errors::Result;

/// The key part of a key/value pair.
/// 
/// In TiKV, keys are an ordered sequence of bytes. This has an advantage over choosing `String` as valid `UTF-8` is not required. This means that the user is permitted to store any data they wish,
/// as long as it can be represented by bytes. (Which is to say, pretty much anything!)
/// 
/// This is a *wrapper type* that implements `Deref<Target=Vec<u8>>` so it can be used like one transparently.
/// 
/// This type also implements `From` for many types. With one exception, these are all done without reallocation. Using a `&'static str`, like many examples do for simplicity, has an internal
/// allocation cost. 
/// 
/// This type wraps around an owned value, so it should be treated it like `String` or `Vec<u8>`
/// over a `&str` or `&[u8]`.
/// 
/// ```rust
/// use tikv_client::Key;
/// 
/// let static_str: &'static str = "TiKV";
/// let from_static_str = Key::from(static_str);
/// 
/// let string: String = String::from(static_str);
/// let from_string = Key::from(string);
/// assert_eq!(from_static_str, from_string);
/// 
/// let vec: Vec<u8> = static_str.as_bytes().to_vec();
/// let from_vec = Key::from(vec);
/// assert_eq!(from_static_str, from_vec);
/// 
/// let bytes = static_str.as_bytes().to_vec();
/// let from_bytes = Key::from(bytes);
/// assert_eq!(from_static_str, from_bytes);
/// ```
/// 
/// **But, you should not need to worry about all this:** Many functions which accept a `Key` 
/// accept an `Into<Key>`, which means all of the above types can be passed directly to those
/// functions.
#[derive(Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Key(Vec<u8>);

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

impl<'a> From<&'static str> for Key {
    fn from(v: &'static str) -> Key {
        let mut vec = Vec::new();
        vec.extend_from_slice(v.as_bytes());
        Key(vec)
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

impl Key {
    pub fn new(value: Vec<u8>) -> Self {
        Key(value)
    }
}

/// The value part of a key/value pair.
///
/// In TiKV, values are an ordered sequence of bytes. This has an advantage over choosing `String` as valid `UTF-8` is not required. This means that the user is permitted to store any data they wish,
/// as long as it can be represented by bytes. (Which is to say, pretty much anything!)
///
/// This is a *wrapper type* that implements `Deref<Target=Vec<u8>>` so it can be used like one transparently.
///
/// This type also implements `From` for many types. With one exception, these are all done without reallocation. Using a `&'static str`, like many examples do for simplicity, has an internal
/// allocation cost.
///
/// This type wraps around an owned value, so it should be treated it like `String` or `Vec<u8>`
/// over a `&str` or `&[u8]`.
///
/// ```rust
/// use tikv_client::Value;
///
/// let static_str: &'static str = "TiKV";
/// let from_static_str = Value::from(static_str);
///
/// let string: String = String::from(static_str);
/// let from_string = Value::from(string);
/// assert_eq!(from_static_str, from_string);
///
/// let vec: Vec<u8> = static_str.as_bytes().to_vec();
/// let from_vec = Value::from(vec);
/// assert_eq!(from_static_str, from_vec);
///
/// let bytes = static_str.as_bytes().to_vec();
/// let from_bytes = Value::from(bytes);
/// assert_eq!(from_static_str, from_bytes);
/// ```
///
/// **But, you should not need to worry about all this:** Many functions which accept a `Value`
/// accept an `Into<Value>`, which means all of the above types can be passed directly to those
/// functions.
#[derive(Default, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Value(Vec<u8>);

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

impl From<&'static str> for Value {
    fn from(v: &'static str) -> Value {
        let mut vec = Vec::new();
        vec.extend_from_slice(v.as_bytes());
        Value(vec)
    }
}

impl Deref for Value {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A key/value pair.
/// 
/// ```rust
/// # use tikv_client::{Key, Value, KvPair};
/// let key = "key";
/// let value = "value";
/// let constructed = KvPair::new(key, value);
/// let from_tuple = KvPair::from((key, value));
/// assert_eq!(constructed, from_tuple);
/// ```
/// 
/// **But, you should not need to worry about all this:** Many functions which accept a `KvPair`
/// accept an `Into<KvPair>`, which means all of the above types can be passed directly to those
/// functions.
#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct KvPair(Key, Value);

impl KvPair {
    /// Create a new `KvPair`.
    pub fn new(key: impl Into<Key>, value: impl Into<Value>) -> Self {
        KvPair(key.into(), value.into())
    }

    /// Immutably borrow the `Key` part of the `KvPair`.
    pub fn key(&self) -> &Key {
        &self.0
    }

    /// Immutably borrow the `Value` part of the `KvPair`.
    pub fn value(&self) -> &Value {
        &self.1
    }

    /// Mutably borrow the `Key` part of the `KvPair`.
    pub fn key_mut(&mut self) -> &mut Key {
        &mut self.0
    }

    /// Mutably borrow the `Value` part of the `KvPair`.
    pub fn value_mut(&mut self) -> &mut Value {
        &mut self.1
    }

    /// Set the `Key` part of the `KvPair`.
    pub fn set_key(&mut self, k: impl Into<Key>) {
        self.0 = k.into();
    }

    /// Set the `Value` part of the `KvPair`.
    pub fn set_value(&mut self, v: impl Into<Value>) {
        self.1 = v.into();
    }
}

impl<K, V> From<(K, V)> for KvPair
where K: Into<Key>, V: Into<Value> {
    fn from((k, v): (K, V)) -> Self {
        KvPair(k.into(), v.into())
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

fn transmute_bound<K>(b: Bound<&K>) -> Bound<Key>
where
    K: Into<Key> + Clone,
{
    use std::ops::Bound::*;
    match b {
        Included(k) => Included(k.clone().into()),
        Excluded(k) => Excluded(k.clone().into()),
        Unbounded => Unbounded,
    }
}
