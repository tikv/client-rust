// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::{PdClient, PdRpcClient},
    request::KvRequest,
    transaction::{buffer::Buffer, requests::*, Timestamp},
    Error, ErrorKind, Key, KvPair, Result, Value,
};

use derive_new::new;
use futures::executor::ThreadPool;
use futures::prelude::*;
use futures::stream::BoxStream;
use kvproto::kvrpcpb;
use std::mem;
use std::ops::RangeBounds;
use std::sync::Arc;

/// A undo-able set of actions on the dataset.
///
/// Using a transaction you can prepare a set of actions (such as `get`, or `set`) on data at a
/// particular timestamp obtained from the placement driver.
///
/// Once a transaction is commited, a new commit timestamp is obtained from the placement driver.
///
/// Create a new transaction from a timestamp using `new`.
///
/// ```rust,no_run
/// use tikv_client::{Config, TransactionClient};
/// use futures::prelude::*;
/// # futures::executor::block_on(async {
/// let client = TransactionClient::new(Config::default()).await.unwrap();
/// let txn = client.begin().await.unwrap();
/// # });
/// ```
pub struct Transaction {
    timestamp: Timestamp,
    buffer: Buffer,
    bg_worker: ThreadPool,
    rpc: Arc<PdRpcClient>,
}

impl Transaction {
    pub(crate) fn new(
        timestamp: Timestamp,
        bg_worker: ThreadPool,
        rpc: Arc<PdRpcClient>,
    ) -> Transaction {
        Transaction {
            timestamp,
            buffer: Default::default(),
            bg_worker,
            rpc,
        }
    }

    /// Gets the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = Client::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let result: Option<Value> = txn.get(key).await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        let key = key.into();
        self.buffer
            .get_or_else(key, |key| {
                new_mvcc_get_request(key, self.timestamp).execute(self.rpc.clone())
            })
            .await
    }

    /// Gets the values associated with the given keys.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = Client::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let result: HashMap<Key, Value> = txn
    ///     .batch_get(keys)
    ///     .await
    ///     .unwrap()
    ///     .filter_map(|(k, v)| v.map(move |v| (k, v))).collect();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn batch_get(
        &self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<impl Iterator<Item = (Key, Option<Value>)>> {
        let timestamp = self.timestamp;
        let rpc = self.rpc.clone();
        self.buffer
            .batch_get_or_else(keys.into_iter().map(|k| k.into()), move |keys| {
                new_mvcc_get_batch_request(keys, timestamp).execute(rpc)
            })
            .await
    }

    pub fn scan(&self, _range: impl RangeBounds<Key>) -> BoxStream<Result<KvPair>> {
        unimplemented!()
    }

    pub fn scan_reverse(&self, _range: impl RangeBounds<Key>) -> BoxStream<Result<KvPair>> {
        unimplemented!()
    }

    /// Sets the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = Client::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// txn.set(key, val);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn set(&self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        self.buffer.put(key.into(), value.into());
        Ok(())
    }

    /// Deletes the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = Client::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// txn.delete(key);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn delete(&self, key: impl Into<Key>) -> Result<()> {
        self.buffer.delete(key.into());
        Ok(())
    }

    /// Locks the given keys.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = Client::new(Config::default()).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// txn.lock_keys(vec!["TiKV".to_owned(), "Rust".to_owned()]);
    /// // ... Do some actions.
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn lock_keys(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> Result<()> {
        for key in keys {
            self.buffer.lock(key.into());
        }
        Ok(())
    }

    /// Commits the actions of the transaction.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = Client::new(Config::default()).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// // ... Do some actions.
    /// let req = txn.commit();
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn commit(&mut self) -> Result<()> {
        TwoPhaseCommitter::new(
            self.buffer.to_proto_mutations(),
            self.timestamp.into_version(),
            self.bg_worker.clone(),
            self.rpc.clone(),
        )
        .commit()
        .await
    }
}

/// The default TTL of a lock in milliseconds
const DEFAULT_LOCK_TTL: u64 = 3000;

#[derive(new)]
struct TwoPhaseCommitter {
    mutations: Vec<kvrpcpb::Mutation>,
    start_version: u64,
    bg_worker: ThreadPool,
    rpc: Arc<PdRpcClient>,
    #[new(default)]
    committed: bool,
    #[new(default)]
    undetermined: bool,
}

impl TwoPhaseCommitter {
    async fn commit(mut self) -> Result<()> {
        if self.mutations.is_empty() {
            return Ok(());
        }
        self.prewrite().await?;
        match self.commit_primary().await {
            Ok(commit_version) => {
                self.committed = true;
                self.bg_worker
                    .clone()
                    .spawn_ok(self.commit_secondary(commit_version).map(|res| {
                        if let Err(e) = res {
                            warn!("Failed to commit secondary keys: {}", e);
                        }
                    }));
                Ok(())
            }
            Err(e) => {
                if self.undetermined {
                    Err(Error::undetermined_error(e))
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn prewrite(&mut self) -> Result<()> {
        let primary_lock = self.mutations[0].key.clone().into();
        // TODO: calculate TTL for big transactions
        let lock_ttl = DEFAULT_LOCK_TTL;
        new_prewrite_request(
            self.mutations.clone(),
            primary_lock,
            self.start_version,
            lock_ttl,
        )
        .execute(self.rpc.clone())
        .await
    }

    /// Commits the primary key and returns the commit version
    async fn commit_primary(&mut self) -> Result<u64> {
        let primary_key = vec![self.mutations[0].key.clone().into()];
        let commit_version = self.rpc.clone().get_timestamp().await?.into_version();
        new_commit_request(primary_key, self.start_version, commit_version)
            .execute(self.rpc.clone())
            .inspect_err(|e| {
                // We don't know whether the transaction is committed or not if we fail to receive
                // the response. Then, we mark the transaction as undetermined and propagate the
                // error to the user.
                if let ErrorKind::Grpc(_) = e.kind() {
                    self.undetermined = true;
                }
            })
            .await?;
        Ok(commit_version)
    }

    async fn commit_secondary(mut self, commit_version: u64) -> Result<()> {
        let mutations = mem::replace(&mut self.mutations, Vec::default());
        // No need to commit secondary keys when there is only one key
        if mutations.len() == 1 {
            return Ok(());
        }

        let keys = mutations
            .into_iter()
            .skip(1) // skip primary key
            .map(|mutation| mutation.key.into())
            .collect();
        new_commit_request(keys, self.start_version, commit_version)
            .execute(self.rpc.clone())
            .await
    }

    fn rollback(&mut self) -> impl Future<Output = Result<()>> + 'static {
        let mutations = mem::replace(&mut self.mutations, Vec::default());
        let keys = mutations
            .into_iter()
            .map(|mutation| mutation.key.into())
            .collect();
        new_batch_rollback_request(keys, self.start_version).execute(self.rpc.clone())
    }
}

impl Drop for TwoPhaseCommitter {
    fn drop(&mut self) {
        if !self.committed {
            self.bg_worker.clone().spawn_ok(self.rollback().map(|res| {
                if let Err(e) = res {
                    warn!("Failed to rollback: {}", e);
                }
            }))
        }
    }
}
