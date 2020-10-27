// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{requests::new_scan_lock_request, resolve_locks};
use crate::{
    config::Config,
    pd::{PdClient, PdRpcClient},
    request::{KvRequest, OPTIMISTIC_BACKOFF},
    transaction::{Snapshot, Transaction},
};
use futures::executor::ThreadPool;
use kvproto::kvrpcpb;
use std::{mem, sync::Arc};
use tikv_client_common::{Result, Timestamp, TimestampExt};

const SCAN_LOCK_BATCH_SIZE: u32 = 1024; // TODO: cargo-culted value

/// The TiKV transactional `Client` is used to issue requests to the TiKV server and PD cluster.
pub struct Client {
    pd: Arc<PdRpcClient>,
    /// The thread pool for background tasks including committing secondary keys and failed
    /// transaction cleanups.
    bg_worker: ThreadPool,
    key_only: bool,
}

impl Client {
    /// Creates a new [`Client`](Client) once the [`Connect`](Connect) resolves.
    ///
    /// ```rust,no_run
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(Config::default()).await.unwrap();
    /// # });
    /// ```
    pub async fn new(config: Config) -> Result<Client> {
        let bg_worker = ThreadPool::new()?;
        // TODO: PdRpcClient::connect currently uses a blocking implementation.
        //       Make it asynchronous later.
        let pd = Arc::new(PdRpcClient::connect(&config, true).await?);
        Ok(Client {
            pd,
            bg_worker,
            key_only: false,
        })
    }

    pub fn with_key_only(&self, key_only: bool) -> Client {
        Client {
            pd: self.pd.clone(),
            bg_worker: self.bg_worker.clone(),
            key_only,
        }
    }

    /// Creates a new [`Transaction`](Transaction).
    ///
    /// Using the transaction you can issue commands like [`get`](Transaction::get) or [`set`](Transaction::set).
    ///
    /// ```rust,no_run
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(Config::default()).await.unwrap();
    /// let mut transaction = client.begin().await.unwrap();
    /// // ... Issue some commands.
    /// let commit = transaction.commit();
    /// let result: () = commit.await.unwrap();
    /// # });
    /// ```
    pub async fn begin(&self) -> Result<Transaction> {
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, false))
    }

    pub async fn begin_pessimistic(&self) -> Result<Transaction> {
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, true))
    }

    /// Creates a new [`Snapshot`](Snapshot) at the given time.
    pub fn snapshot(&self, timestamp: Timestamp) -> Snapshot {
        Snapshot::new(self.new_transaction(timestamp, false))
    }

    /// Retrieves the current [`Timestamp`](Timestamp).
    ///
    /// ```rust,no_run
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(Config::default()).await.unwrap();
    /// let timestamp = client.current_timestamp().await.unwrap();
    /// # });
    /// ```
    pub async fn current_timestamp(&self) -> Result<Timestamp> {
        self.pd.clone().get_timestamp().await
    }

    /// Cleans stale MVCC records in TiKV.
    ///
    /// It is done by:
    /// 1. resolve all locks with ts <= safepoint
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
                safepoint.clone(),
                SCAN_LOCK_BATCH_SIZE,
            );
            let res: Vec<kvrpcpb::LockInfo> =
                req.execute(self.pd.clone(), OPTIMISTIC_BACKOFF).await?;
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

    fn new_transaction(&self, timestamp: Timestamp, is_pessimistic: bool) -> Transaction {
        Transaction::new(
            timestamp,
            self.bg_worker.clone(),
            self.pd.clone(),
            self.key_only,
            is_pessimistic,
        )
    }
}
