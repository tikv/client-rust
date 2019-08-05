// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{Mutation, Scanner, Timestamp};
use crate::{Key, KvPair, Result, Value};

use derive_new::new;
use std::collections::BTreeMap;
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
/// let txn = client.begin().await.unwrap();
/// # });
/// ```
#[derive(new)]
pub struct Transaction {
    snapshot: Snapshot,
    #[new(default)]
    mutations: BTreeMap<Key, Mutation>,
}

impl Transaction {
    /// Gets the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Value, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = txn.get(key);
    /// let result: Value = req.await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn get(&self, _key: impl Into<Key>) -> Result<Value> {
        unimplemented!()
    }

    /// Gets the values associated with the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{KvPair, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin().await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = txn.batch_get(keys);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn batch_get(
        &self,
        _keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<Vec<KvPair>> {
        unimplemented!()
    }

    pub fn scan(&self, _range: impl RangeBounds<Key>) -> Scanner {
        unimplemented!()
    }

    pub fn scan_reverse(&self, _range: impl RangeBounds<Key>) -> Scanner {
        unimplemented!()
    }

    /// Sets the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Value, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// txn.set(key, val);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub fn set(&mut self, key: impl Into<Key>, value: impl Into<Value>) {
        self.mutations
            .insert(key.into(), Mutation::Put(value.into()));
    }

    /// Deletes the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// txn.delete(key);
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub fn delete(&mut self, key: impl Into<Key>) {
        self.mutations.insert(key.into(), Mutation::Del);
    }

    /// Locks the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connect = Client::connect(Config::default());
    /// # let connected_client = connect.await.unwrap();
    /// let mut txn = connected_client.begin().await.unwrap();
    /// txn.lock_keys(vec!["TiKV".to_owned(), "Rust".to_owned()]);
    /// // ... Do some actions.
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub fn lock_keys(&mut self, keys: impl IntoIterator<Item = impl Into<Key>>) {
        self.mutations
            .extend(keys.into_iter().map(|key| (key.into(), Mutation::Lock)));
    }

    /// Commits the actions of the transaction.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connect = Client::connect(Config::default());
    /// # let connected_client = connect.await.unwrap();
    /// let mut txn = connected_client.begin().await.unwrap();
    /// // ... Do some actions.
    /// let req = txn.commit();
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn commit(&mut self) -> Result<()> {
        self.prewrite().await?;
        self.commit_primary().await?;
        // FIXME: return from this method once the primary key is committed
        let _ = self.commit_secondary().await;
        Ok(())
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
    /// let txn = connected_client.begin().await.unwrap();
    /// // ... Do some actions.
    /// let ts: Timestamp = txn.start_ts();
    /// # });
    /// ```
    pub fn start_ts(&self) -> Timestamp {
        self.snapshot().timestamp
    }

    /// Gets the `Snapshot` the transaction is operating on.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, transaction::{Client, Snapshot}};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connect = Client::connect(Config::default());
    /// # let connected_client = connect.await.unwrap();
    /// let txn = connected_client.begin().await.unwrap();
    /// // ... Do some actions.
    /// let snap: &Snapshot = txn.snapshot();
    /// # });
    /// ```
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    async fn prewrite(&mut self) -> Result<()> {
        unimplemented!()
    }

    async fn commit_primary(&mut self) -> Result<()> {
        unimplemented!()
    }

    async fn commit_secondary(&mut self) -> Result<()> {
        unimplemented!()
    }
}

pub struct TxnInfo {
    pub txn: u64,
    pub status: u64,
}

/// A snapshot of dataset at a particular point in time.
#[derive(new)]
pub struct Snapshot {
    timestamp: Timestamp,
}

impl Snapshot {
    /// Gets the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Value, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let snapshot = connected_client.snapshot().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = snapshot.get(key);
    /// let result: Value = req.await.unwrap();
    /// # });
    /// ```
    pub async fn get(&self, _key: impl Into<Key>) -> Result<Value> {
        unimplemented!()
    }

    /// Gets the values associated with the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{KvPair, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin().await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = txn.batch_get(keys);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn batch_get(
        &self,
        _keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<Vec<KvPair>> {
        unimplemented!()
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
