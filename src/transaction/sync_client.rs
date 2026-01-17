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
    /// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for
    /// PD must be provided, not the TiKV nodes. It's important to include more than one PD endpoint
    /// (include all endpoints, if possible), this helps avoid having a single point of failure.
    ///
    /// This is a synchronous version of [`TransactionClient::new`](crate::TransactionClient::new).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::SyncTransactionClient;
    /// let client = SyncTransactionClient::new(vec!["192.168.0.100"]).unwrap();
    /// ```
    pub fn new<S: Into<String>>(pd_endpoints: Vec<S>) -> Result<Self> {
        Self::new_with_config(pd_endpoints, Config::default())
    }

    /// Create a synchronous transactional [`SyncTransactionClient`] with a custom configuration.
    ///
    /// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for
    /// PD must be provided, not the TiKV nodes. It's important to include more than one PD endpoint
    /// (include all endpoints, if possible), this helps avoid having a single point of failure.
    ///
    /// This is a synchronous version of [`TransactionClient::new_with_config`](crate::TransactionClient::new_with_config).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, SyncTransactionClient};
    /// # use std::time::Duration;
    /// let client = SyncTransactionClient::new_with_config(
    ///     vec!["192.168.0.100"],
    ///     Config::default().with_timeout(Duration::from_secs(60)),
    /// )
    /// .unwrap();
    /// ```
    pub fn new_with_config<S: Into<String>>(pd_endpoints: Vec<S>, config: Config) -> Result<Self> {
        let runtime =
            Arc::new(tokio::runtime::Runtime::new()?);
        let client = runtime.block_on(Client::new_with_config(pd_endpoints, config))?;
        Ok(Self { client, runtime })
    }

    /// Creates a new optimistic [`SyncTransaction`].
    ///
    /// Use the transaction to issue requests like [`get`](SyncTransaction::get) or
    /// [`put`](SyncTransaction::put).
    ///
    /// Write operations do not lock data in TiKV, thus the commit request may fail due to a write
    /// conflict.
    ///
    /// This is a synchronous version of [`TransactionClient::begin_optimistic`](crate::TransactionClient::begin_optimistic).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::SyncTransactionClient;
    /// let client = SyncTransactionClient::new(vec!["192.168.0.100"]).unwrap();
    /// let mut transaction = client.begin_optimistic().unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().unwrap();
    /// ```
    pub fn begin_optimistic(&self) -> Result<SyncTransaction> {
        let inner = self.runtime.block_on(self.client.begin_optimistic())?;
        Ok(SyncTransaction::new(inner, Arc::clone(&self.runtime)))
    }

    /// Creates a new pessimistic [`SyncTransaction`].
    ///
    /// Write operations will lock the data until committed, thus commit requests should not suffer
    /// from write conflicts.
    ///
    /// This is a synchronous version of [`TransactionClient::begin_pessimistic`](crate::TransactionClient::begin_pessimistic).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::SyncTransactionClient;
    /// let client = SyncTransactionClient::new(vec!["192.168.0.100"]).unwrap();
    /// let mut transaction = client.begin_pessimistic().unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().unwrap();
    /// ```
    pub fn begin_pessimistic(&self) -> Result<SyncTransaction> {
        let inner = self.runtime.block_on(self.client.begin_pessimistic())?;
        Ok(SyncTransaction::new(inner, Arc::clone(&self.runtime)))
    }

    /// Create a new customized [`SyncTransaction`].
    ///
    /// This is a synchronous version of [`TransactionClient::begin_with_options`](crate::TransactionClient::begin_with_options).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{SyncTransactionClient, TransactionOptions};
    /// let client = SyncTransactionClient::new(vec!["192.168.0.100"]).unwrap();
    /// let mut transaction = client
    ///     .begin_with_options(TransactionOptions::default().use_async_commit())
    ///     .unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().unwrap();
    /// ```
    pub fn begin_with_options(&self, options: TransactionOptions) -> Result<SyncTransaction> {
        let inner = self
            .runtime
            .block_on(self.client.begin_with_options(options))?;
        Ok(SyncTransaction::new(inner, Arc::clone(&self.runtime)))
    }

    /// Create a new [`SyncSnapshot`] at the given [`Timestamp`].
    ///
    /// This is a synchronous version of [`TransactionClient::snapshot`](crate::TransactionClient::snapshot).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{SyncTransactionClient, TransactionOptions};
    /// let client = SyncTransactionClient::new(vec!["192.168.0.100"]).unwrap();
    /// let timestamp = client.current_timestamp().unwrap();
    /// let snapshot = client.snapshot(timestamp, TransactionOptions::default());
    /// ```
    pub fn snapshot(&self, timestamp: Timestamp, options: TransactionOptions) -> SyncSnapshot {
        let inner = self.client.snapshot(timestamp, options);
        SyncSnapshot::new(inner, Arc::clone(&self.runtime))
    }

    /// Retrieve the current [`Timestamp`].
    ///
    /// This is a synchronous version of [`TransactionClient::current_timestamp`](crate::TransactionClient::current_timestamp).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::SyncTransactionClient;
    /// let client = SyncTransactionClient::new(vec!["192.168.0.100"]).unwrap();
    /// let timestamp = client.current_timestamp().unwrap();
    /// ```
    pub fn current_timestamp(&self) -> Result<Timestamp> {
        self.runtime.block_on(self.client.current_timestamp())
    }

    /// Request garbage collection (GC) of the TiKV cluster.
    ///
    /// GC deletes MVCC records whose timestamp is lower than the given `safepoint`. We must guarantee
    /// that all transactions started before this timestamp had committed. We can keep an active
    /// transaction list in application to decide which is the minimal start timestamp of them.
    ///
    /// For each key, the last mutation record (unless it's a deletion) before `safepoint` is retained.
    ///
    /// GC is performed by:
    /// 1. resolving all locks with timestamp <= `safepoint`
    /// 2. updating PD's known safepoint
    ///
    /// This is a simplified version of [GC in TiDB](https://docs.pingcap.com/tidb/stable/garbage-collection-overview).
    /// We skip the second step "delete ranges" which is an optimization for TiDB.
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
