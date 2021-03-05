// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    backoff::Backoff,
    pd::{PdClient, PdRpcClient},
    request::{Collect, CollectError, Plan, PlanBuilder, RetryOptions},
    timestamp::TimestampExt,
    transaction::{buffer::Buffer, lowering::*},
    BoundRange, Error, Key, KvPair, Result, Value,
};
use derive_new::new;
use fail::fail_point;
use futures::{executor::ThreadPool, prelude::*, stream::BoxStream};
use std::{iter, ops::RangeBounds, sync::Arc};
use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};
use tokio::{sync::RwLock, time::Duration};

/// An undo-able set of actions on the dataset.
///
/// Using a transaction you can prepare a set of actions (such as `get`, or `put`) on data at a
/// particular timestamp called `start_ts` obtained from the placement driver.
/// Once a transaction is commited, a new timestamp called `commit_ts` is obtained from the placement driver.
///
/// The snapshot isolation in TiKV ensures that a transaction behaves as if it operates on the snapshot taken at
/// `start_ts` and its mutations take effect at `commit_ts`.
/// In other words, the transaction can read mutations with `commit_ts` <= its `start_ts`,
/// and its mutations are readable for transactions with `start_ts` >= its `commit_ts`.
///
/// Mutations, or write operations made in a transaction are buffered locally and sent at the time of commit,
/// except for pessimisitc locking.
/// In pessimistic mode, all write operations or `xxx_for_update` operations will first acquire pessimistic locks in TiKV.
/// A lock exists until the transaction is committed (in the first phase of 2PC) or rolled back, or it exceeds its Time To Live (TTL).
///
/// For details, the [SIG-Transaction](https://github.com/tikv/sig-transaction)
/// provides materials explaining designs and implementations of multiple features in TiKV transactions.
///
///
/// # Examples
/// ```rust,no_run
/// use tikv_client::{Config, TransactionClient};
/// use futures::prelude::*;
/// # futures::executor::block_on(async {
/// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
/// let txn = client.begin_optimistic().await.unwrap();
/// # });
/// ```
pub struct Transaction<PdC: PdClient = PdRpcClient> {
    status: Arc<RwLock<TransactionStatus>>,
    timestamp: Timestamp,
    buffer: Buffer,
    bg_worker: ThreadPool,
    rpc: Arc<PdC>,
    options: TransactionOptions,
    is_heartbeat_started: bool,
}

impl<PdC: PdClient> Transaction<PdC> {
    pub(crate) fn new(
        timestamp: Timestamp,
        bg_worker: ThreadPool,
        rpc: Arc<PdC>,
        options: TransactionOptions,
    ) -> Transaction<PdC> {
        let status = if options.read_only {
            TransactionStatus::ReadOnly
        } else {
            TransactionStatus::Active
        };
        Transaction {
            status: Arc::new(RwLock::new(status)),
            timestamp,
            buffer: Default::default(),
            bg_worker,
            rpc,
            options,
            is_heartbeat_started: false,
        }
    }

    /// Create a new 'get' request
    ///
    /// Once resolved this request will result in the fetching of the value associated with the
    /// given key.
    ///
    /// Retuning `Ok(None)` indicates the key does not exist in TiKV.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let result: Option<Value> = txn.get(key).await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.check_allow_operation().await?;
        let timestamp = self.timestamp.clone();
        let rpc = self.rpc.clone();
        let key = key.into();
        let retry_options = self.options.retry_options.clone();

