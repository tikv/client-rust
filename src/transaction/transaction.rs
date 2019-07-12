// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{BatchGet, Commit, Delete, Get, LockKeys, Rollback, Scanner, Set, Timestamp};
use crate::{Key, Value};

use derive_new::new;
use std::ops::RangeBounds;

/// A undo-able set of actions on the dataset.
///
/// Using a transaction you can prepare a set of actions (such as `get`, or `set`) on data at a
/// particular timestamp obtained from the placement driver.
///
/// Once a transaction is commited, a new commit timestamp is obtained from the placement driver.
///
/// Create a new transaction from a snapshot using `new`.
///
/// ```rust,no_run
/// # #![feature(async_await)]
/// use tikv_client::{Config, transaction::Client};
/// use futures::prelude::*;
/// # futures::executor::block_on(async {
/// let connect = Client::connect(Config::default());
/// let client = connect.await.unwrap();
/// let txn = client.begin();
/// # });
/// ```
#[derive(new)]
pub struct Transaction {
    snapshot: Snapshot,
}

impl Transaction {
    /// Commit the actions of the transaction.
    ///
    /// Once committed, it is no longer possible to `rollback` the actions in the transaction.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connect = Client::connect(Config::default());
    /// # let connected_client = connect.await.unwrap();
    /// let txn = connected_client.begin();
    /// // ... Do some actions.
    /// let req = txn.commit();
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub fn commit(self) -> Commit {
        Commit::new(self)
    }

    /// Rollback the actions of the transaction.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connect = Client::connect(Config::default());
    /// # let connected_client = connect.await.unwrap();
    /// let txn = connected_client.begin();
    /// // ... Do some actions.
    /// let req = txn.rollback();
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub fn rollback(self) -> Rollback {
        Rollback::new(self)
    }

    /// Lock the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connect = Client::connect(Config::default());
    /// # let connected_client = connect.await.unwrap();
    /// let mut txn = connected_client.begin();
    /// // ... Do some actions.
    /// let req = txn.lock_keys(vec!["TiKV".to_owned(), "Rust".to_owned()]);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub fn lock_keys(&mut self, keys: impl IntoIterator<Item = impl Into<Key>>) -> LockKeys {
        LockKeys::new(keys.into_iter().map(|v| v.into()).collect())
    }

    pub fn is_readonly(&self) -> bool {
        unimplemented!()
    }

    /// Returns the timestamp which the transaction started at.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, transaction::{Client, Timestamp}};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connect = Client::connect(Config::default());
    /// # let connected_client = connect.await.unwrap();
    /// let txn = connected_client.begin();
    /// // ... Do some actions.
    /// let ts: Timestamp = txn.start_ts();
    /// # });
    /// ```
    pub fn start_ts(&self) -> Timestamp {
        unimplemented!()
    }

    /// Get the `Snapshot` the transaction is operating on.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, transaction::{Client, Snapshot}};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connect = Client::connect(Config::default());
    /// # let connected_client = connect.await.unwrap();
    /// let txn = connected_client.begin();
    /// // ... Do some actions.
    /// let snap: Snapshot = txn.snapshot();
    /// # });
    /// ```
    pub fn snapshot(&self) -> Snapshot {
        unimplemented!()
    }

    /// Set the isolation level of the transaction.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, transaction::{Client, IsolationLevel}};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connect = Client::connect(Config::default());
    /// # let connected_client = connect.await.unwrap();
    /// let mut txn = connected_client.begin();
    /// txn.set_isolation_level(IsolationLevel::SnapshotIsolation);
    /// # });
    /// ```
    pub fn set_isolation_level(&mut self, _level: IsolationLevel) {
        unimplemented!()
    }

    /// Create a new [`Get`](Get) request.
    ///
    /// Once resolved this request will result in the fetching of the value associated with the
    /// given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Value, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin();
    /// let key = "TiKV".to_owned();
    /// let req = txn.get(key);
    /// let result: Value = req.await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub fn get(&self, key: impl Into<Key>) -> Get {
        self.snapshot.get(key.into())
    }

    /// Create a new [`BatchGet`](BatchGet) request.
    ///
    /// Once resolved this request will result in the fetching of the values associated with the
    /// given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{KvPair, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = txn.batch_get(keys);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub fn batch_get(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> BatchGet {
        self.snapshot.batch_get(keys)
    }

    pub fn scan(&self, range: impl RangeBounds<Key>) -> Scanner {
        self.snapshot.scan(range)
    }

    pub fn scan_reverse(&self, range: impl RangeBounds<Key>) -> Scanner {
        self.snapshot.scan_reverse(range)
    }

    /// Create a new [`Set`](Set) request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Value, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// let req = txn.set(key, val);
    /// let result: () = req.await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub fn set(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Set {
        Set::new(key.into(), value.into())
    }

    /// Create a new [`Delete`](Delete) request.
    ///
    /// Once resolved this request will result in the deletion of the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin();
    /// let key = "TiKV".to_owned();
    /// let req = txn.delete(key);
    /// let result: () = req.await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub fn delete(&mut self, key: impl Into<Key>) -> Delete {
        Delete::new(key.into())
    }
}

pub struct TxnInfo {
    pub txn: u64,
    pub status: u64,
}

/// A snapshot of dataset at a particular point in time.
pub struct Snapshot;

impl Snapshot {
    pub fn get(&self, key: impl Into<Key>) -> Get {
        Get::new(key.into())
    }

    pub fn batch_get(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> BatchGet {
        BatchGet::new(keys.into_iter().map(|v| v.into()).collect())
    }

    pub fn scan(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
        unimplemented!()
    }

    pub fn scan_reverse(&self, range: impl RangeBounds<Key>) -> Scanner {
        drop(range);
        unimplemented!()
    }
}

/// The isolation level guarantees provided by the transaction.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum IsolationLevel {
    /// Consistent reads and conflict free writes.
    ///
    /// Snapshot isolation guarantees:
    /// * All reads will see the last committed value of the data at the snapshot timestamp.
    /// * The transaction will only successfully commit if no updates to the data have created a
    ///   conflict with concurrent updates made sine the snapshot.
    ///
    /// Using this level means:
    /// * Lost updates don't occur.
    /// * Dirty reads don't occur.
    /// * Non-repeatable reads don't occur.
    /// * Phantom reads don't occur.
    SnapshotIsolation,
    /// Reads may not be consistent, but writes are conflict free.
    ///
    /// Read committed guarantees:
    /// * All reads are committed at the moment it is read.
    /// not repeatable.
    /// * Write locks are only released at the end of the transaction.
    ///
    /// Using this level means:
    /// * Lost updates don't occur.
    /// * Dirty reads don't occur.
    /// * Non-repeatable reads may occur.
    /// * Phantom reads may occur.
    ReadCommitted,
}
