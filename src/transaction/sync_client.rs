use crate::{
    request::plan::CleanupLocksResult,
    transaction::{client::Client, ResolveLocksOptions},
    BoundRange, Config, Result, Snapshot, Timestamp, Transaction, TransactionOptions,
};
use futures::executor::block_on;

pub struct SyncTransactionClient {
    client: Client,
}

impl SyncTransactionClient {
    pub fn new<S: Into<String>>(pd_endpoints: Vec<S>) -> Result<Self> {
        Self::new_with_config(pd_endpoints, Config::default())
    }

    pub fn new_with_config<S: Into<String>>(pd_endpoints: Vec<S>, config: Config) -> Result<Self> {
        let client = block_on(Client::new_with_config(pd_endpoints, config))?;
        Ok(Self { client })
    }

    pub fn begin_optimistic(&self) -> Result<Transaction> {
        block_on(self.client.begin_optimistic())
    }

    pub fn begin_pessimistic(&self) -> Result<Transaction> {
        block_on(self.client.begin_pessimistic())
    }

    pub fn begin_with_options(&self, options: TransactionOptions) -> Result<Transaction> {
        block_on(self.client.begin_with_options(options))
    }

    pub fn snapshot(&self, timestamp: Timestamp, options: TransactionOptions) -> Snapshot {
        self.client.snapshot(timestamp, options)
    }

    pub fn current_timestamp(&self) -> Result<Timestamp> {
        block_on(self.client.current_timestamp())
    }

    pub fn gc(&self, safepoint: Timestamp) -> Result<bool> {
        block_on(self.client.gc(safepoint))
    }

    pub fn cleanup_locks(
        &self,
        range: impl Into<BoundRange>,
        safepoint: &Timestamp,
        options: ResolveLocksOptions,
    ) -> Result<CleanupLocksResult> {
        block_on(self.client.cleanup_locks(range, safepoint, options))
    }

    pub fn unsafe_destroy_range(&self, range: impl Into<BoundRange>) -> Result<()> {
        block_on(self.client.unsafe_destroy_range(range))
    }
}

impl Clone for SyncTransactionClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
        }
    }
}