        self.buffer
            .get_or_else(key, |key| async move {
                let request = new_get_request(key, timestamp);
                let plan = PlanBuilder::new(rpc, request)
                    .single_region()
                    .await?
                    .resolve_lock(retry_options.lock_backoff)
                    .retry_region(retry_options.region_backoff)
                    .post_process_default()
                    .plan();
                plan.execute().await
            })
            .await
    }

    /// Create a `get for udpate` request.
    /// Once resolved this request will pessimistically lock and fetch the latest
    /// value associated with the given key at **current timestamp**.
    ///
    /// The "current timestamp" (also called `for_update_ts` of the request) is fetched immediately from PD.
    ///
    /// Note: The behavior of this command does not follow snapshot isolation. It is similar to `select for update` in TiDB,
    /// which is similar to that in MySQL. It reads the latest value (using current timestamp),
    /// and the value is not cached in the local buffer.
    /// So normal `get`-like commands after `get_for_update` will not be influenced, they still read values at `start_ts`.
    ///
    /// Different from `get`, this request does not distinguish between empty values and non-existent keys
    /// , i.e. querying non-existent keys will result in empty values.
    ///
    /// It can only be used in pessimistic mode.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_pessimistic().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let result: Value = txn.get_for_update(key).await.unwrap().unwrap();
    /// // now the key "TiKV" is locked, other transactions cannot modify it
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn get_for_update(&mut self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.check_allow_operation().await?;
        if !self.is_pessimistic() {
            Err(Error::InvalidTransactionType)
        } else {
            let key = key.into();
            let mut values = self.pessimistic_lock(iter::once(key.clone()), true).await?;
            assert!(values.len() == 1);
            Ok(values.pop().unwrap())
        }
    }

    /// Check whether the key exists.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_pessimistic().await.unwrap();
    /// let exists = txn.key_exists("k1".to_owned()).await.unwrap();
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn key_exists(&self, key: impl Into<Key>) -> Result<bool> {
        let key = key.into();
        Ok(self.scan_keys(key.clone()..=key, 1).await?.next().is_some())
    }

    /// Create a new 'batch get' request.
    ///
    /// Once resolved this request will result in the fetching of the values associated with the
    /// given keys.
    ///
    /// Non-existent entries will not appear in the result. The order of the keys is not retained in the result.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
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
        self.check_allow_operation().await?;
        let timestamp = self.timestamp.clone();
        let rpc = self.rpc.clone();
        let retry_options = self.options.retry_options.clone();

        self.buffer
            .batch_get_or_else(keys.into_iter().map(|k| k.into()), move |keys| async move {
                let request = new_batch_get_request(keys, timestamp);
                let plan = PlanBuilder::new(rpc, request)
                    .resolve_lock(retry_options.lock_backoff)
                    .multi_region()
                    .retry_region(retry_options.region_backoff)
                    .merge(Collect)
                    .plan();
                plan.execute()
                    .await
                    .map(|r| r.into_iter().map(Into::into).collect())
            })
            .await
    }

    /// Create a new 'batch get for update' request.
    ///
    /// Once resolved this request will pessimistically lock the keys and
    /// fetch the values associated with the given keys.
    ///
    /// Note: The behavior of this command does not follow snapshot isolation. It is similar to `select for update` in TiDB,
    /// which is similar to that in MySQL. It reads the latest value (using current timestamp),
    /// and the value is not cached in the local buffer.
    /// So normal `get`-like commands after `batch_get_for_update` will not be influenced, they still read values at `start_ts`.
    ///
    /// Different from `batch_get`, this request does not distinguish between empty values and non-existent keys
    /// , i.e. querying non-existent keys will result in empty values.
    ///
    /// It can only be used in pessimistic mode.
    ///
    /// # Examples
    /// ```rust,no_run,compile_fail
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
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
    // This is temporarily disabled because we cannot correctly match the keys and values.
    // See `impl KvRequest for kvrpcpb::PessimisticLockRequest` for details.
    #[allow(dead_code)]
    async fn batch_get_for_update(
        &mut self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.check_allow_operation().await?;
        if !self.is_pessimistic() {
            Err(Error::InvalidTransactionType)
        } else {
            let keys: Vec<Key> = keys.into_iter().map(|it| it.into()).collect();
            self.pessimistic_lock(keys.clone(), false).await?;
            self.batch_get(keys).await
        }
    }

    /// Create a new 'scan' request.
    ///
    /// Once resolved this request will result in a `Vec` of key-value pairs that lies in the specified range.
    ///
    /// If the number of eligible key-value pairs are greater than `limit`,
    /// only the first `limit` pairs are returned, ordered by the key.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, KvPair, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
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
        self.scan_inner(range, limit, false).await
    }

    /// Create a new 'scan' request that only returns the keys.
    ///
    /// Once resolved this request will result in a `Vec` of keys that lies in the specified range.
    ///
    /// If the number of eligible keys are greater than `limit`,
    /// only the first `limit` keys are returned, ordered by the key.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, KvPair, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key1: Key = b"TiKV".to_vec().into();
    /// let key2: Key = b"TiDB".to_vec().into();
    /// let result: Vec<Key> = txn
    ///     .scan_keys(key1..key2, 10)
    ///     .await
    ///     .unwrap()
    ///     .collect();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn scan_keys(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = Key>> {
        Ok(self
            .scan_inner(range, limit, true)
            .await?
            .map(KvPair::into_key))
    }

    /// Create a 'scan_reverse' request.
    ///
    /// Similar to [`scan`](Transaction::scan), but in the reverse direction.
    pub(crate) fn scan_reverse(&self, _range: impl RangeBounds<Key>) -> BoxStream<Result<KvPair>> {
        unimplemented!()
    }

    /// Sets the value associated with the given key.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// txn.put(key, val);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn put(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        self.check_allow_operation().await?;
        let key = key.into();
        if self.is_pessimistic() {
            self.pessimistic_lock(iter::once(key.clone()), false)
                .await?;
        }
        self.buffer.put(key, value.into()).await;
        Ok(())
    }

    /// Inserts the value associated with the given key.
    /// It has a constraint that key should not exist before.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// txn.insert(key, val);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn insert(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        self.check_allow_operation().await?;
        let key = key.into();
        if self.buffer.get(&key).await.is_some() {
            return Err(Error::DuplicateKeyInsertion);
        }
        if self.is_pessimistic() {
            self.pessimistic_lock(iter::once(key.clone()), false)
                .await?;
        }
        self.buffer.insert(key, value.into()).await;
        Ok(())
    }

    /// Deletes the given key.
    ///
    /// Deleting a non-existent key will not result in an error.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// txn.delete(key);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn delete(&mut self, key: impl Into<Key>) -> Result<()> {
        self.check_allow_operation().await?;
        let key = key.into();
        if self.is_pessimistic() {
            self.pessimistic_lock(iter::once(key.clone()), false)
                .await?;
        }
        self.buffer.delete(key).await;
        Ok(())
    }

    /// Lock the given keys without mutating value (at the time of commit).
    ///
    /// In optimistic mode, write conflicts are not checked until commit.
    /// So use this command to indicate that
    /// "I do not want to commit if the value associated with this key has been modified".
    /// It's useful to avoid *write skew* anomaly.
    ///
    /// In pessimistic mode, it is similar to [`batch_get_for_update`](Transaction::batch_get_for_update),
    /// except that it does not read values.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// txn.lock_keys(vec!["TiKV".to_owned(), "Rust".to_owned()]);
    /// // ... Do some actions.
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn lock_keys(
        &mut self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<()> {
        self.check_allow_operation().await?;
        match self.options.kind {
            TransactionKind::Optimistic => {
                for key in keys {
                    self.buffer.lock(key.into()).await;
                }
            }
            TransactionKind::Pessimistic(_) => {
                self.pessimistic_lock(keys.into_iter().map(|k| k.into()), false)
                    .await?;
            }
        }
        Ok(())
    }

    /// Commits the actions of the transaction. On success, we return the commit timestamp (or None
    /// if there was nothing to commit).
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Config, Timestamp, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// // ... Do some actions.
    /// let req = txn.commit();
    /// let result: Timestamp = req.await.unwrap().unwrap();
    /// # });
    /// ```
    pub async fn commit(&mut self) -> Result<Option<Timestamp>> {
        {
            let mut status = self.status.write().await;
            if !matches!(
                *status,
                TransactionStatus::StartedCommit | TransactionStatus::Active
            ) {
                return Err(Error::OperationAfterCommitError);
            }
            *status = TransactionStatus::StartedCommit;
        }

        let primary_key = self.buffer.get_primary_key().await;
        let mutations = self.buffer.to_proto_mutations().await;
        if mutations.is_empty() {
            assert!(primary_key.is_none());
            return Ok(None);
        }

        self.start_auto_heartbeat().await;

        let res = Committer::new(
            primary_key,
            mutations,
            self.timestamp.clone(),
            self.bg_worker.clone(),
            self.rpc.clone(),
            self.options.clone(),
        )
        .commit()
        .await;

        if res.is_ok() {
            let mut status = self.status.write().await;
            *status = TransactionStatus::Committed;
        }
        res
    }

    /// Rollback the transaction.
    ///
    /// If it succeeds, all mutations made by this transaciton will not take effect.
    pub async fn rollback(&mut self) -> Result<()> {
        {
            let status = self.status.read().await;
            if !matches!(
                *status,
                TransactionStatus::StartedRollback | TransactionStatus::Active
            ) {
                return Err(Error::OperationAfterCommitError);
            }
        }

        {
            let mut status = self.status.write().await;
            *status = TransactionStatus::StartedRollback;
        }

        let primary_key = self.buffer.get_primary_key().await;
        let mutations = self.buffer.to_proto_mutations().await;
        let res = Committer::new(
            primary_key,
            mutations,
            self.timestamp.clone(),
            self.bg_worker.clone(),
            self.rpc.clone(),
            self.options.clone(),
        )
        .rollback()
        .await;

        if res.is_ok() {
            let mut status = self.status.write().await;
            *status = TransactionStatus::Rolledback;
        }
        res
    }

    /// Send a heart beat message to keep the transaction alive on the server and update its TTL.
    ///
    /// Returns the TTL set on the lock by the server.
    pub async fn send_heart_beat(&mut self) -> Result<u64> {
        self.check_allow_operation().await?;
        let primary_key = match self.buffer.get_primary_key().await {
            Some(k) => k,
            None => return Err(Error::NoPrimaryKey),
        };
        let request = new_heart_beat_request(self.timestamp.clone(), primary_key, DEFAULT_LOCK_TTL);
        let plan = PlanBuilder::new(self.rpc.clone(), request)
            .single_region()
            .await?
            .resolve_lock(self.options.retry_options.lock_backoff.clone())
            .retry_region(self.options.retry_options.region_backoff.clone())
            .post_process_default()
            .plan();
        plan.execute().await
    }

    async fn scan_inner(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
        key_only: bool,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.check_allow_operation().await?;
        let timestamp = self.timestamp.clone();
        let rpc = self.rpc.clone();
        let retry_options = self.options.retry_options.clone();

        self.buffer
            .scan_and_fetch(
                range.into(),
                limit,
                move |new_range, new_limit| async move {
                    let request = new_scan_request(new_range, timestamp, new_limit, key_only);
                    let plan = PlanBuilder::new(rpc, request)
                        .resolve_lock(retry_options.lock_backoff)
                        .multi_region()
                        .retry_region(retry_options.region_backoff)
                        .merge(Collect)
                        .plan();
                    plan.execute()
                        .await
                        .map(|r| r.into_iter().map(Into::into).collect())
                },
            )
            .await
    }

    /// Pessimistically lock the keys.
    ///
    /// Once resolved it acquires a lock on the key in TiKV.
    /// The lock prevents other transactions from mutating the entry until it is released.
    ///
    /// Only valid for pessimistic transactions, panics if called on an optimistic transaction.
    async fn pessimistic_lock(
        &mut self,
        keys: impl IntoIterator<Item = Key>,
        need_value: bool,
    ) -> Result<Vec<Option<Value>>> {
        assert!(
            matches!(self.options.kind, TransactionKind::Pessimistic(_)),
            "`pessimistic_lock` is only valid to use with pessimistic transactions"
        );

        let keys: Vec<Key> = keys.into_iter().collect();
        let first_key = keys[0].clone();
        let primary_lock = self.buffer.get_primary_key_or(&first_key).await;
        let lock_ttl = DEFAULT_LOCK_TTL;
        let for_update_ts = self.rpc.clone().get_timestamp().await.unwrap();
        self.options.push_for_update_ts(for_update_ts.clone());
        let request = new_pessimistic_lock_request(
            keys.clone().into_iter(),
            primary_lock,
            self.timestamp.clone(),
            lock_ttl,
            for_update_ts,
            need_value,
        );
        let plan = PlanBuilder::new(self.rpc.clone(), request)
            .resolve_lock(self.options.retry_options.lock_backoff.clone())
            .multi_region()
            .retry_region(self.options.retry_options.region_backoff.clone())
            .merge(Collect)
            .plan();
        let values = plan
            .execute()
            .await
            .map(|r| r.into_iter().map(Into::into).collect());

        self.start_auto_heartbeat().await;

        for key in keys {
            self.buffer.lock(key).await;
        }

        values
    }

    /// Checks if the transaction can perform arbitrary operations.
    async fn check_allow_operation(&self) -> Result<()> {
        let status = self.status.read().await;
        match *status {
            TransactionStatus::ReadOnly | TransactionStatus::Active => Ok(()),
            TransactionStatus::Committed
            | TransactionStatus::Rolledback
            | TransactionStatus::StartedCommit
            | TransactionStatus::StartedRollback
            | TransactionStatus::Dropped => Err(Error::OperationAfterCommitError),
        }
    }

    fn is_pessimistic(&self) -> bool {
        matches!(self.options.kind, TransactionKind::Pessimistic(_))
    }

    async fn start_auto_heartbeat(&mut self) {
        if !self.options.auto_heartbeat || self.is_heartbeat_started {
            return;
        }
        self.is_heartbeat_started = true;

        let status = self.status.clone();
        let primary_key = self
            .buffer
            .get_primary_key()
            .await
            .expect("Primary key should exist");
        let start_ts = self.timestamp.clone();
        let region_backoff = self.options.retry_options.region_backoff.clone();
        let rpc = self.rpc.clone();

        let heartbeat_task = async move {
            loop {
                tokio::time::sleep(DEFAULT_HEARTBEAT_INTERVAL).await;
                {
                    let status = status.read().await;
                    if matches!(
                        *status,
                        TransactionStatus::Rolledback
                            | TransactionStatus::Committed
                            | TransactionStatus::Dropped
                    ) {
                        break;
                    }
                }
                let current_ts = rpc.clone().get_timestamp().await?;
                let request = new_heart_beat_request(
                    start_ts.clone(),
                    primary_key.clone(),
                    (current_ts.physical - start_ts.physical) as u64 + DEFAULT_LOCK_TTL,
                );
                let plan = PlanBuilder::new(rpc.clone(), request)
                    .single_region()
                    .await?
                    .retry_region(region_backoff.clone())
                    .plan();
                plan.execute().await?;
            }
            Ok::<(), Error>(())
        };

        tokio::spawn(async {
            if let Err(err) = heartbeat_task.await {
                error!("Error: While sending heartbeat. {}", err);
            }
        });
    }
}

