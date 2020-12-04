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
use std::{iter, ops::RangeBounds, sync::Arc};
use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

#[derive(PartialEq)]
enum TransactionStatus {
    /// The transaction is read-only [`Snapshot`](super::Snapshot::Snapshot), no need to commit or rollback or panic on drop.
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
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TransactionKind {
    Optimistic,
    /// Argument is for_update_ts
    Pessimistic(u64),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TransactionStyle {
    kind: TransactionKind,
    try_one_pc: bool,
    async_commit: bool,
}

impl TransactionStyle {
    pub fn new_optimistic(try_one_pc: bool) -> TransactionStyle {
        TransactionStyle {
            kind: TransactionKind::Optimistic,
            try_one_pc,
            async_commit: false,
        }
    }

    pub fn new_pessimistic(try_one_pc: bool) -> TransactionStyle {
        TransactionStyle {
            kind: TransactionKind::Pessimistic(0),
            try_one_pc,
            async_commit: false,
        }
    }

    pub fn async_commit(mut self) -> TransactionStyle {
        self.async_commit = true;
        self
    }

    fn push_for_update_ts(&mut self, for_update_ts: u64) {
        match &mut self.kind {
            TransactionKind::Optimistic => {
                self.kind = TransactionKind::Pessimistic(for_update_ts);
            }
            TransactionKind::Pessimistic(old_for_update_ts) => {
                self.kind =
                    TransactionKind::Pessimistic(std::cmp::max(*old_for_update_ts, for_update_ts));
            }
        }
    }
}

/// A undo-able set of actions on the dataset.
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
/// let txn = client.begin().await.unwrap();
/// # });
/// ```
pub struct Transaction {
    status: TransactionStatus,
    timestamp: Timestamp,
    buffer: Buffer,
    bg_worker: ThreadPool,
    rpc: Arc<PdRpcClient>,
    style: TransactionStyle,
}

impl Transaction {
    pub(crate) fn new(
        timestamp: Timestamp,
        bg_worker: ThreadPool,
        rpc: Arc<PdRpcClient>,
        style: TransactionStyle,
        read_only: bool,
    ) -> Transaction {
        let status = if read_only {
            TransactionStatus::ReadOnly
        } else {
            TransactionStatus::Active
        };
        Transaction {
            status,
            timestamp,
            buffer: Default::default(),
            bg_worker,
            rpc,
            style,
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
    /// let mut txn = client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let result: Option<Value> = txn.get(key).await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.check_allow_operation()?;
        let key = key.into();
        self.buffer
            .get_or_else(key, |key| {
                new_mvcc_get_request(key, self.timestamp.clone())
                    .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            })
            .await
    }

    /// Create a `get_for_udpate` request.
    /// Once resolved this request will pessimistically lock and fetch the value associated with the given key.
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
    /// let result: Option<Value> = txn.get_for_update(key).await.unwrap();
    /// // now the key "TiKV" is locked, other transactions cannot modify it
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn get_for_update(&mut self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.check_allow_operation()?;
        if !self.is_pessimistic() {
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
        self.check_allow_operation()?;
        let timestamp = self.timestamp.clone();
        let rpc = self.rpc.clone();
        self.buffer
            .batch_get_or_else(keys.into_iter().map(|k| k.into()), move |keys| {
                new_mvcc_get_batch_request(keys, timestamp).execute(rpc, OPTIMISTIC_BACKOFF)
            })
            .await
    }

    /// Create a new 'batch get' request.
    ///
    /// Once resolved this request will pessimistically lock the keys and
    /// fetch the values associated with the given keys.
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
        self.check_allow_operation()?;
        if !self.is_pessimistic() {
            Err(ErrorKind::InvalidTransactionType.into())
        } else {
            let keys: Vec<Key> = keys.into_iter().map(|it| it.into()).collect();
            self.pessimistic_lock(keys.clone()).await?;
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
    /// let mut txn = client.begin().await.unwrap();
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
    /// let mut txn = client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// txn.put(key, val);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn put(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        self.check_allow_operation()?;
        let key = key.into();
        if self.is_pessimistic() {
            self.pessimistic_lock(iter::once(key.clone())).await?;
        }
        self.buffer.put(key, value.into()).await;
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
    /// let mut txn = client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// txn.delete(key);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn delete(&mut self, key: impl Into<Key>) -> Result<()> {
        self.check_allow_operation()?;
        let key = key.into();
        if self.is_pessimistic() {
            self.pessimistic_lock(iter::once(key.clone())).await?;
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
    /// In pessimistic mode, please use [`get_for_update`](Transaction::get_for_update) instead.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// txn.lock_keys(vec!["TiKV".to_owned(), "Rust".to_owned()]);
    /// // ... Do some actions.
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn lock_keys(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> Result<()> {
        self.check_allow_operation()?;
        for key in keys {
            self.buffer.lock(key.into()).await;
        }
        Ok(())
    }

    /// Commits the actions of the transaction. On success, we return the commit timestamp.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = TransactionClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let mut txn = client.begin().await.unwrap();
    /// // ... Do some actions.
    /// let req = txn.commit();
    /// let result: u64 = req.await.unwrap();
    /// # });
    /// ```
    pub async fn commit(&mut self) -> Result<u64> {
        if !matches!(
            self.status,
            TransactionStatus::StartedCommit | TransactionStatus::Active
        ) {
            return Err(ErrorKind::OperationAfterCommitError.into());
        }
        self.status = TransactionStatus::StartedCommit;

        let res = TwoPhaseCommitter::new(
            self.buffer.to_proto_mutations().await,
            self.timestamp.version(),
            self.bg_worker.clone(),
            self.rpc.clone(),
            self.style,
        )
        .commit()
        .await;

        if res.is_ok() {
            self.status = TransactionStatus::Committed;
        }
        res
    }

    /// Rollback the transaction.
    ///
    /// If it succeeds, all mutations made by this transaciton will not take effect.
    pub async fn rollback(&mut self) -> Result<()> {
        if !matches!(
            self.status,
            TransactionStatus::StartedRollback | TransactionStatus::Active
        ) {
            return Err(ErrorKind::OperationAfterCommitError.into());
        }
        self.status = TransactionStatus::StartedRollback;

        let res = TwoPhaseCommitter::new(
            self.buffer.to_proto_mutations().await,
            self.timestamp.version(),
            self.bg_worker.clone(),
            self.rpc.clone(),
            self.style,
        )
        .rollback()
        .await;

        if res.is_ok() {
            self.status = TransactionStatus::Rolledback;
        }
        res
    }

    async fn scan_inner(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
        key_only: bool,
    ) -> Result<impl Iterator<Item = KvPair>> {
        self.check_allow_operation()?;
        let timestamp = self.timestamp.clone();
        let rpc = self.rpc.clone();

        self.buffer
            .scan_and_fetch(range.into(), limit, move |new_range, new_limit| {
                new_mvcc_scan_request(new_range, timestamp, new_limit, key_only)
                    .execute(rpc, OPTIMISTIC_BACKOFF)
            })
            .await
    }

    /// Pessimistically lock the keys.
    ///
    /// Once resovled it acquires a lock on the key in TiKV.
    /// The lock prevents other transactions from mutating the entry until it is released.
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
        self.style.push_for_update_ts(for_update_ts);
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

    /// Checks if the transaction can perform arbitrary operations.
    fn check_allow_operation(&self) -> Result<()> {
        match self.status {
            TransactionStatus::ReadOnly | TransactionStatus::Active => Ok(()),
            TransactionStatus::Committed
            | TransactionStatus::Rolledback
            | TransactionStatus::StartedCommit
            | TransactionStatus::StartedRollback => {
                Err(ErrorKind::OperationAfterCommitError.into())
            }
        }
    }

    fn is_pessimistic(&self) -> bool {
        matches!(self.style.kind, TransactionKind::Pessimistic(_))
    }

    pub fn use_async_commit(&mut self) {
        self.style = self.style.async_commit();
    }
}

/// The default TTL of a lock in milliseconds
const DEFAULT_LOCK_TTL: u64 = 3000;

/// A struct wrapping the details of two-phase commit protocol (2PC).
///
/// The two phases are `prewrite` and `commit`.
/// Generally, the `prewrite` phase is to send data to all regions and write them.
/// The `commit` phase is to mark all written data as successfully committed.
///
/// The committer implements `prewrite`, `commit` and `rollback` functions.
#[derive(new)]
struct TwoPhaseCommitter {
    mutations: Vec<kvrpcpb::Mutation>,
    start_version: u64,
    bg_worker: ThreadPool,
    rpc: Arc<PdRpcClient>,
    style: TransactionStyle,
    #[new(default)]
    undetermined: bool,
}

impl TwoPhaseCommitter {
    async fn commit(mut self) -> Result<u64> {
        if self.mutations.is_empty() {
            return Ok(0);
        }

        let min_commit_ts = self.prewrite().await?;

        if self.style.try_one_pc {
            return Ok(min_commit_ts);
        }

        let commit_ts = if self.style.async_commit {
            assert_ne!(min_commit_ts, 0);
            min_commit_ts
        } else {
            match self.commit_primary().await {
                Ok(commit_ts) => commit_ts,
                Err(e) => {
                    return if self.undetermined {
                        Err(Error::undetermined_error(e))
                    } else {
                        Err(e)
                    };
                }
            }
        };
        self.bg_worker
            .clone()
            .spawn_ok(self.commit_secondary(commit_ts).map(|res| {
                if let Err(e) = res {
                    warn!("Failed to commit secondary keys: {}", e);
                }
            }));
        Ok(commit_ts)
    }

    async fn prewrite(&mut self) -> Result<u64> {
        let primary_lock = self.mutations[0].key.clone().into();
        // TODO: calculate TTL for big transactions
        let lock_ttl = DEFAULT_LOCK_TTL;
        let mut request = match self.style.kind {
            TransactionKind::Optimistic => new_prewrite_request(
                self.mutations.clone(),
                primary_lock,
                self.start_version,
                lock_ttl,
            ),
            TransactionKind::Pessimistic(for_update_ts) => new_pessimistic_prewrite_request(
                self.mutations.clone(),
                primary_lock,
                self.start_version,
                lock_ttl,
                for_update_ts,
            ),
        };

        request.use_async_commit = self.style.async_commit;
        request.try_one_pc = self.style.try_one_pc;
        request.secondaries = self.mutations[1..].iter().map(|m| m.key.clone()).collect();
        // FIXME set max_commit_ts and min_commit_ts

        let response = match self.style.kind {
            TransactionKind::Optimistic => {
                request
                    .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
                    .await?
            }
            TransactionKind::Pessimistic(_) => {
                request
                    .execute(self.rpc.clone(), PESSIMISTIC_BACKOFF)
                    .await?
            }
        };

        if self.style.try_one_pc {
            if response.one_pc_commit_ts == 0 {
                return Err(ErrorKind::OnePcFailure.into());
            }

            return Ok(response.one_pc_commit_ts);
        }
        assert_eq!(response.one_pc_commit_ts, 0);
        Ok(response.min_commit_ts)
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

    async fn commit_secondary(self, commit_version: u64) -> Result<()> {
        let primary_only = self.mutations.len() == 1;
        let mutations = self.mutations.into_iter();

        // Only skip the primary if we committed it earlier (i.e., we are not using async commit).
        let keys = if self.style.async_commit {
            mutations.skip(0)
        } else {
            if primary_only {
                return Ok(());
            }
            mutations.skip(1)
        }
        .map(|mutation| mutation.key.into())
        .collect();
        new_commit_request(keys, self.start_version, commit_version)
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
    }

    async fn rollback(self) -> Result<()> {
        let keys: Vec<_> = self
            .mutations
            .into_iter()
            .map(|mutation| mutation.key.into())
            .collect();
        match self.style.kind {
            TransactionKind::Optimistic if keys.is_empty() => Ok(()),
            TransactionKind::Optimistic => {
                new_batch_rollback_request(keys, self.start_version)
                    .execute(self.rpc, OPTIMISTIC_BACKOFF)
                    .await
            }
            TransactionKind::Pessimistic(for_update_ts) => {
                new_pessimistic_rollback_request(keys, self.start_version, for_update_ts)
                    .execute(self.rpc, OPTIMISTIC_BACKOFF)
                    .await
            }
        }
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if self.status == TransactionStatus::Active {
            panic!("Dropping an active transaction. Consider commit or rollback it.")
        }
    }
}
