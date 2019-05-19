// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! Transactional related functionality.
//!
//! Using the [`transaction::Client`](struct.Client.html) you can utilize TiKV's transactional interface.
//!
//! This interface offers SQL-like transactions on top of the raw interface.
//!
//! **Warning:** It is not advisable to use both raw and transactional functionality in the same keyspace.
//!

use crate::{Config, Error, Key, KvPair, Value};
use futures::{Future, Poll, Stream};
use std::ops::RangeBounds;

/// The TiKV transactional [`Client`](struct.Client.html) is used to issue requests to the TiKV server and PD cluster.
pub struct Client;

impl Client {
    /// Create a new [`Client`](struct.Client.html) once the [`Connect`](struct.Connect.html) resolves.
    ///
    /// ```rust,no_run
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::Future;
    /// let connect = Client::new(Config::default());
    /// let client = connect.wait();
    /// ```
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    pub fn new(config: Config) -> Connect {
        Connect::new(config)
    }

    /// Create a new [`Transaction`](struct.Transaction.html) using the timestamp from [`current_timestamp`](struct.Client.html#method.current_timestamp).
    ///
    /// Using the transaction you can issue commands like [`get`](struct.Transaction.html#method.get) or [`set`](file:///home/hoverbear/git/client-rust/target/doc/tikv_client/transaction/struct.Transaction.html#method.set).
    ///
    /// ```rust,no_run
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::Future;
    /// let connect = Client::new(Config::default());
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
    /// let connect = Client::new(Config::default());
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
    /// let connect = Client::new(Config::default());
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
    /// let connect = Client::new(Config::default());
    /// let client = connect.wait().unwrap();
    /// let timestamp = client.current_timestamp();
    /// ```
    pub fn current_timestamp(&self) -> Timestamp {
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
/// let connect: Connect = Client::new(Config::default());
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

pub enum Mutation {
    Put(Key, Value),
    Del(Key),
    Lock(Key),
    Rollback(Key),
}

pub struct TxnInfo {
    pub txn: u64,
    pub status: u64,
}

impl Future for Connect {
    type Item = Client;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _config = &self.config;
        unimplemented!()
    }
}

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
    ///
    /// ```rust,no_run
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::Future;
    /// let connect = Client::new(Config::default());
    /// let client = connect.wait().unwrap();
    /// let txn = client.begin();
    /// ```
    pub fn new(snapshot: Snapshot) -> Self {
        Self { snapshot }
    }

    /// Commit the actions of the transaction.
    ///
    /// Once committed, it is no longer possible to `rollback` the actions in the transaction.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::Future;
    /// # let connect = Client::new(Config::default());
    /// # let connected_client = connect.wait().unwrap();
    /// let txn = connected_client.begin();
    /// // ... Do some actions.
    /// let req = txn.commit();
    /// let result: () = req.wait().unwrap();
    /// ```
    pub fn commit(self) -> Commit {
        Commit::new(self)
    }

    /// Rollback the actions of the transaction.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::Future;
    /// # let connect = Client::new(Config::default());
    /// # let connected_client = connect.wait().unwrap();
    /// let txn = connected_client.begin();
    /// // ... Do some actions.
    /// let req = txn.rollback();
    /// let result: () = req.wait().unwrap();
    /// ```
    pub fn rollback(self) -> Rollback {
        Rollback::new(self)
    }

    /// Lock the given keys.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::Future;
    /// # let connect = Client::new(Config::default());
    /// # let connected_client = connect.wait().unwrap();
    /// let mut txn = connected_client.begin();
    /// // ... Do some actions.
    /// let req = txn.lock_keys(vec!["TiKV", "Rust"]);
    /// let result: () = req.wait().unwrap();
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
    /// # use tikv_client::{Config, transaction::{Client, Timestamp}};
    /// # use futures::Future;
    /// # let connect = Client::new(Config::default());
    /// # let connected_client = connect.wait().unwrap();
    /// let txn = connected_client.begin();
    /// // ... Do some actions.
    /// let ts: Timestamp = txn.start_ts();
    /// ```
    pub fn start_ts(&self) -> Timestamp {
        unimplemented!()
    }

    /// Get the `Snapshot` the transaction is operating on.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, transaction::{Client, Snapshot}};
    /// # use futures::Future;
    /// # let connect = Client::new(Config::default());
    /// # let connected_client = connect.wait().unwrap();
    /// let txn = connected_client.begin();
    /// // ... Do some actions.
    /// let snap: Snapshot = txn.snapshot();
    /// ```
    pub fn snapshot(&self) -> Snapshot {
        unimplemented!()
    }