impl<PdC: PdClient> Drop for Transaction<PdC> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        let mut status = futures::executor::block_on(self.status.write());
        if *status == TransactionStatus::Active {
            match self.options.check_level {
                CheckLevel::Panic => {
                    panic!("Dropping an active transaction. Consider commit or rollback it.")
                }
                CheckLevel::Warn => {
                    warn!("Dropping an active transaction. Consider commit or rollback it.")
                }

                CheckLevel::None => {}
            }
        }
        *status = TransactionStatus::Dropped;
    }
}

/// Optimistic or pessimistic transaction.
#[derive(Clone, PartialEq, Debug)]
pub enum TransactionKind {
    Optimistic,
    /// Argument is for_update_ts
    Pessimistic(Timestamp),
}

/// Options for configuring a transaction.
#[derive(Clone, PartialEq, Debug)]
pub struct TransactionOptions {
    /// Optimistic or pessimistic (default) transaction.
    kind: TransactionKind,
    /// Try using 1pc rather than 2pc (default is to always use 2pc).
    try_one_pc: bool,
    /// Try to use async commit (default is not to).
    async_commit: bool,
    /// Is the transaction read only? (Default is no).
    read_only: bool,
    /// How to retry in the event of certain errors.
    retry_options: RetryOptions,
    /// What to do if the transaction is dropped without an attempt to commit or rollback
    check_level: CheckLevel,
    /// Whether heartbeat will be sent automatically
    auto_heartbeat: bool,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum CheckLevel {
    Panic,
    Warn,
    None,
}

impl Default for TransactionOptions {
    fn default() -> TransactionOptions {
        Self::new_pessimistic()
    }
}

impl TransactionOptions {
    /// Default options for an optimistic transaction.
    pub fn new_optimistic() -> TransactionOptions {
        TransactionOptions {
            kind: TransactionKind::Optimistic,
            try_one_pc: false,
            async_commit: false,
            read_only: false,
            retry_options: RetryOptions::default_optimistic(),
            check_level: CheckLevel::Panic,
            auto_heartbeat: true,
        }
    }

