// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{requests::new_scan_lock_request, resolve_locks};
use crate::{
    backoff::{DEFAULT_REGION_BACKOFF, OPTIMISTIC_BACKOFF},
    config::Config,
    pd::{PdClient, PdRpcClient},
    request::Plan,
    timestamp::TimestampExt,
    transaction::{Snapshot, Transaction, TransactionOptions},
    Result,
};
use log::debug;
use std::{mem, sync::Arc};
use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

// FIXME: cargo-culted value
const SCAN_LOCK_BATCH_SIZE: u32 = 1024;

/// The TiKV transactional `Client` is used to interact with TiKV using transactional requests.
///
/// Transactions support optimistic and pessimistic modes. For more details see the SIG-transaction
/// [docs](https://github.com/tikv/sig-transaction/tree/master/doc/tikv#optimistic-and-pessimistic-transactions).
///
/// Begin a [`Transaction`] by calling [`begin_optimistic`](Client::begin_optimistic) or
/// [`begin_pessimistic`](Client::begin_pessimistic). A transaction must be rolled back or committed.
///
/// Besides transactions, the client provides some further functionality:
/// - `gc`: trigger a GC process which clears stale data in the cluster.
/// - `current_timestamp`: get the current `Timestamp` from PD.
/// - `snapshot`: get a [`Snapshot`] of the database at a specified timestamp.
/// A `Snapshot` is a read-only transaction.
///
/// The returned results of transactional requests are [`Future`](std::future::Future)s that must be
/// awaited to execute.
pub struct Client {
    pd: Arc<PdRpcClient>,
}

impl Client {
    /// Create a transactional [`Client`] and connect to the TiKV cluster.
    ///
    /// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for
    /// PD must be provided, not the TiKV nodes. It's important to include more than one PD endpoint
    /// (include all endpoints, if possible), this helps avoid having a single point of failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// # });
    /// ```
    pub async fn new<S: Into<String>>(pd_endpoints: Vec<S>) -> Result<Client> {
        debug!("creating transactional client");
        Self::new_with_config(pd_endpoints, Config::default()).await
    }

    /// Create a transactional [`Client`] with a custom configuration, and connect to the TiKV cluster.
    ///
    /// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for
    /// PD must be provided, not the TiKV nodes. It's important to include more than one PD endpoint
    /// (include all endpoints, if possible), this helps avoid having a single point of failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::time::Duration;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new_with_config(
    ///     vec!["192.168.0.100"],
    ///     Config::default().with_timeout(Duration::from_secs(60)),
    /// ).await.unwrap();
    /// # });
    /// ```
    pub async fn new_with_config<S: Into<String>>(
        pd_endpoints: Vec<S>,
        config: Config,
    ) -> Result<Client> {
        debug!("creating transactional client with custom configuration");
        let pd_endpoints: Vec<String> = pd_endpoints.into_iter().map(Into::into).collect();
        let pd = Arc::new(PdRpcClient::connect(&pd_endpoints, &config, true).await?);
        Ok(Client { pd })
    }

    /// Creates a new optimistic [`Transaction`].
    ///
    /// Use the transaction to issue requests like [`get`](Transaction::get) or
    /// [`put`](Transaction::put).
    ///
    /// Write operations do not lock data in TiKV, thus the commit request may fail due to a write
    /// conflict.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut transaction = client.begin_optimistic().await.unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn begin_optimistic(&self) -> Result<Transaction> {
        debug!("creating new optimistic transaction");
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, TransactionOptions::new_optimistic()))
    }

    /// Creates a new pessimistic [`Transaction`].
    ///
    /// Write operations will lock the data until committed, thus commit requests should not suffer
    /// from write conflicts.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut transaction = client.begin_pessimistic().await.unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn begin_pessimistic(&self) -> Result<Transaction> {
        debug!("creating new pessimistic transaction");
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, TransactionOptions::new_pessimistic()))
    }

    /// Create a new customized [`Transaction`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient, TransactionOptions};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut transaction = client
    ///     .begin_with_options(TransactionOptions::default().use_async_commit())
    ///     .await
    ///     .unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn begin_with_options(&self, options: TransactionOptions) -> Result<Transaction> {
        debug!("creating new customized transaction");
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, options))
    }

    /// Create a new [`Snapshot`](Snapshot) at the given [`Timestamp`](Timestamp).
    pub fn snapshot(&self, timestamp: Timestamp, options: TransactionOptions) -> Snapshot {
        debug!("creating new snapshot");
        Snapshot::new(self.new_transaction(timestamp, options.read_only()))
    }

    /// Retrieve the current [`Timestamp`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let timestamp = client.current_timestamp().await.unwrap();
    /// # });
    /// ```
    pub async fn current_timestamp(&self) -> Result<Timestamp> {
        self.pd.clone().get_timestamp().await
    }

    /// Request garbage collection (GC) of the TiKV cluster.
    ///
    /// GC deletes MVCC records whose timestamp is lower than the given `safepoint`.
    ///
    /// For each key, the last mutation record (unless it's a deletion) before `safepoint` is retained.
    ///
    /// GC is performed by:
    /// 1. resolving all locks with timestamp <= `safepoint`
    /// 2. updating PD's known safepoint
    ///
    /// This is a simplified version of [GC in TiDB](https://docs.pingcap.com/tidb/stable/garbage-collection-overview).
    /// We skip the second step "delete ranges" which is an optimization for TiDB.
    pub async fn gc(&self, safepoint: Timestamp) -> Result<bool> {
        debug!("invoking transactional gc request");
        // scan all locks with ts <= safepoint
        let mut locks: Vec<kvrpcpb::LockInfo> = vec![];
        let mut start_key = vec![];
        loop {
            let req = new_scan_lock_request(
                mem::take(&mut start_key),
                safepoint.version(),
                SCAN_LOCK_BATCH_SIZE,
            );

            let plan = crate::request::PlanBuilder::new(self.pd.clone(), req)
                .resolve_lock(OPTIMISTIC_BACKOFF)
                .multi_region()
                .retry_region(DEFAULT_REGION_BACKOFF)
                .merge(crate::request::Collect)
                .plan();
            let res: Vec<kvrpcpb::LockInfo> = plan.execute().await?;

            if res.is_empty() {
                break;
            }
            start_key = res.last().unwrap().key.clone();
            start_key.push(0);
            locks.extend(res);
        }

        // resolve locks
        resolve_locks(locks, self.pd.clone()).await?;

        // update safepoint to PD
        let res: bool = self
            .pd
            .clone()
            .update_safepoint(safepoint.version())
            .await?;
        if !res {
            info!("new safepoint != user-specified safepoint");
        }
        Ok(res)
    }

    fn new_transaction(&self, timestamp: Timestamp, options: TransactionOptions) -> Transaction {
        Transaction::new(timestamp, self.pd.clone(), options)
    }
}
