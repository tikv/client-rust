// Copyright 2018 The TiKV Project Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

/*! Transactional related functionality.

Using the [`transaction::Client`](struct.Client.html) you can utilize TiKV's transactional interface.

This interface offers SQL-like transactions on top of the raw interface.

**Warning:** It is not advisible to use the both raw and transactional functionality in the same keyspace.
 */
use crate::{Config, Error, Key, KvPair, Value};
use futures::{Future, Poll, Stream};
use std::ops::RangeBounds;

/// A logical timestamp produced by PD.
#[derive(Copy, Clone)]
pub struct Timestamp(u64);

impl From<u64> for Timestamp {
    fn from(v: u64) -> Self {
        Timestamp(v)
    }
}

impl Timestamp {
    pub fn timestamp(self) -> u64 {
        self.0
    }

    pub fn physical(self) -> i64 {
        (self.0 >> 16) as i64
    }

    pub fn logical(self) -> i64 {
        (self.0 & 0xFFFF as u64) as i64
    }
}

pub struct Scanner;

impl Stream for Scanner {
    type Item = KvPair;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
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

pub struct Get {
    key: Key,
}

impl Get {
    fn new(key: Key) -> Self {
        Get { key }
    }
}

impl Future for Get {
    type Item = Value;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _key = &self.key;
        unimplemented!()
    }
}

pub struct BatchGet {
    keys: Vec<Key>,
}

impl BatchGet {
    fn new(keys: Vec<Key>) -> Self {
        BatchGet { keys }
    }
}

impl Future for BatchGet {
    type Item = Value;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _keys = &self.keys;
        unimplemented!()
    }
}

pub struct Commit {
    txn: Transaction,
}

impl Commit {
    fn new(txn: Transaction) -> Self {
        Commit { txn }
    }
}

impl Future for Commit {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _txn = &self.txn;
        unimplemented!()
    }
}

pub struct Rollback {
    txn: Transaction,
}

impl Rollback {
    fn new(txn: Transaction) -> Self {
        Rollback { txn }
    }
}

impl Future for Rollback {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _txn = &self.txn;
        unimplemented!()
    }
}

pub struct LockKeys {
    keys: Vec<Key>,
}

impl LockKeys {
    fn new(keys: Vec<Key>) -> Self {
        LockKeys { keys }
    }
}

impl Future for LockKeys {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _keys = &self.keys;
        unimplemented!()
    }
}

pub struct Set {
    key: Key,
    value: Value,
}

impl Set {
    fn new(key: Key, value: Value) -> Self {
        Set { key, value }
    }
}

impl Future for Set {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _key = &self.key;
        let _value = &self.value;
        unimplemented!()
    }
}

pub struct Delete {
    key: Key,
}

impl Delete {
    fn new(key: Key) -> Self {
        Delete { key }
    }
}

impl Future for Delete {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _key = &self.key;
        unimplemented!()
    }
}

/// A undo-able set of actions on the dataset.
/// 
/// Using a transaction you can prepare a set of actions (such as `get`, or `set`) on data at a 
/// particular timestamp obtained from the placement driver.
/// 
/// Once a transaction is commited, a new commit timestamp is obtained from the placement driver.
pub struct Transaction {
    snapshot: Snapshot,
}

impl Transaction {
    /// Create a new transaction operating on the given snapshot.
    pub fn new(snapshot: Snapshot) -> Self {
        Self {
            snapshot
        }
    }

    /// Commit the actions of the transaction.
    /// 
    /// Once committed, it is no longer possible to `rollback` the actions in the transaction.
    pub fn commit(self) -> Commit {
        Commit::new(self)
    }

    /// Rollback the actions of the transaction.
    pub fn rollback(self) -> Rollback {
        Rollback::new(self)
    }

    pub fn lock_keys(&mut self, keys: impl AsRef<[Key]>) -> LockKeys {
        LockKeys::new(keys.as_ref().to_vec().clone())
    }

    pub fn is_readonly(&self) -> bool {
        unimplemented!()
    }

    /// Returns the timestamp which the transaction started at.
    /// 
    /// 
    pub fn start_ts(&self) -> Timestamp {
        unimplemented!()
    }

    /// Get the `Snapshot` the transaction is operating on.
    pub fn snapshot(&self) -> Snapshot {
        unimplemented!()
    }

    /// Set the isolation level of the transaction.
    pub fn set_isolation_level(&mut self, _level: IsolationLevel) {
        unimplemented!()
    }

    pub fn get(&self, key: impl AsRef<Key>) -> Get {
        self.snapshot.get(key)
    }