    /// Default options for a pessimistic transaction.
    pub fn new_pessimistic() -> TransactionOptions {
        TransactionOptions {
            kind: TransactionKind::Pessimistic(Timestamp::from_version(0)),
            try_one_pc: false,
            async_commit: false,
            read_only: false,
            retry_options: RetryOptions::default_pessimistic(),
            check_level: CheckLevel::Panic,
            auto_heartbeat: true,
        }
    }

    /// Try to use async commit.
    pub fn use_async_commit(mut self) -> TransactionOptions {
        self.async_commit = true;
        self
    }

    /// Try to use 1pc.
    pub fn try_one_pc(mut self) -> TransactionOptions {
        self.try_one_pc = true;
        self
    }

    /// Make the transaction read only.
    pub fn read_only(mut self) -> TransactionOptions {
        self.read_only = true;
        self
    }

    /// Don't automatically resolve locks and retry if keys are locked.
    pub fn no_resolve_locks(mut self) -> TransactionOptions {
        self.retry_options.lock_backoff = Backoff::no_backoff();
        self
    }

    /// Don't automatically resolve regions with PD if we have outdated region information.
    pub fn no_resolve_regions(mut self) -> TransactionOptions {
        self.retry_options.region_backoff = Backoff::no_backoff();
        self
    }

