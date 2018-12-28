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

#![recursion_limit = "128"]
#![type_length_limit = "2097152"]
use std::{convert::AsRef, ops::Deref, path::PathBuf, time::Duration};

use futures::Future;
use serde_derive::*;

pub use crate::errors::{Error, Result};

pub mod errors;
pub mod raw;
mod rpc;
pub mod transaction;

#[derive(Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Key(Vec<u8>);
#[derive(Default, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Value(Vec<u8>);
#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct KvPair(Key, Value);

impl From<Vec<u8>> for Key {
    fn from(vec: Vec<u8>) -> Key {
        Key(vec)
    }
}

impl<'a> From<&'a [u8]> for Key {
    fn from(s: &'a [u8]) -> Key {
        Key(s.to_vec())
    }
}

impl AsRef<Key> for Key {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl AsRef<[u8]> for Key {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for Key {
    type Target = [u8];

    fn deref<'a>(&'a self) -> &'a Self::Target {
        &self.0
    }
}

impl Key {
    fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for Value {
    fn from(vec: Vec<u8>) -> Value {
        Value(vec)
    }
}

impl<'a> From<&'a [u8]> for Value {
    fn from(s: &'a [u8]) -> Value {
        Value(s.to_vec())
    }
}

impl AsRef<Value> for Value {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl AsRef<[u8]> for Value {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for Value {
    type Target = [u8];

    fn deref<'a>(&'a self) -> &'a Self::Target {
        &self.0
    }
}

impl Value {
    fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl KvPair {
    pub fn new(key: Key, value: Value) -> Self {
        KvPair(key, value)
    }

    pub fn key(&self) -> &Key {
        &self.0
    }

    pub fn value(&self) -> &Value {
        &self.1
    }

    pub fn into_inner(self) -> (Key, Value) {
        (self.0, self.1)
    }
}

impl From<(Key, Value)> for KvPair {
    fn from(pair: (Key, Value)) -> KvPair {
        KvPair(pair.0, pair.1)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pd_endpoints: Vec<String>,
    ca_path: Option<PathBuf>,
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
    timeout: Duration,
}

impl Config {
    pub fn new(pd_endpoints: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Config {
            pd_endpoints: pd_endpoints.into_iter().map(Into::into).collect(),
            ca_path: None,
            cert_path: None,
            key_path: None,
            timeout: Duration::from_secs(2),
        }
    }

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

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

pub type KvFuture<Resp> = Box<Future<Item = Resp, Error = Error>>;