    /// Set the isolation level of the transaction.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, transaction::{Client, IsolationLevel}};
    /// # use futures::Future;
    /// # let connect = Client::new(Config::default());
    /// # let connected_client = connect.wait().unwrap();
    /// let mut txn = connected_client.begin();
    /// txn.set_isolation_level(IsolationLevel::SnapshotIsolation);
    /// ```
    pub fn set_isolation_level(&mut self, _level: IsolationLevel) {
        unimplemented!()
    }

    /// Create a new [`Get`](struct.Get.html) request.
    ///
    /// Once resolved this request will result in the fetching of the value associated with the
    /// given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, transaction::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let mut txn = connected_client.begin();
    /// let key = "TiKV";
    /// let req = txn.get(key);
    /// let result: Value = req.wait().unwrap();
    /// // Finish the transaction...
    /// txn.commit().wait().unwrap();
    /// ```
    pub fn get(&self, key: impl Into<Key>) -> Get {
        self.snapshot.get(key.into())
    }

    /// Create a new [`BatchGet`](struct.BatchGet.html) request.
    ///
    /// Once resolved this request will result in the fetching of the values associated with the
    /// given keys.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{KvPair, Config, transaction::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let mut txn = connected_client.begin();
    /// let keys = vec!["TiKV", "TiDB"];
    /// let req = txn.batch_get(keys);
    /// let result: Vec<KvPair> = req.wait().unwrap();
    /// // Finish the transaction...
    /// txn.commit().wait().unwrap();
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

    /// Create a new [`Set`](struct.Set.html) request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, transaction::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let mut txn = connected_client.begin();
    /// let key = "TiKV";
    /// let val = "TiKV";
    /// let req = txn.set(key, val);
    /// let result: () = req.wait().unwrap();
    /// // Finish the transaction...
    /// txn.commit().wait().unwrap();
    /// ```
    pub fn set(&mut self, key: impl Into<Key>, value: impl Into<Value>) -> Set {
        Set::new(key.into(), value.into())
    }

    /// Create a new [`Delete`](struct.Delete.html) request.
    ///
    /// Once resolved this request will result in the deletion of the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, transaction::Client};
    /// # use futures::Future;
    /// # let connecting_client = Client::new(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.wait().unwrap();
    /// let mut txn = connected_client.begin();
    /// let key = "TiKV";
    /// let req = txn.delete(key);
    /// let result: () = req.wait().unwrap();
    /// // Finish the transaction...
    /// txn.commit().wait().unwrap();
    /// ```
    pub fn delete(&mut self, key: impl Into<Key>) -> Delete {
        Delete::new(key.into())
    }
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

/// An unresolved [`Transaction::scan`](struct.Transaction.html#method.scan) request.
///
/// Once resolved this request will result in a scanner over the given keys.
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

/// An unresolved [`Transaction::get`](struct.Transaction.html#method.get) request.
///
/// Once resolved this request will result in the fetching of the value associated with the given
/// key.
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

/// An unresolved [`Transaction::batch_get`](struct.Transaction.html#method.batch_get) request.
///
/// Once resolved this request will result in the fetching of the values associated with the given
/// keys.
pub struct BatchGet {
    keys: Vec<Key>,
}

impl BatchGet {
    fn new(keys: Vec<Key>) -> Self {
        BatchGet { keys }
    }
}

impl Future for BatchGet {
    type Item = Vec<KvPair>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _keys = &self.keys;
        unimplemented!()
    }
}

/// An unresolved [`Transaction::commit`](struct.Transaction.html#method.commit) request.
///
/// Once resolved this request will result in the committing of the transaction.
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

/// An unresolved [`Transaction::rollback`](struct.Transaction.html#method.rollback) request.
///
/// Once resolved this request will result in the rolling back of the transaction.
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

/// An unresolved [`Transaction::lock_keys`](struct.Transaction.html#method.lock_keys) request.
///
/// Once resolved this request will result in the locking of the given keys.
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

/// An unresolved [`Transaction::set`](struct.Transaction.html#method.set) request.
///
/// Once resolved this request will result in the setting of the value associated with the given
/// key.
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

/// An unresolved [`Transaction::delete`](struct.Transaction.html#method.delete) request.
///
/// Once resolved this request will result in the deletion of the given key.
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