    /// Set RetryOptions.
    pub fn retry_options(mut self, options: RetryOptions) -> TransactionOptions {
        self.retry_options = options;
        self
    }

    /// Set the behavior when dropping a transaction without an attempt to commit or rollback it.
    pub fn drop_check(mut self, level: CheckLevel) -> TransactionOptions {
        self.check_level = level;
        self
    }

    fn push_for_update_ts(&mut self, for_update_ts: Timestamp) {
        match &mut self.kind {
            TransactionKind::Optimistic => unreachable!(),
            TransactionKind::Pessimistic(old_for_update_ts) => {
                self.kind = TransactionKind::Pessimistic(Timestamp::from_version(std::cmp::max(
                    old_for_update_ts.version(),
                    for_update_ts.version(),
                )));
            }
        }
    }

    pub fn no_auto_hearbeat(mut self) -> TransactionOptions {
        self.auto_heartbeat = false;
        self
    }
}

/// The default TTL of a lock in milliseconds
const DEFAULT_LOCK_TTL: u64 = 3000;
/// The default heartbeat interval in milliseconds
const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);

/// A struct wrapping the details of two-phase commit protocol (2PC).
///
/// The two phases are `prewrite` and `commit`.
/// Generally, the `prewrite` phase is to send data to all regions and write them.
/// The `commit` phase is to mark all written data as successfully committed.
///
/// The committer implements `prewrite`, `commit` and `rollback` functions.
#[derive(new)]
struct Committer<PdC: PdClient = PdRpcClient> {
    primary_key: Option<Key>,
    mutations: Vec<kvrpcpb::Mutation>,
    start_version: Timestamp,
    bg_worker: ThreadPool,
    rpc: Arc<PdC>,
    options: TransactionOptions,
    #[new(default)]
    undetermined: bool,
}

