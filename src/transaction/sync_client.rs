use crate::{
    request::plan::CleanupLocksResult,
    transaction::{
        client::Client, sync_snapshot::SyncSnapshot, sync_transaction::SyncTransaction,
        ResolveLocksOptions,
    },
    BoundRange, Config, Result, Timestamp, TransactionOptions,
};
use std::sync::Arc;

/// Synchronous TiKV transactional client.
///
/// This is a synchronous wrapper around the async [`TransactionClient`](crate::TransactionClient).
/// All methods block the current thread until completion.
///
/// For async operations, use [`TransactionClient`](crate::TransactionClient) instead.
pub struct SyncTransactionClient {
    client: Client,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl SyncTransactionClient {
    /// Create a synchronous transactional [`SyncTransactionClient`] and connect to the TiKV cluster.
    ///
    /// See usage example in the documentation of [`TransactionClient::new`](crate::TransactionClient::new).
    pub fn new<S: Into<String>>(pd_endpoints: Vec<S>) -> Result<Self> {
        Self::new_with_config(pd_endpoints, Config::default())
    }

    /// Create a synchronous transactional [`SyncTransactionClient`] with a custom configuration.
    ///
    /// See usage example in the documentation of [`TransactionClient::new_with_config`](crate::TransactionClient::new_with_config).
    pub fn new_with_config<S: Into<String>>(pd_endpoints: Vec<S>, config: Config) -> Result<Self> {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?,
        );
        let client = runtime.block_on(Client::new_with_config(pd_endpoints, config))?;
        Ok(Self { client, runtime })
    }

    /// Creates a new optimistic [`SyncTransaction`].
    ///
    /// Use the transaction to issue requests like [`get`](SyncTransaction::get) or
    /// [`put`](SyncTransaction::put).
    ///
    /// This is a synchronous version of [`TransactionClient::begin_optimistic`](crate::TransactionClient::begin_optimistic).
    pub fn begin_optimistic(&self) -> Result<SyncTransaction> {
        let inner = self.runtime.block_on(self.client.begin_optimistic())?;
        Ok(SyncTransaction::new(inner, Arc::clone(&self.runtime)))
    }

    /// Creates a new pessimistic [`SyncTransaction`].
    ///
    /// This is a synchronous version of [`TransactionClient::begin_pessimistic`](crate::TransactionClient::begin_pessimistic).
    pub fn begin_pessimistic(&self) -> Result<SyncTransaction> {
        let inner = self.runtime.block_on(self.client.begin_pessimistic())?;
        Ok(SyncTransaction::new(inner, Arc::clone(&self.runtime)))
    }

    /// Create a new customized [`SyncTransaction`].
    ///
    /// This is a synchronous version of [`TransactionClient::begin_with_options`](crate::TransactionClient::begin_with_options).
    pub fn begin_with_options(&self, options: TransactionOptions) -> Result<SyncTransaction> {
        let inner = self
            .runtime
            .block_on(self.client.begin_with_options(options))?;
        Ok(SyncTransaction::new(inner, Arc::clone(&self.runtime)))
    }

    /// Create a new read-only [`SyncSnapshot`] at the given [`Timestamp`].
    ///
    /// This is a synchronous version of [`TransactionClient::snapshot`](crate::TransactionClient::snapshot).
    pub fn snapshot(&self, timestamp: Timestamp, options: TransactionOptions) -> SyncSnapshot {
        let inner = self.client.snapshot(timestamp, options);
        SyncSnapshot::new(inner, Arc::clone(&self.runtime))
    }

    /// Retrieve the current [`Timestamp`].
    ///
    /// This is a synchronous version of [`TransactionClient::current_timestamp`](crate::TransactionClient::current_timestamp).
    pub fn current_timestamp(&self) -> Result<Timestamp> {
        self.runtime.block_on(self.client.current_timestamp())
    }

    /// Request garbage collection (GC) of the TiKV cluster.
    ///
    /// This is a synchronous version of [`TransactionClient::gc`](crate::TransactionClient::gc).
    pub fn gc(&self, safepoint: Timestamp) -> Result<bool> {
        self.runtime.block_on(self.client.gc(safepoint))
    }

    /// Clean up all locks in the specified range.
    ///
    /// This is a synchronous version of [`TransactionClient::cleanup_locks`](crate::TransactionClient::cleanup_locks).
    pub fn cleanup_locks(
        &self,
        range: impl Into<BoundRange>,
        safepoint: &Timestamp,
        options: ResolveLocksOptions,
    ) -> Result<CleanupLocksResult> {
        self.runtime
            .block_on(self.client.cleanup_locks(range, safepoint, options))
    }

    /// Cleans up all keys in a range and quickly reclaim disk space.
    ///
    /// The range can span over multiple regions.
    ///
    /// Note that the request will directly delete data from RocksDB, and all MVCC will be erased.
    ///
    /// This interface is intended for special scenarios that resemble operations like "drop table" or "drop database" in TiDB.
    ///
    /// This is a synchronous version of [`TransactionClient::unsafe_destroy_range`](crate::TransactionClient::unsafe_destroy_range).
    pub fn unsafe_destroy_range(&self, range: impl Into<BoundRange>) -> Result<()> {
        self.runtime
            .block_on(self.client.unsafe_destroy_range(range))
    }

    /// Scan all locks in the specified range.
    ///
    /// This is only available for integration tests.
    ///
    /// Note: `batch_size` must be >= expected number of locks.
    ///
    /// This is a synchronous version of [`TransactionClient::scan_locks`](crate::TransactionClient::scan_locks).
    #[cfg(feature = "integration-tests")]
    pub fn scan_locks(
        &self,
        safepoint: &Timestamp,
        range: impl Into<BoundRange>,
        batch_size: u32,
    ) -> Result<Vec<crate::proto::kvrpcpb::LockInfo>> {
        self.runtime
            .block_on(self.client.scan_locks(safepoint, range, batch_size))
    }
}

impl Clone for SyncTransactionClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            runtime: Arc::clone(&self.runtime),
        }
    }
}
