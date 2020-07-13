// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::transaction::{Snapshot, Transaction};

use futures::executor::ThreadPool;
use grpcio::EnvBuilder;
use std::sync::Arc;
use tikv_client_common::{security::SecurityManager, Config, Result, Timestamp};
use tikv_client_pd::{PdClient, PdRpcClient};
use tikv_client_store::TikvConnect;

/// The TiKV transactional `Client` is used to issue requests to the TiKV server and PD cluster.
pub struct Client {
    pd: Arc<PdRpcClient>,
    kv_connect: Arc<TikvConnect>,
    /// The thread pool for background tasks including committing secondary keys and failed
    /// transaction cleanups.
    bg_worker: ThreadPool,
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
        let pd = Arc::new(PdRpcClient::connect(&config).await?);
        // FIXME: only for test. The initialization should be either
        // 1. same as pd (get from pd)
        // 2. inside the TikvConnect::new()

        const CQ_COUNT: usize = 1;
        const CLIENT_PREFIX: &str = "tikv-client";
        let env = Arc::new(
            EnvBuilder::new()
                .cq_count(CQ_COUNT)
                .name_prefix(thread_name!(CLIENT_PREFIX))
                .build(),
        );
        let security_mgr = Arc::new(
            if let (Some(ca_path), Some(cert_path), Some(key_path)) =
                (&config.ca_path, &config.cert_path, &config.key_path)
            {
                SecurityManager::load(ca_path, cert_path, key_path)?
            } else {
                SecurityManager::default()
            },
        );

        let kv_connect = Arc::new(TikvConnect::new(env, security_mgr));
        Ok(Client {
            pd,
            kv_connect,
            bg_worker,
        })
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

    fn new_transaction(&self, timestamp: Timestamp) -> Transaction {
        Transaction::new(
            timestamp,
            self.bg_worker.clone(),
            self.pd.clone(),
            self.kv_connect.clone(),
        )
    }
}
