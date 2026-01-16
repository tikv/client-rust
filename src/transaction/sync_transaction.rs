use crate::{
    transaction::Mutation, BoundRange, Key, KvPair, Result, Timestamp, Transaction, Value,
};
use std::sync::Arc;

/// A synchronous transaction.
///
/// This is a wrapper around the async [`Transaction`] that provides blocking methods.
/// All operations block the current thread until completed.
pub struct SyncTransaction {
    inner: Transaction,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl SyncTransaction {
    pub(crate) fn new(inner: Transaction, runtime: Arc<tokio::runtime::Runtime>) -> Self {
        Self { inner, runtime }
    }

    /// Get the value associated with the given key.
    pub fn get(&mut self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.runtime.block_on(self.inner.get(key))
    }

    /// Get the value associated with the given key, and lock the key.
    pub fn get_for_update(&mut self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.runtime.block_on(self.inner.get_for_update(key))
    }

    /// Check if the given key exists.
    pub fn key_exists(&mut self, key: impl Into<Key>) -> Result<bool> {
        self.runtime.block_on(self.inner.key_exists(key))
    }

    /// Get the values associated with the given keys.
    pub fn batch_get(
        &mut self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.runtime.block_on(self.inner.batch_get(keys))
    }

    /// Get the values associated with the given keys, and lock the keys.
    pub fn batch_get_for_update(
        &mut self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<Vec<KvPair>> {
        self.runtime.block_on(self.inner.batch_get_for_update(keys))
    }

    /// Scan a range and return the key-value pairs.
    pub fn scan(
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.runtime.block_on(self.inner.scan(range, limit))
    }

    /// Scan a range and return only the keys.
    pub fn scan_keys(
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = Key>> {
        self.runtime.block_on(self.inner.scan_keys(range, limit))
    }

    /// Scan a range in reverse order.
    pub fn scan_reverse(
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.runtime.block_on(self.inner.scan_reverse(range, limit))
    }

    /// Scan keys in a range in reverse order.
    pub fn scan_keys_reverse(
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = Key>> {
        self.runtime
            .block_on(self.inner.scan_keys_reverse(range, limit))
    }

    /// Set the value associated with the given key.
    pub fn put(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        self.runtime.block_on(self.inner.put(key, value))
    }

    /// Insert the key-value pair. Returns an error if the key already exists.
    pub fn insert(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        self.runtime.block_on(self.inner.insert(key, value))
    }

    /// Delete the given key.
    pub fn delete(&mut self, key: impl Into<Key>) -> Result<()> {
        self.runtime.block_on(self.inner.delete(key))
    }

    /// Apply multiple mutations atomically.
    pub fn batch_mutate(&mut self, mutations: impl IntoIterator<Item = Mutation>) -> Result<()> {
        self.runtime.block_on(self.inner.batch_mutate(mutations))
    }

    /// Lock the given keys without associating any values.
    pub fn lock_keys(&mut self, keys: impl IntoIterator<Item = impl Into<Key>>) -> Result<()> {
        self.runtime.block_on(self.inner.lock_keys(keys))
    }

    /// Commit the transaction.
    pub fn commit(&mut self) -> Result<Option<Timestamp>> {
        self.runtime.block_on(self.inner.commit())
    }

    /// Rollback the transaction.
    pub fn rollback(&mut self) -> Result<()> {
        self.runtime.block_on(self.inner.rollback())
    }

    /// Send a heart beat message to keep the transaction alive.
    pub fn send_heart_beat(&mut self) -> Result<u64> {
        self.runtime.block_on(self.inner.send_heart_beat())
    }
}
