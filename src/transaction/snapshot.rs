// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{BoundRange, Key, KvPair, Result, Transaction, Value};
use derive_new::new;
use futures::stream::BoxStream;
use std::ops::RangeBounds;

/// A read-only transaction which reads at the given timestamp.
///
/// It behaves as if the snapshot was taken at the given timestamp,
/// i.e. it can read operations happened before the timestamp,
/// but ignores operations after the timestamp.
///
/// See the [Transaction](struct@crate::Transaction) docs for more information on the methods.
#[derive(new)]
pub struct Snapshot {
    transaction: Transaction,
}

impl Snapshot {
    /// Get the value associated with the given key.
    pub async fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.transaction.get(key).await
    }

    /// Check whether the key exists.
    pub async fn key_exists(&self, key: impl Into<Key>) -> Result<bool> {
        self.transaction.key_exists(key).await
    }

    /// Get the values associated with the given keys.
    pub async fn batch_get(
        &self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.transaction.batch_get(keys).await
    }

    /// Scan a range, return at most `limit` key-value pairs that lying in the range.
    pub async fn scan(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.transaction.scan(range, limit).await
    }

    /// Scan a range, return at most `limit` keys that lying in the range.
    pub async fn scan_keys(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = Key>> {
        self.transaction.scan_keys(range, limit).await
    }

    /// Unimplemented. Similar to scan, but in the reverse direction.
    #[allow(dead_code)]
    fn scan_reverse(&self, range: impl RangeBounds<Key>) -> BoxStream<Result<KvPair>> {
        self.transaction.scan_reverse(range)
    }
}