impl<PdC: PdClient> Committer<PdC> {
    async fn commit(mut self) -> Result<Option<Timestamp>> {
        let min_commit_ts = self.prewrite().await?;
        if (|| {
            fail_point!("after-prewrite", |_| true);
            false
        })() {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        // If we didn't use 1pc, prewrite will set `try_one_pc` to false.
        if self.options.try_one_pc {
            return Ok(min_commit_ts);
        }

        let commit_ts = if self.options.async_commit {
            min_commit_ts.unwrap()
        } else {
            match self.commit_primary().await {
                Ok(commit_ts) => commit_ts,
                Err(e) => {
                    return if self.undetermined {
                        Err(Error::UndeterminedError(Box::new(e)))
                    } else {
                        Err(e)
                    };
                }
            }
        };
        self.bg_worker
            .clone()
            .spawn_ok(self.commit_secondary(commit_ts.clone()).map(|res| {
                if let Err(e) = res {
                    warn!("Failed to commit secondary keys: {}", e);
                }
            }));
        Ok(Some(commit_ts))
    }

    async fn prewrite(&mut self) -> Result<Option<Timestamp>> {
        let primary_lock = self.primary_key.clone().unwrap();
        // FIXME: calculate TTL for big transactions
        let lock_ttl = DEFAULT_LOCK_TTL;
        let mut request = match &self.options.kind {
            TransactionKind::Optimistic => new_prewrite_request(
                self.mutations.clone(),
                primary_lock,
                self.start_version.clone(),
                lock_ttl,
            ),
            TransactionKind::Pessimistic(for_update_ts) => new_pessimistic_prewrite_request(
                self.mutations.clone(),
                primary_lock,
                self.start_version.clone(),
                lock_ttl,
                for_update_ts.clone(),
            ),
        };

        request.use_async_commit = self.options.async_commit;
        request.try_one_pc = self.options.try_one_pc;
        request.secondaries = self
            .mutations
            .iter()
            .filter(|m| self.primary_key.as_ref().unwrap() != m.key.as_ref())
            .map(|m| m.key.clone())
            .collect();
        // FIXME set max_commit_ts and min_commit_ts

        let plan = PlanBuilder::new(self.rpc.clone(), request)
            .resolve_lock(self.options.retry_options.lock_backoff.clone())
            .multi_region()
            .retry_region(self.options.retry_options.region_backoff.clone())
            .merge(CollectError)
            .plan();
        let response = plan.execute().await?;

        if self.options.try_one_pc && response.len() == 1 {
            if response[0].one_pc_commit_ts == 0 {
                return Err(Error::OnePcFailure);
            }

            return Ok(Timestamp::try_from_version(response[0].one_pc_commit_ts));
        }

        self.options.try_one_pc = false;

        let min_commit_ts = response
            .iter()
            .map(|r| {
                assert_eq!(r.one_pc_commit_ts, 0);
                r.min_commit_ts
            })
            .max()
            .map(|ts| Timestamp::from_version(ts));

        Ok(min_commit_ts)
    }

    /// Commits the primary key and returns the commit version
    async fn commit_primary(&mut self) -> Result<Timestamp> {
        let primary_key = self.primary_key.clone().into_iter();
        let commit_version = self.rpc.clone().get_timestamp().await?;
        let req = new_commit_request(
            primary_key,
            self.start_version.clone(),
            commit_version.clone(),
        );
        let plan = PlanBuilder::new(self.rpc.clone(), req)
            .resolve_lock(self.options.retry_options.lock_backoff.clone())
            .multi_region()
            .retry_region(self.options.retry_options.region_backoff.clone())
            .plan();
        plan.execute()
            .inspect_err(|e| {
                // We don't know whether the transaction is committed or not if we fail to receive
                // the response. Then, we mark the transaction as undetermined and propagate the
                // error to the user.
                if let Error::Grpc(_) = e {
                    self.undetermined = true;
                }
            })
            .await?;

        Ok(commit_version)
    }

    async fn commit_secondary(self, commit_version: Timestamp) -> Result<()> {
        let mutations_len = self.mutations.len();
        let primary_only = mutations_len == 1;
        let mutations = self.mutations.into_iter();

        let req = if self.options.async_commit {
            let keys = mutations.map(|m| m.key.into());
            new_commit_request(keys, self.start_version, commit_version)
        } else if primary_only {
            return Ok(());
        } else {
            let primary_key = self.primary_key.unwrap();
            let keys = mutations
                .map(|m| m.key.into())
                .filter(|key| &primary_key != key);
            new_commit_request(keys, self.start_version, commit_version)
        };
        let plan = PlanBuilder::new(self.rpc, req)
            .resolve_lock(self.options.retry_options.lock_backoff)
            .multi_region()
            .retry_region(self.options.retry_options.region_backoff)
            .plan();
        plan.execute().await?;
        Ok(())
    }

    async fn rollback(self) -> Result<()> {
        if self.options.kind == TransactionKind::Optimistic && self.mutations.is_empty() {
            return Ok(());
        }
        let keys = self
            .mutations
            .into_iter()
            .map(|mutation| mutation.key.into());
        match self.options.kind {
            TransactionKind::Optimistic => {
                let req = new_batch_rollback_request(keys, self.start_version);
                let plan = PlanBuilder::new(self.rpc, req)
                    .resolve_lock(self.options.retry_options.lock_backoff)
                    .multi_region()
                    .retry_region(self.options.retry_options.region_backoff)
                    .plan();
                plan.execute().await?;
            }
            TransactionKind::Pessimistic(for_update_ts) => {
                let req = new_pessimistic_rollback_request(keys, self.start_version, for_update_ts);
                let plan = PlanBuilder::new(self.rpc, req)
                    .resolve_lock(self.options.retry_options.lock_backoff)
                    .multi_region()
                    .retry_region(self.options.retry_options.region_backoff)
                    .plan();
                plan.execute().await?;
            }
        }
        Ok(())
    }
}

#[derive(PartialEq)]
enum TransactionStatus {
    /// The transaction is read-only [`Snapshot`](super::Snapshot), no need to commit or rollback or panic on drop.
    ReadOnly,
    /// The transaction have not been committed or rolled back.
    Active,
    /// The transaction has committed.
    Committed,
    /// The transaction has tried to commit. Only `commit` is allowed.
    StartedCommit,
    /// The transaction has rolled back.
    Rolledback,
    /// The transaction has tried to rollback. Only `rollback` is allowed.
    StartedRollback,
    /// The transaction has been dropped.
    Dropped,
}

#[cfg(test)]
mod tests {
    use crate::{
        mock::{MockKvClient, MockPdClient},
        Transaction, TransactionOptions,
    };
    use fail::FailScenario;
    use futures::executor::ThreadPool;
    use std::{
        any::Any,
        io,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

    #[tokio::test(flavor = "multi_thread", worker_threads = 6)]
    async fn test_optimistic_heartbeat() -> Result<(), io::Error> {
        let scenario = FailScenario::setup();
        fail::cfg("after-prewrite", "return(true)").unwrap();
        let heartbeats = Arc::new(AtomicUsize::new(0));
        let heartbeats_cloned = heartbeats.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if let Some(_heartbeat) = req.downcast_ref::<kvrpcpb::TxnHeartBeatRequest>() {
                    heartbeats_cloned.fetch_add(1, Ordering::SeqCst);
                    return Ok(Box::new(kvrpcpb::TxnHeartBeatResponse::default()) as Box<dyn Any>);
                } else if let Some(_prewrite) = req.downcast_ref::<kvrpcpb::PrewriteRequest>() {
                    return Ok(Box::new(kvrpcpb::PrewriteResponse::default()) as Box<dyn Any>);
                }
                Ok(Box::new(kvrpcpb::CommitResponse::default()) as Box<dyn Any>)
            },
        )));
        let key1 = "key1".to_owned();
        let bg_worker = ThreadPool::new()?;
        let mut heartbeat_txn = Transaction::new(
            Timestamp::default(),
            bg_worker,
            pd_client,
            TransactionOptions::new_optimistic(),
        );
        heartbeat_txn.put(key1.clone(), "foo").await.unwrap();
        let heartbeat_txn_handle = tokio::spawn(async move {
            assert!(heartbeat_txn.commit().await.is_ok());
        });
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        heartbeat_txn_handle.await.unwrap();
        assert!(heartbeats.load(Ordering::SeqCst) >= 1);
        scenario.teardown();
        Ok(())
    }

    #[tokio::test]
    async fn test_pessimistic_heartbeat() -> Result<(), io::Error> {
        let heartbeats = Arc::new(AtomicUsize::new(0));
        let heartbeats_cloned = heartbeats.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if let Some(_heartbeat) = req.downcast_ref::<kvrpcpb::TxnHeartBeatRequest>() {
                    heartbeats_cloned.fetch_add(1, Ordering::SeqCst);
                    return Ok(Box::new(kvrpcpb::TxnHeartBeatResponse::default()) as Box<dyn Any>);
                } else if let Some(_prewrite) = req.downcast_ref::<kvrpcpb::PrewriteRequest>() {
                    return Ok(Box::new(kvrpcpb::PrewriteResponse::default()) as Box<dyn Any>);
                } else if let Some(_pessimistic_lock) =
                    req.downcast_ref::<kvrpcpb::PessimisticLockRequest>()
                {
                    return Ok(
                        Box::new(kvrpcpb::PessimisticLockResponse::default()) as Box<dyn Any>
                    );
                }
                Ok(Box::new(kvrpcpb::CommitResponse::default()) as Box<dyn Any>)
            },
        )));
        let key1 = "key1".to_owned();
        let bg_worker = ThreadPool::new()?;
        let mut heartbeat_txn = Transaction::new(
            Timestamp::default(),
            bg_worker,
            pd_client,
            TransactionOptions::new_pessimistic(),
        );
        heartbeat_txn.put(key1.clone(), "foo").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
        let heartbeat_txn_handle = tokio::spawn(async move {
            assert!(heartbeat_txn.commit().await.is_ok());
        });
        heartbeat_txn_handle.await.unwrap();
        assert!(heartbeats.load(Ordering::SeqCst) > 1);
        Ok(())
    }
}
