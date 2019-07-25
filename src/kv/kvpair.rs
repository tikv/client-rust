// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{HexRepr, Key, Value};
use kvproto::kvrpcpb;
#[cfg(test)]
use proptest_derive::Arbitrary;
use std::{fmt, str};

/// A key/value pair.
///
/// ```rust
/// # use tikv_client::{Key, Value, KvPair};
/// let key = "key".to_owned();
/// let value = "value".to_owned();
/// let constructed = KvPair::new(key.clone(), value.clone());
/// let from_tuple = KvPair::from((key, value));
/// assert_eq!(constructed, from_tuple);
/// ```
///
/// Many functions which accept a `KvPair` accept an `Into<KvPair>`, which means all of the above
/// types (Like a `(Key, Value)`) can be passed directly to those functions.
#[derive(Default, Clone, Eq, PartialEq)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct KvPair(pub Key, pub Value);

impl KvPair {
    /// Create a new `KvPair`.
    #[inline]
    pub fn new(key: impl Into<Key>, value: impl Into<Value>) -> Self {
        KvPair(key.into(), value.into())
    }

    /// Immutably borrow the `Key` part of the `KvPair`.
    #[inline]
    pub fn key(&self) -> &Key {
        &self.0
    }

    /// Immutably borrow the `Value` part of the `KvPair`.
    #[inline]
    pub fn value(&self) -> &Value {
        &self.1
    }

    #[inline]
    pub fn into_key(self) -> Key {
        self.0
    }

    #[inline]
    pub fn into_value(self) -> Value {
        self.1
    }

    /// Mutably borrow the `Key` part of the `KvPair`.
    #[inline]
    pub fn key_mut(&mut self) -> &mut Key {
        &mut self.0
    }

    /// Mutably borrow the `Value` part of the `KvPair`.
    #[inline]
    pub fn value_mut(&mut self) -> &mut Value {
        &mut self.1
    }

    /// Set the `Key` part of the `KvPair`.
    #[inline]
    pub fn set_key(&mut self, k: impl Into<Key>) {
        self.0 = k.into();
    }

    /// Set the `Value` part of the `KvPair`.
    #[inline]
    pub fn set_value(&mut self, v: impl Into<Value>) {
        self.1 = v.into();
    }
}

impl<K, V> From<(K, V)> for KvPair
where
    K: Into<Key>,
    V: Into<Value>,
{
    fn from((k, v): (K, V)) -> Self {
        KvPair(k.into(), v.into())
    }
}

impl Into<(Key, Value)> for KvPair {
    fn into(self) -> (Key, Value) {
        (self.0, self.1)
    }
}

impl From<kvrpcpb::KvPair> for KvPair {
    fn from(mut pair: kvrpcpb::KvPair) -> Self {
        KvPair(Key::from(pair.take_key()), Value::from(pair.take_value()))
    }
}

impl Into<kvrpcpb::KvPair> for KvPair {
    fn into(self) -> kvrpcpb::KvPair {
        let mut result = kvrpcpb::KvPair::default();
        let (key, value) = self.into();
        result.set_key(key.into());
        result.set_value(value.into());
        result
    }
}

impl AsRef<Key> for KvPair {
    fn as_ref(&self) -> &Key {
        &self.0
    }
}

impl AsRef<Value> for KvPair {
    fn as_ref(&self) -> &Value {
        &self.1
    }
}

impl fmt::Debug for KvPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let KvPair(key, value) = self;
        match str::from_utf8(&value.0) {
            Ok(s) => write!(f, "KvPair({}, {:?})", HexRepr(&key.0), s),
            Err(_) => write!(f, "KvPair({}, {})", HexRepr(&key.0), HexRepr(&value.0)),
        }
    }
}
