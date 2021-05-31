// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    backoff::Backoff,
    pd::{PdClient, PdRpcClient},
    request::{Collect, CollectAndMatchKey, CollectError, Plan, PlanBuilder, RetryOptions},
    timestamp::TimestampExt,
    transaction::{buffer::Buffer, lowering::*},
    BoundRange, Error, Key, KvPair, Result, Value,
};
use derive_new::new;
use fail::fail_point;
use futures::{prelude::*, stream::BoxStream};
use log::debug;
use std::{iter, ops::RangeBounds, sync::Arc, time::Instant};
use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};
use tokio::{sync::RwLock, time::Duration};

/// An undo-able set of actions on the dataset.
///
/// Create a transaction using a [`TransactionClient`](crate::TransactionClient), then run actions
/// (such as `get`, or `put`) on the transaction. Reads are executed immediately, writes are
/// buffered locally. Once complete, `commit` the transaction. Behind the scenes, the client will
/// perform a two phase commit and return success as soon as the writes are guaranteed to be
/// committed (some finalisation may continue in the background after the return, but no data can be
/// lost).
///
/// TiKV transactions use multi-version concurrency control. All reads logically happen at the start
/// of the transaction (at the start timestamp, `start_ts`). Once a transaction is commited, a
/// its writes atomically become visible to other transactions at (logically) the commit timestamp.
///
/// In other words, a transaction can read data that was committed at `commit_ts` < its `start_ts`,
/// and its writes are readable by transactions with `start_ts` >= its `commit_ts`.
///
/// Mutations are buffered locally and sent to the TiKV cluster at the time of commit.
/// In a pessimistic transaction, all write operations and `xxx_for_update` operations will immediately
/// acquire locks from TiKV. Such a lock blocks other transactions from writing to that key.
/// A lock exists until the transaction is committed or rolled back, or the lock reaches its time to
/// live (TTL).
///
/// For details, the [SIG-Transaction](https://github.com/tikv/sig-transaction)
/// provides materials explaining designs and implementations of TiKV transactions.
///
/// # Examples
///
/// ```rust,no_run
/// # use tikv_client::{Config, TransactionClient};
/// # use futures::prelude::*;
/// # futures::executor::block_on(async {
/// let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
/// let mut txn = client.begin_optimistic().await.unwrap();
/// let foo = txn.get("foo".to_owned()).await.unwrap().unwrap();
/// txn.put("bar".to_owned(), foo).await.unwrap();
/// txn.commit().await.unwrap();
/// # });
/// ```
pub struct Transaction<PdC: PdClient = PdRpcClient> {
    status: Arc<RwLock<TransactionStatus>>,
    timestamp: Timestamp,
    buffer: Buffer,
    rpc: Arc<PdC>,
    options: TransactionOptions,
    is_heartbeat_started: bool,
    start_instant: Instant,
}