    pub fn batch_get(&self, keys: impl AsRef<[Key]>) -> BatchGet {
        self.snapshot.batch_get(keys)
    }

    pub fn scan(&self, range: impl RangeBounds<Key>) -> Scanner {
        self.snapshot.scan(range)
    }

    pub fn scan_reverse(&self, range: impl RangeBounds<Key>) -> Scanner {
        self.snapshot.scan_reverse(range)
    }

    pub fn set(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Set {
        Set::new(key.into(), value.into())
    }

    pub fn delete(&mut self, key: impl AsRef<Key>) -> Delete {
        Delete::new(key.as_ref().clone())
    }
}

/// A snapshot of dataset at a particular point in time.
pub struct Snapshot;

impl Snapshot {
    pub fn get(&self, key: impl AsRef<Key>) -> Get {
        Get::new(key.as_ref().clone())
    }

    pub fn batch_get(&self, keys: impl AsRef<[Key]>) -> BatchGet {
        BatchGet::new(keys.as_ref().to_vec().clone())
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

/// An unresolved [`Client`](struct.Client.html) connection to a TiKV cluster.
///
/// Once resolved it will result in a connected [`Client`](struct.Client.html).
///
/// ```rust,no_run
/// use tikv_client::{Config, transaction::{Client, Connect}};
/// use futures::Future;
///
/// let connect: Connect = Client::new(&Config::default());
/// let client: Client = connect.wait().unwrap();
/// ```
pub struct Connect {
    config: Config,
}

impl Connect {
    fn new(config: Config) -> Self {
        Connect { config }
    }
}

impl Future for Connect {
    type Item = Client;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _config = &self.config;
        unimplemented!()
    }
}

/// The TiKV transactional [`Client`](struct.Client.html) is used to issue requests to the TiKV server and PD cluster.
pub struct Client {}

impl Client {
    /// Create a new [`Client`](struct.Client.html) once the [`Connect`](struct.Connect.html) resolves.
    ///
    /// ```rust,no_run
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::Future;
    /// let connect = Client::new(&Config::default());
    /// let client = connect.wait();
    /// ```
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    pub fn new(config: &Config) -> Connect {
        Connect::new(config.clone())
    }

    /// Create a new [`Transaction`](struct.Transaction.html) using the timestamp from [`current_timestamp`](struct.Client.html#method.current_timestamp).
    /// 
    /// Using the transaction you can issue commands like [`get`](struct.Transaction.html#method.get) or [`set`](file:///home/hoverbear/git/client-rust/target/doc/tikv_client/transaction/struct.Transaction.html#method.set).
    /// 
    /// ```rust,no_run
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::Future;
    /// let connect = Client::new(&Config::default());
    /// let client = connect.wait().unwrap();
    /// let transaction = client.begin();
    /// // ... Issue some commands.
    /// let commit = transaction.commit();
    /// let result: () = commit.wait().unwrap();
    /// ```
    pub fn begin(&self) -> Transaction {
        unimplemented!()
    }

    /// Create a new [`Transaction`](struct.Transaction.html) at the provded timestamp.
    /// 
    /// ```rust,no_run
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::Future;
    /// let connect = Client::new(&Config::default());
    /// let client = connect.wait().unwrap();
    /// let timestamp = client.current_timestamp();
    /// let transaction = client.begin_with_timestamp(timestamp);
    /// // ... Issue some commands.
    /// let commit = transaction.commit();
    /// let result: () = commit.wait().unwrap();
    /// ```
    pub fn begin_with_timestamp(&self, _timestamp: Timestamp) -> Transaction {
        unimplemented!()
    }

    /// Get a [`Snapshot`](struct.Snapshot.html) using the timestamp from [`current_timestamp`](struct.Client.html#method.current_timestamp).
    /// 
    /// ```rust,no_run
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::Future;
    /// let connect = Client::new(&Config::default());
    /// let client = connect.wait().unwrap();
    /// let snapshot = client.snapshot();
    /// // ... Issue some commands.
    /// ```
    pub fn snapshot(&self) -> Snapshot {
        unimplemented!()
    }

    /// Retrieve the current [`Timestamp`](struct.Timestamp.html).
    /// 
    /// ```rust,no_run
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::Future;
    /// let connect = Client::new(&Config::default());
    /// let client = connect.wait().unwrap();
    /// let timestamp = client.current_timestamp();
    /// ```
    pub fn current_timestamp(&self) -> Timestamp {
        unimplemented!()
    }
}
