// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{requests::new_scan_lock_request, resolve_locks};
use crate::{
    pd::{PdClient, PdRpcClient},
    request::KvRequest,
    transaction::{Snapshot, Transaction},
};
use futures::executor::ThreadPool;
use kvproto::kvrpcpb;
use std::sync::Arc;
use tikv_client_common::{Config, Result, Timestamp, TimestampExt};

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
        Ok(self.new_transaction(timestamp))
    }

    /// Creates a new [`Snapshot`](Snapshot) at the given time.
    pub fn snapshot(&self, timestamp: Timestamp) -> Snapshot {
        Snapshot::new(self.new_transaction(timestamp))
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

    pub async fn gc(&self, safepoint: Timestamp) -> Result<bool> {
        /* TODO
           1. scan all locks with ts <= safepoint, resolve them all
           2. (ignored) delete_range
           3. update safepoint to PD
        */

        // scan all locks with ts <= safepoint
        let mut locks: Vec<kvrpcpb::LockInfo> = vec![];
        let mut start_key = vec![];
        loop {
            let req =
                new_scan_lock_request(start_key.clone(), safepoint.clone(), SCAN_LOCK_BATCH_SIZE);
            let res: Vec<kvrpcpb::LockInfo> = req.execute(self.pd.clone()).await?;
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

    fn new_transaction(&self, timestamp: Timestamp) -> Transaction {
        Transaction::new(
            timestamp,
            self.bg_worker.clone(),
            self.pd.clone(),
            self.key_only,
        )
    }
}
