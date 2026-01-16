use crate::{
    request::plan::CleanupLocksResult,
    transaction::{
        client::Client, sync_snapshot::SyncSnapshot, sync_transaction::SyncTransaction,
        ResolveLocksOptions,
    },
    BoundRange, Config, Result, Timestamp, TransactionOptions,
};
use std::sync::Arc;

pub struct SyncTransactionClient {
    client: Client,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl SyncTransactionClient {
    pub fn new<S: Into<String>>(pd_endpoints: Vec<S>) -> Result<Self> {
        Self::new_with_config(pd_endpoints, Config::default())
    }

    pub fn new_with_config<S: Into<String>>(pd_endpoints: Vec<S>, config: Config) -> Result<Self> {
        let runtime =
            Arc::new(tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime"));
        let client = runtime.block_on(Client::new_with_config(pd_endpoints, config))?;
        Ok(Self { client, runtime })
    }

    pub fn begin_optimistic(&self) -> Result<SyncTransaction> {
        let inner = self.runtime.block_on(self.client.begin_optimistic())?;
        Ok(SyncTransaction::new(inner, Arc::clone(&self.runtime)))
    }

    pub fn begin_pessimistic(&self) -> Result<SyncTransaction> {
        let inner = self.runtime.block_on(self.client.begin_pessimistic())?;
        Ok(SyncTransaction::new(inner, Arc::clone(&self.runtime)))
    }

    pub fn begin_with_options(&self, options: TransactionOptions) -> Result<SyncTransaction> {
        let inner = self
            .runtime
            .block_on(self.client.begin_with_options(options))?;
        Ok(SyncTransaction::new(inner, Arc::clone(&self.runtime)))
    }

    pub fn snapshot(&self, timestamp: Timestamp, options: TransactionOptions) -> SyncSnapshot {
        let inner = self.client.snapshot(timestamp, options);
        SyncSnapshot::new(inner, Arc::clone(&self.runtime))
    }

    pub fn current_timestamp(&self) -> Result<Timestamp> {
        self.runtime.block_on(self.client.current_timestamp())
    }

    pub fn gc(&self, safepoint: Timestamp) -> Result<bool> {
        self.runtime.block_on(self.client.gc(safepoint))
    }

    pub fn cleanup_locks(
        &self,
        range: impl Into<BoundRange>,
        safepoint: &Timestamp,
        options: ResolveLocksOptions,
    ) -> Result<CleanupLocksResult> {
        self.runtime
            .block_on(self.client.cleanup_locks(range, safepoint, options))
    }

    pub fn unsafe_destroy_range(&self, range: impl Into<BoundRange>) -> Result<()> {
        self.runtime
            .block_on(self.client.unsafe_destroy_range(range))
    }

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
