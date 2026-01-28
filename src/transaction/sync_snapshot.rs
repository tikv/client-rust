use crate::{BoundRange, Key, KvPair, Result, Snapshot, Value};
use std::sync::Arc;

/// A synchronous read-only snapshot.
///
/// This is a wrapper around the async [`Snapshot`] that provides blocking methods.
/// All operations block the current thread until completed.
pub struct SyncSnapshot {
    inner: Snapshot,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl SyncSnapshot {
    pub(crate) fn new(inner: Snapshot, runtime: Arc<tokio::runtime::Runtime>) -> Self {
        Self { inner, runtime }
    }

    /// Get the value associated with the given key.
    pub fn get(&mut self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.runtime.block_on(self.inner.get(key))
    }

    /// Check whether the key exists.
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

    /// Scan a range, return at most `limit` key-value pairs that lie in the range.
    pub fn scan(
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.runtime.block_on(self.inner.scan(range, limit))
    }

    /// Scan a range, return at most `limit` keys that lie in the range.
    pub fn scan_keys(
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = Key>> {
        self.runtime.block_on(self.inner.scan_keys(range, limit))
    }

    /// Similar to scan, but in the reverse direction.
    pub fn scan_reverse(
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.runtime.block_on(self.inner.scan_reverse(range, limit))
    }

    /// Similar to scan_keys, but in the reverse direction.
    pub fn scan_keys_reverse(
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = Key>> {
        self.runtime
            .block_on(self.inner.scan_keys_reverse(range, limit))
    }
}
