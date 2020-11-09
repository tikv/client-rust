// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::{PdClient, PdRpcClient},
    request::{KvRequest, OPTIMISTIC_BACKOFF, PESSIMISTIC_BACKOFF},
    timestamp::TimestampExt,
    transaction::{buffer::Buffer, requests::*},
    BoundRange, Error, ErrorKind, Key, KvPair, Result, Value,
};
use derive_new::new;
use futures::{executor::ThreadPool, prelude::*, stream::BoxStream};
use std::{iter, mem, ops::RangeBounds, sync::Arc};
use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

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
    key_only: bool,
    for_update_ts: u64,
    is_pessimistic: bool,
}

impl Transaction {
    pub(crate) fn new(
        timestamp: Timestamp,
        bg_worker: ThreadPool,
        rpc: Arc<PdRpcClient>,
        key_only: bool,
        is_pessimistic: bool,
    ) -> Transaction {
        Transaction {
            timestamp,
            buffer: Default::default(),
            bg_worker,
            rpc,
            key_only,
            for_update_ts: 0,
            is_pessimistic,
        }
    }

    /// Gets the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
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
                new_mvcc_get_request(key, self.timestamp.clone())
                    .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            })
            .await
    }

    /// Pessimistically lock and get the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin_pessimistic().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let result: Option<Value> = txn.get_for_update(key).await.unwrap();
    /// // now the key "TiKV" is locked, other transactions cannot modify it
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn get_for_update(&mut self, key: impl Into<Key>) -> Result<Option<Value>> {
        if !self.is_pessimistic {
            Err(ErrorKind::InvalidTransactionType.into())
        } else {
            let key = key.into();
            self.pessimistic_lock(iter::once(key.clone())).await?;
            self.buffer
                .get_or_else(key, |key| {
                    new_mvcc_get_request(key, self.timestamp.clone())
                        .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
                })
                .await
        }
    }

    /// Gets the values associated with the given keys, skipping non-existent entries.
    ///
    /// Non-existent entries will be skipped. The order of the keys is not retained.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let result: HashMap<Key, Value> = txn
    ///     .batch_get(keys)
    ///     .await
    ///     .unwrap()
    ///     .map(|pair| (pair.0, pair.1))
    ///     .collect();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn batch_get(
        &self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<impl Iterator<Item = KvPair>> {
        let timestamp = self.timestamp.clone();
        let rpc = self.rpc.clone();
        self.buffer
            .batch_get_or_else(keys.into_iter().map(|k| k.into()), move |keys| {
                new_mvcc_get_batch_request(keys, timestamp).execute(rpc, OPTIMISTIC_BACKOFF)
            })
            .await
    }

    /// Gets the values associated with the given keys, skipping non-existent entries.
    ///
    /// Non-existent entries will be skipped. The order of the keys is not retained.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin_pessimistic().await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let result: HashMap<Key, Value> = txn
    ///     .batch_get_for_update(keys)
    ///     .await
    ///     .unwrap()
    ///     .map(|pair| (pair.0, pair.1))
    ///     .collect();
    /// // now "TiKV" and "TiDB" are both locked
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn batch_get_for_update(
        &mut self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<impl Iterator<Item = KvPair>> {
        if !self.is_pessimistic {
            Err(ErrorKind::InvalidTransactionType.into())
        } else {
            let keys: Vec<Key> = keys.into_iter().map(|it| it.into()).collect();
            self.pessimistic_lock(keys.clone()).await?;
            self.batch_get(keys).await
        }
    }

    /// Scan queries continuous key-value pairs in range.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, KvPair, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// let key1: Key = b"TiKV".to_vec().into();
    /// let key2: Key = b"TiDB".to_vec().into();
    /// let result: Vec<KvPair> = txn
    ///     .scan(key1..key2, 10)
    ///     .await
    ///     .unwrap()
    ///     .collect();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn scan(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = KvPair>> {
        let timestamp = self.timestamp.clone();
        let rpc = self.rpc.clone();

        let key_only = self.key_only;
        self.buffer
            .scan_and_fetch(range.into(), limit, move |new_range, new_limit| {
                new_mvcc_scan_request(new_range, timestamp, new_limit, key_only)
                    .execute(rpc, OPTIMISTIC_BACKOFF)
            })
            .await
    }

    pub fn scan_reverse(&self, _range: impl RangeBounds<Key>) -> BoxStream<Result<KvPair>> {
        unimplemented!()
    }

    /// Sets the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// txn.put(key, val);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn put(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        let key = key.into();
        if self.is_pessimistic {
            self.pessimistic_lock(iter::once(key.clone())).await?;
        }
        self.buffer.put(key, value.into()).await;
        Ok(())
    }

    /// Deletes the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(Config::new(vec!["192.168.0.100", "192.168.0.101"])).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// txn.delete(key);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn delete(&mut self, key: impl Into<Key>) -> Result<()> {
        let key = key.into();
        if self.is_pessimistic {
            self.pessimistic_lock(iter::once(key.clone())).await?;
        }
        self.buffer.delete(key).await;
        Ok(())
    }

    /// Locks the given keys.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(Config::default()).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// txn.lock_keys(vec!["TiKV".to_owned(), "Rust".to_owned()]);
    /// // ... Do some actions.
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn lock_keys(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> Result<()> {
        for key in keys {
            self.buffer.lock(key.into()).await;
        }
        Ok(())
    }

    /// Commits the actions of the transaction.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(Config::default()).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// // ... Do some actions.
    /// let req = txn.commit();
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn commit(&mut self) -> Result<()> {
        TwoPhaseCommitter::new(
            self.buffer.to_proto_mutations().await,
            self.timestamp.version(),
            self.bg_worker.clone(),
            self.rpc.clone(),
            self.for_update_ts,
        )
        .commit()
        .await
    }

    /// Pessimistically lock the keys
    async fn pessimistic_lock(
        &mut self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<()> {
        let mut keys: Vec<Vec<u8>> = keys
            .into_iter()
            .map(|it| it.into())
            .map(|it: Key| it.into())
            .collect();
        keys.sort();
        let primary_lock = keys[0].clone();
        let lock_ttl = DEFAULT_LOCK_TTL;
        let for_update_ts = self.rpc.clone().get_timestamp().await.unwrap().version();
        self.for_update_ts = std::cmp::max(self.for_update_ts, for_update_ts);
        new_pessimistic_lock_request(
            keys,
            primary_lock,
            self.timestamp.version(),
            lock_ttl,
            for_update_ts,
        )
        .execute(self.rpc.clone(), PESSIMISTIC_BACKOFF)
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
    for_update_ts: u64,
    #[new(default)]
    committed: bool,
    #[new(default)]
    undetermined: bool,
}

impl TwoPhaseCommitter {
    async fn commit(mut self) -> Result<()> {
        if self.mutations.is_empty() {
            self.committed = true;
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
        if self.for_update_ts > 0 {
            new_pessimistic_prewrite_request(
                self.mutations.clone(),
                primary_lock,
                self.start_version,
                lock_ttl,
                self.for_update_ts,
            )
            .execute(self.rpc.clone(), PESSIMISTIC_BACKOFF)
            .await
        } else {
            new_prewrite_request(
                self.mutations.clone(),
                primary_lock,
                self.start_version,
                lock_ttl,
            )
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
        }
    }

    /// Commits the primary key and returns the commit version
    async fn commit_primary(&mut self) -> Result<u64> {
        let primary_key = vec![self.mutations[0].key.clone().into()];
        let commit_version = self.rpc.clone().get_timestamp().await?.version();
        new_commit_request(primary_key, self.start_version, commit_version)
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
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
        let mutations = mem::take(&mut self.mutations);
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
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
    }

    fn rollback(&mut self) -> impl Future<Output = Result<()>> + 'static {
        let mutations = mem::take(&mut self.mutations);
        let keys = mutations
            .into_iter()
            .map(|mutation| mutation.key.into())
            .collect();
        if self.for_update_ts > 0 {
            new_pessimistic_rollback_request(keys, self.start_version, self.for_update_ts)
                .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
        } else {
            new_batch_rollback_request(keys, self.start_version)
                .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
        }
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
