// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{requests::new_scan_lock_request, resolve_locks};
use crate::{
    config::Config,
    pd::{PdClient, PdRpcClient},
    request::{KvRequest, RetryOptions},
    timestamp::TimestampExt,
    transaction::{Snapshot, Transaction, TransactionOptions},
    Result,
};
use futures::executor::ThreadPool;
use std::{mem, sync::Arc};
use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

const SCAN_LOCK_BATCH_SIZE: u32 = 1024; // TODO: cargo-culted value

/// The TiKV transactional `Client` is used to interact with TiKV using transactional (MVCC) requests.
///
/// A [`Transaction`](crate::transaction::Transaction) provides a SQL-like interface.
/// It begins with a [`begin`](Client::begin) or [`begin_pessimistic`](Client::begin_pessimistic) request
/// and ends with a `rollback` or `commit` request.
/// If a `Transaction` is dropped before it's rolled back or committed, it is automatically rolled back.
///
/// Transaction supports optimistic and pessimistic modes, for mroe deatils, check our
/// [SIG-transaction](https://github.com/tikv/sig-transaction/tree/master/doc/tikv#optimistic-and-pessimistic-transactions).
///
/// Besides transaction, the client provides some utility methods:
/// - `gc`: execute a GC process which clear stale data. It is not stablized yet.
/// - `current_timestamp`: get the current `Timestamp`.
/// - `snapshot`: get the [`Snapshot`](crate::transaction::Snapshot) of the database at a certain timestamp.
/// A `Snapshot` is a read-only transaction.
///
/// The returned results of transactional requests are [`Future`](std::future::Future)s that must be awaited to execute.
pub struct Client {
    pd: Arc<PdRpcClient>,
    /// The thread pool for background tasks including committing secondary keys and failed
    /// transaction cleanups.
    bg_worker: ThreadPool,
}

impl Client {
    /// Creates a transactional [`Client`](Client).
    ///
    /// It's important to **include more than one PD endpoint** (include all, if possible!)
    /// This helps avoid having a *single point of failure*.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// # });
    /// ```
    pub async fn new<S: Into<String>>(pd_endpoints: Vec<S>) -> Result<Client> {
        Self::new_with_config(pd_endpoints, Config::default()).await
    }

    /// Creates a transactional [`Client`](Client).
    ///
    /// It's important to **include more than one PD endpoint** (include all, if possible!)
    /// This helps avoid having a *single point of failure*.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// # });
    /// ```
    pub async fn new_with_config<S: Into<String>>(
        pd_endpoints: Vec<S>,
        config: Config,
    ) -> Result<Client> {
        let pd_endpoints: Vec<String> = pd_endpoints.into_iter().map(Into::into).collect();
        let bg_worker = ThreadPool::new()?;
        let pd = Arc::new(PdRpcClient::connect(&pd_endpoints, &config, true).await?);
        Ok(Client { pd, bg_worker })
    }

    /// Creates a new [`Transaction`](Transaction) in optimistic mode.
    ///
    /// Using the transaction you can issue commands like [`get`](Transaction::get) or [`put`](Transaction::put).
    ///
    /// Write operations do not lock data in TiKV, thus commit request may fail due to write conflict.
    ///
    /// For details, check our [SIG-transaction](https://github.com/tikv/sig-transaction/tree/master/doc/tikv#optimistic-and-pessimistic-transactions).
    ///
    /// # Examples
    /// ```rust,no_run
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut transaction = client.begin_optimistic().await.unwrap();
    /// // ... Issue some commands.
    /// let commit = transaction.commit();
    /// let result = commit.await.unwrap();
    /// # });
    /// ```
    pub async fn begin_optimistic(&self) -> Result<Transaction> {
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, TransactionOptions::new_optimistic()))
    }

    /// Creates a new [`Transaction`](Transaction) in pessimistic mode.
    ///
    /// Write operations will lock the data until commit, thus commit requests should not suffer from write conflict.
    /// For details, check our [SIG-transaction](https://github.com/tikv/sig-transaction/tree/master/doc/tikv#optimistic-and-pessimistic-transactions).
    ///
    /// # Examples
    /// ```rust,no_run
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut transaction = client.begin_pessimistic().await.unwrap();
    /// // ... Issue some commands.
    /// let commit = transaction.commit();
    /// let result = commit.await.unwrap();
    /// # });
    /// ```
    pub async fn begin_pessimistic(&self) -> Result<Transaction> {
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, TransactionOptions::new_pessimistic()))
    }

    /// Creates a new customized [`Transaction`](Transaction).
    ///
    /// # Examples
    /// ```rust,no_run
    /// use tikv_client::{Config, TransactionClient, TransactionOptions};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut transaction = client
    ///     .begin_with_options(TransactionOptions::default().use_async_commit())
    ///     .await
    ///     .unwrap();
    /// // ... Issue some commands.
    /// let commit = transaction.commit();
    /// let result = commit.await.unwrap();
    /// # });
    /// ```
    pub async fn begin_with_options(&self, options: TransactionOptions) -> Result<Transaction> {
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, options))
    }

    /// Creates a new [`Snapshot`](Snapshot) at the given [`Timestamp`](Timestamp).
    pub fn snapshot(&self, timestamp: Timestamp, options: TransactionOptions) -> Snapshot {
        Snapshot::new(self.new_transaction(timestamp, options.read_only()))
    }

    /// Retrieves the current [`Timestamp`](Timestamp).
    ///
    /// # Examples
    /// ```rust,no_run
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let timestamp = client.current_timestamp().await.unwrap();
    /// # });
    /// ```
    pub async fn current_timestamp(&self) -> Result<Timestamp> {
        self.pd.clone().get_timestamp().await
    }

    /// Cleans MVCC records whose timestamp is lower than the given `timestamp` in TiKV.
    ///
    /// For each key, the last mutation record (unless it's a deletion) before `safepoint` is retained.
    ///
    /// It is done by:
    /// 1. resolve all locks with ts <= `safepoint`
    /// 2. update safepoint to PD
    ///
    /// This is a simplified version of [GC in TiDB](https://docs.pingcap.com/tidb/stable/garbage-collection-overview).
    /// We omit the second step "delete ranges" which is an optimization for TiDB.
    pub async fn gc(&self, safepoint: Timestamp) -> Result<bool> {
        // scan all locks with ts <= safepoint
        let mut locks: Vec<kvrpcpb::LockInfo> = vec![];
        let mut start_key = vec![];
        loop {
            let req = new_scan_lock_request(
                mem::take(&mut start_key),
                safepoint.version(),
                SCAN_LOCK_BATCH_SIZE,
            );
            let res: Vec<kvrpcpb::LockInfo> = req
                .execute(self.pd.clone(), RetryOptions::default_optimistic())
                .await?;
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
        Transaction::new(timestamp, self.bg_worker.clone(), self.pd.clone(), options)
    }
}