impl<PdC: PdClient> Transaction<PdC> {
    pub(crate) fn new(
        timestamp: Timestamp,
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
            buffer: Buffer::new(options.is_pessimistic()),
            rpc,
            options,
            is_heartbeat_started: false,
            start_instant: std::time::Instant::now(),
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
    /// # });
    /// ```
    pub async fn get(&mut self, key: impl Into<Key>) -> Result<Option<Value>> {
        debug!("invoking transactional get request");
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

    /// Create a `get for update` request.
    ///
    /// The request reads and "locks" a key. It is similar to `SELECT ... FOR
    /// UPDATE` in TiDB, and has different behavior in optimistic and
    /// pessimistic transactions.
    ///
    /// # Optimistic transaction
    ///
    /// It reads at the "start timestamp" and caches the value, just like normal
    /// get requests. The lock is written in prewrite and commit, so it cannot
    /// prevent concurrent transactions from writing the same key, but can only
    /// prevent itself from committing.
    ///
    /// # Pessimistic transaction
    ///
    /// It reads at the "current timestamp" and thus does not cache the value.
    /// So following read requests won't be affected by the `get_for_udpate`.
    /// A lock will be acquired immediately with this request, which prevents
    /// concurrent transactions from mutating the keys.
    ///
    /// The "current timestamp" (also called `for_update_ts` of the request) is fetched from PD.
    ///
    /// Note: The behavior of this command under pessimistic transaction does not follow snapshot.
    /// It reads the latest value (using current timestamp), and the value is not cached in the
    /// local buffer. So normal `get`-like commands after `get_for_update` will not be influenced,
    /// they still read values at the transaction's `start_ts`.
    ///
    /// # Examples
    ///
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
        debug!("invoking transactional get_for_update request");
        self.check_allow_operation().await?;
        if !self.is_pessimistic() {
            let key = key.into();
            self.lock_keys(iter::once(key.clone())).await?;
            self.get(key).await
        } else {
            let mut pairs = self.pessimistic_lock(iter::once(key.into()), true).await?;
            debug_assert!(pairs.len() <= 1);
            match pairs.pop() {
                Some(pair) => Ok(Some(pair.1)),
                None => Ok(None),
            }
        }
    }

    /// Check whether a key exists.
    ///
    /// # Examples
    ///
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
    pub async fn key_exists(&mut self, key: impl Into<Key>) -> Result<bool> {
        debug!("invoking transactional key_exists request");
        let key = key.into();
        Ok(self.scan_keys(key.clone()..=key, 1).await?.next().is_some())
    }

    /// Create a new 'batch get' request.
    ///
    /// Once resolved this request will result in the fetching of the values associated with the
    /// given keys.
    ///
    /// Non-existent entries will not appear in the result. The order of the keys is not retained in
    /// the result.
    ///
    /// # Examples
    ///
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
        &mut self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<impl Iterator<Item = KvPair>> {
        debug!("invoking transactional batch_get request");
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
    /// Similar to [`get_for_update`](Transaction::get_for_update), but it works
    /// for a batch of keys.
    ///
    /// Non-existent entries will not appear in the result. The order of the
    /// keys is not retained in the result.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, TransactionClient, KvPair};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_pessimistic().await.unwrap();
    /// let keys = vec!["foo".to_owned(), "bar".to_owned()];
    /// let result: Vec<KvPair> = txn
    ///     .batch_get_for_update(keys)
    ///     .await
    ///     .unwrap();
    /// // now "foo" and "bar" are both locked
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn batch_get_for_update(
        &mut self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<Vec<KvPair>> {
        debug!("invoking transactional batch_get_for_update request");
        self.check_allow_operation().await?;
        let keys: Vec<Key> = keys.into_iter().map(|k| k.into()).collect();
        if !self.is_pessimistic() {
            self.lock_keys(keys.clone()).await?;
            Ok(self.batch_get(keys).await?.collect())
        } else {
            self.pessimistic_lock(keys, true).await
        }
    }

    /// Create a new 'scan' request.
    ///
    /// Once resolved this request will result in a `Vec` of all key-value pairs that lie in the
    /// specified range.
    ///
    /// If the number of eligible key-value pairs are greater than `limit`,
    /// only the first `limit` pairs are returned, ordered by key.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, KvPair, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key1: Key = b"foo".to_vec().into();
    /// let key2: Key = b"bar".to_vec().into();
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
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = KvPair>> {
        debug!("invoking transactional scan request");
        self.scan_inner(range, limit, false).await
    }

    /// Create a new 'scan' request that only returns the keys.
    ///
    /// Once resolved this request will result in a `Vec` of keys that lies in the specified range.
    ///
    /// If the number of eligible keys are greater than `limit`,
    /// only the first `limit` keys are returned, ordered by key.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, KvPair, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key1: Key = b"foo".to_vec().into();
    /// let key2: Key = b"bar".to_vec().into();
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
        &mut self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<impl Iterator<Item = Key>> {
        debug!("invoking transactional scan_keys request");
        Ok(self
            .scan_inner(range, limit, true)
            .await?
            .map(KvPair::into_key))
    }

    /// Create a 'scan_reverse' request.
    ///
    /// Similar to [`scan`](Transaction::scan), but scans in the reverse direction.
    pub(crate) fn scan_reverse(&self, _range: impl RangeBounds<Key>) -> BoxStream<Result<KvPair>> {
        debug!("invoking transactional scan_reverse request");
        unimplemented!()
    }

    /// Sets the value associated with the given key.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key = "foo".to_owned();
    /// let val = "FOO".to_owned();
    /// txn.put(key, val);
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn put(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        debug!("invoking transactional put request");
        self.check_allow_operation().await?;
        let key = key.into();
        if self.is_pessimistic() {
            self.pessimistic_lock(iter::once(key.clone()), false)
                .await?;
        }
        self.buffer.put(key, value.into());
        Ok(())
    }

    /// Inserts the value associated with the given key.
    ///
    /// Similar to [`put'], but it has an additional constraint that the key should not exist
    /// before this operation.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key = "foo".to_owned();
    /// let val = "FOO".to_owned();
    /// txn.insert(key, val);
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn insert(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        debug!("invoking transactional insert request");
        self.check_allow_operation().await?;
        let key = key.into();
        if self.buffer.get(&key).is_some() {
            return Err(Error::DuplicateKeyInsertion);
        }
        if self.is_pessimistic() {
            self.pessimistic_lock(
                iter::once((key.clone(), kvrpcpb::Assertion::NotExist)),
                false,
            )
            .await?;
        }
        self.buffer.insert(key, value.into());
        Ok(())
    }

    /// Deletes the given key and its value from the database.
    ///
    /// Deleting a non-existent key will not result in an error.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// let key = "foo".to_owned();
    /// txn.delete(key);
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn delete(&mut self, key: impl Into<Key>) -> Result<()> {
        debug!("invoking transactional delete request");
        self.check_allow_operation().await?;
        let key = key.into();
        if self.is_pessimistic() {
            self.pessimistic_lock(iter::once(key.clone()), false)
                .await?;
        }
        self.buffer.delete(key);
        Ok(())
    }

    /// Lock the given keys without mutating their values.
    ///
    /// In optimistic mode, write conflicts are not checked until commit.
    /// So use this command to indicate that
    /// "I do not want to commit if the value associated with this key has been modified".
    /// It's useful to avoid the *write skew* anomaly.
    ///
    /// In pessimistic mode, it is similar to [`batch_get_for_update`](Transaction::batch_get_for_update),
    /// except that it does not read values.
    ///
    /// # Examples
    ///
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
        debug!("invoking transactional lock_keys request");
        self.check_allow_operation().await?;
        match self.options.kind {
            TransactionKind::Optimistic => {
                for key in keys {
                    self.buffer.lock(key.into());
                }
            }
            TransactionKind::Pessimistic(_) => {
                let keys: Vec<Key> = keys.into_iter().map(|k| k.into()).collect();
                self.pessimistic_lock(keys.into_iter(), false).await?;
            }
        }
        Ok(())
    }

    /// Commits the actions of the transaction. On success, we return the commit timestamp (or
    /// `None` if there was nothing to commit).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, Timestamp, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// // ... Do some actions.
    /// let result: Timestamp = txn.commit().await.unwrap().unwrap();
    /// # });
    /// ```
    pub async fn commit(&mut self) -> Result<Option<Timestamp>> {
        debug!("commiting transaction");
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

        let primary_key = self.buffer.get_primary_key();
        let mutations = self.buffer.to_proto_mutations();
        if mutations.is_empty() {
            assert!(primary_key.is_none());
            return Ok(None);
        }

        self.start_auto_heartbeat().await;

        let res = Committer::new(
            primary_key,
            mutations,
            self.timestamp.clone(),
            self.rpc.clone(),
            self.options.clone(),
            self.buffer.get_write_size() as u64,
            self.start_instant,
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
    /// If it succeeds, all mutations made by this transaction will be discarded.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, Timestamp, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut txn = client.begin_optimistic().await.unwrap();
    /// // ... Do some actions.
    /// txn.rollback().await.unwrap();
    /// # });
    /// ```
    pub async fn rollback(&mut self) -> Result<()> {
        debug!("rolling back transaction");
        {
            let status = self.status.read().await;
            if !matches!(
                *status,
                TransactionStatus::StartedRollback
                    | TransactionStatus::Active
                    | TransactionStatus::StartedCommit
            ) {
                return Err(Error::OperationAfterCommitError);
            }
        }

        {
            let mut status = self.status.write().await;
            *status = TransactionStatus::StartedRollback;
        }

        let primary_key = self.buffer.get_primary_key();
        let mutations = self.buffer.to_proto_mutations();
        let res = Committer::new(
            primary_key,
            mutations,
            self.timestamp.clone(),
            self.rpc.clone(),
            self.options.clone(),
            self.buffer.get_write_size() as u64,
            self.start_instant,
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
    /// Returns the TTL set on the transaction's locks by TiKV.
    #[doc(hidden)]
    pub async fn send_heart_beat(&mut self) -> Result<u64> {
        debug!("sending heart_beat");
        self.check_allow_operation().await?;
        let primary_key = match self.buffer.get_primary_key() {
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
        &mut self,
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

    /// Pessimistically lock the keys, and optionally retrieve corresponding values.
    /// If a key does not exist, the corresponding pair will not appear in the result.
    ///
    /// Once resolved it acquires locks on the keys in TiKV.
    /// A lock prevents other transactions from mutating the entry until it is released.
    ///
    /// # Panics
    ///
    /// Only valid for pessimistic transactions, panics if called on an optimistic transaction.
    async fn pessimistic_lock(
        &mut self,
        keys: impl IntoIterator<Item = impl PessimisticLock>,
        need_value: bool,
    ) -> Result<Vec<KvPair>> {
        debug!("acquiring pessimistic lock");
        assert!(
            matches!(self.options.kind, TransactionKind::Pessimistic(_)),
            "`pessimistic_lock` is only valid to use with pessimistic transactions"
        );

        let keys: Vec<_> = keys.into_iter().collect();
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let first_key = keys[0].clone().key();
        // we do not set the primary key here, because pessimistic lock request
        // can fail, in which case the keys may not be part of the transaction.
        let primary_lock = self
            .buffer
            .get_primary_key()
            .unwrap_or_else(|| first_key.clone());
        let for_update_ts = self.rpc.clone().get_timestamp().await?;
        self.options.push_for_update_ts(for_update_ts.clone());
        let request = new_pessimistic_lock_request(
            keys.clone().into_iter(),
            primary_lock,
            self.timestamp.clone(),
            DEFAULT_LOCK_TTL,
            for_update_ts,
            need_value,
        );
        let plan = PlanBuilder::new(self.rpc.clone(), request)
            .resolve_lock(self.options.retry_options.lock_backoff.clone())
            .preserve_keys()
            .multi_region()
            .retry_region(self.options.retry_options.region_backoff.clone())
            .merge(CollectAndMatchKey)
            .plan();
        let pairs = plan.execute().await;

        // primary key will be set here if needed
        self.buffer.primary_key_or(&first_key);

        self.start_auto_heartbeat().await;

        for key in keys {
            self.buffer.lock(key.key());
        }

        pairs
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
        debug!("starting auto_heartbeat");
        if !self.options.heartbeat_option.is_auto_heartbeat() || self.is_heartbeat_started {
            return;
        }
        self.is_heartbeat_started = true;

        let status = self.status.clone();
        let primary_key = self
            .buffer
            .get_primary_key()
            .expect("Primary key should exist");
        let start_ts = self.timestamp.clone();
        let region_backoff = self.options.retry_options.region_backoff.clone();
        let rpc = self.rpc.clone();
        let heartbeat_interval = match self.options.heartbeat_option {
            HeartbeatOption::NoHeartbeat => DEFAULT_HEARTBEAT_INTERVAL,
            HeartbeatOption::FixedTime(heartbeat_interval) => heartbeat_interval,
        };

        let heartbeat_task = async move {
            loop {
                tokio::time::sleep(heartbeat_interval).await;
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
        debug!("dropping transaction");
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

/// The default max TTL of a lock in milliseconds. Also called `ManagedLockTTL` in TiDB.
const MAX_TTL: u64 = 20000;
/// The default TTL of a lock in milliseconds.
const DEFAULT_LOCK_TTL: u64 = 3000;
/// The default heartbeat interval
const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(MAX_TTL / 2);
/// TiKV recommends each RPC packet should be less than around 1MB. We keep KV size of
/// each request below 16KB.
const TXN_COMMIT_BATCH_SIZE: u64 = 16 * 1024;
const TTL_FACTOR: f64 = 6000.0;

/// Optimistic or pessimistic transaction.
#[derive(Clone, PartialEq, Debug)]
pub enum TransactionKind {
    Optimistic,
    /// Argument is the transaction's for_update_ts
    Pessimistic(Timestamp),
}

/// Options for configuring a transaction.
///
/// `TransactionOptions` has a builder-style API.
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
    #[doc(hidden)]
    heartbeat_option: HeartbeatOption,
}

#[derive(Clone, PartialEq, Debug)]
pub enum HeartbeatOption {
    NoHeartbeat,
    FixedTime(Duration),
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
            heartbeat_option: HeartbeatOption::FixedTime(DEFAULT_HEARTBEAT_INTERVAL),
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
            heartbeat_option: HeartbeatOption::FixedTime(DEFAULT_HEARTBEAT_INTERVAL),
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

    pub fn heartbeat_option(mut self, heartbeat_option: HeartbeatOption) -> TransactionOptions {
        self.heartbeat_option = heartbeat_option;
        self
    }

    // Returns true if these options describe a pessimistic transaction.
    pub fn is_pessimistic(&self) -> bool {
        match self.kind {
            TransactionKind::Pessimistic(_) => true,
            TransactionKind::Optimistic => false,
        }
    }
}

/// Determines what happens when a transaction is dropped without being rolled back or committed.
///
/// The default is to panic.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum CheckLevel {
    /// The program will panic.
    ///
    /// Note that if the thread is already panicking, then we will not double-panic and abort, but
    /// just ignore the issue.
    Panic,
    /// Log a warning.
    Warn,
    /// Do nothing
    None,
}

impl HeartbeatOption {
    pub fn is_auto_heartbeat(&self) -> bool {
        !matches!(self, HeartbeatOption::NoHeartbeat)
    }
}

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
    rpc: Arc<PdC>,
    options: TransactionOptions,
    #[new(default)]
    undetermined: bool,
    write_size: u64,
    start_instant: Instant,
}

impl<PdC: PdClient> Committer<PdC> {
    async fn commit(mut self) -> Result<Option<Timestamp>> {
        let min_commit_ts = self.prewrite().await?;

        fail_point!("after-prewrite");

        // If we didn't use 1pc, prewrite will set `try_one_pc` to false.
        if self.options.try_one_pc {
            return Ok(min_commit_ts);
        }

        let commit_ts = if self.options.async_commit {
            // FIXME: min_commit_ts == 0 => fallback to normal 2PC
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
        tokio::spawn(self.commit_secondary(commit_ts.clone()).map(|res| {
            if let Err(e) = res {
                warn!("Failed to commit secondary keys: {}", e);
            }
        }));
        Ok(Some(commit_ts))
    }

    async fn prewrite(&mut self) -> Result<Option<Timestamp>> {
        let primary_lock = self.primary_key.clone().unwrap();
        let elapsed = self.start_instant.elapsed().as_millis() as u64;
        let lock_ttl = self.calc_txn_lock_ttl();
        let mut request = match &self.options.kind {
            TransactionKind::Optimistic => new_prewrite_request(
                self.mutations.clone(),
                primary_lock,
                self.start_version.clone(),
                lock_ttl + elapsed,
            ),
            TransactionKind::Pessimistic(for_update_ts) => new_pessimistic_prewrite_request(
                self.mutations.clone(),
                primary_lock,
                self.start_version.clone(),
                lock_ttl + elapsed,
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
            .extract_error()
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
            .map(Timestamp::from_version);

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
            .extract_error()
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
            .extract_error()
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
                    .extract_error()
                    .plan();
                plan.execute().await?;
            }
            TransactionKind::Pessimistic(for_update_ts) => {
                let req = new_pessimistic_rollback_request(keys, self.start_version, for_update_ts);
                let plan = PlanBuilder::new(self.rpc, req)
                    .resolve_lock(self.options.retry_options.lock_backoff)
                    .multi_region()
                    .retry_region(self.options.retry_options.region_backoff)
                    .extract_error()
                    .plan();
                plan.execute().await?;
            }
        }
        Ok(())
    }

    fn calc_txn_lock_ttl(&mut self) -> u64 {
        let mut lock_ttl = DEFAULT_LOCK_TTL;
        if self.write_size > TXN_COMMIT_BATCH_SIZE {
            let size_mb = self.write_size as f64 / 1024.0 / 1024.0;
            lock_ttl = (TTL_FACTOR * size_mb.sqrt()) as u64;
            lock_ttl = lock_ttl.min(MAX_TTL).max(DEFAULT_LOCK_TTL);
        }
        lock_ttl
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
        transaction::HeartbeatOption,
        Transaction, TransactionOptions,
    };
    use fail::FailScenario;
    use std::{
        any::Any,
        io,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };
    use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

    #[tokio::test]
    async fn test_optimistic_heartbeat() -> Result<(), io::Error> {
        let scenario = FailScenario::setup();
        fail::cfg("after-prewrite", "sleep(1500)").unwrap();
        let heartbeats = Arc::new(AtomicUsize::new(0));
        let heartbeats_cloned = heartbeats.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if let Some(_) = req.downcast_ref::<kvrpcpb::TxnHeartBeatRequest>() {
                    heartbeats_cloned.fetch_add(1, Ordering::SeqCst);
                    Ok(Box::new(kvrpcpb::TxnHeartBeatResponse::default()) as Box<dyn Any>)
                } else if let Some(_) = req.downcast_ref::<kvrpcpb::PrewriteRequest>() {
                    Ok(Box::new(kvrpcpb::PrewriteResponse::default()) as Box<dyn Any>)
                } else {
                    Ok(Box::new(kvrpcpb::CommitResponse::default()) as Box<dyn Any>)
                }
            },
        )));
        let key1 = "key1".to_owned();
        let mut heartbeat_txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic()
                .heartbeat_option(HeartbeatOption::FixedTime(Duration::from_secs(1))),
        );
        heartbeat_txn.put(key1.clone(), "foo").await.unwrap();
        let heartbeat_txn_handle = tokio::task::spawn_blocking(move || {
            assert!(futures::executor::block_on(heartbeat_txn.commit()).is_ok())
        });
        assert_eq!(heartbeats.load(Ordering::SeqCst), 0);
        heartbeat_txn_handle.await.unwrap();
        assert_eq!(heartbeats.load(Ordering::SeqCst), 1);
        scenario.teardown();
        Ok(())
    }

    #[tokio::test]
    async fn test_pessimistic_heartbeat() -> Result<(), io::Error> {
        let heartbeats = Arc::new(AtomicUsize::new(0));
        let heartbeats_cloned = heartbeats.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if let Some(_) = req.downcast_ref::<kvrpcpb::TxnHeartBeatRequest>() {
                    heartbeats_cloned.fetch_add(1, Ordering::SeqCst);
                    Ok(Box::new(kvrpcpb::TxnHeartBeatResponse::default()) as Box<dyn Any>)
                } else if let Some(_) = req.downcast_ref::<kvrpcpb::PrewriteRequest>() {
                    Ok(Box::new(kvrpcpb::PrewriteResponse::default()) as Box<dyn Any>)
                } else if let Some(_) = req.downcast_ref::<kvrpcpb::PessimisticLockRequest>() {
                    Ok(Box::new(kvrpcpb::PessimisticLockResponse::default()) as Box<dyn Any>)
                } else {
                    Ok(Box::new(kvrpcpb::CommitResponse::default()) as Box<dyn Any>)
                }
            },
        )));
        let key1 = "key1".to_owned();
        let mut heartbeat_txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_pessimistic()
                .heartbeat_option(HeartbeatOption::FixedTime(Duration::from_secs(1))),
        );
        heartbeat_txn.put(key1.clone(), "foo").await.unwrap();
        assert_eq!(heartbeats.load(Ordering::SeqCst), 0);
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
        assert_eq!(heartbeats.load(Ordering::SeqCst), 1);
        let heartbeat_txn_handle = tokio::spawn(async move {
            assert!(heartbeat_txn.commit().await.is_ok());
        });
        heartbeat_txn_handle.await.unwrap();
        Ok(())
    }
}
