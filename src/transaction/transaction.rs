// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::{PdClient, PdRpcClient},
    request::KvRequest,
    transaction::{buffer::Buffer, requests::*, Timestamp},
    ErrorKind, Key, KvPair, Result, Value,
};

use derive_new::new;
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
/// let connect = TransactionClient::connect(Config::default());
/// let client = connect.await.unwrap();
/// let txn = client.begin().await.unwrap();
/// # });
/// ```
#[derive(new)]
pub struct Transaction {
    timestamp: Timestamp,
    #[new(default)]
    buffer: Buffer,
    rpc: Arc<PdRpcClient>,
}

impl Transaction {
    /// Gets the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, transaction::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin().await.unwrap();
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
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let mut txn = connected_client.begin().await.unwrap();
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
    pub fn set(&self, key: impl Into<Key>, value: impl Into<Value>) {
        self.buffer.put(key.into(), value.into());
    }

    /// Deletes the given key.
    ///
    /// ```rust,no_run
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
    pub fn delete(&self, key: impl Into<Key>) {
        self.buffer.delete(key.into());
    }

    /// Locks the given keys.
    ///
    /// ```rust,no_run
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
    pub fn lock_keys(&self, keys: impl IntoIterator<Item = impl Into<Key>>) {
        for key in keys {
            self.buffer.lock(key.into());
        }
    }

    /// Commits the actions of the transaction.
    ///
    /// ```rust,no_run
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
        TwoPhaseCommitter::new(
            self.buffer.to_proto_mutations(),
            self.timestamp.into_version(),
            self.rpc.clone(),
        )
        .commit()
        .await
    }
}

#[derive(new)]
struct TwoPhaseCommitter {
    mutations: Vec<kvrpcpb::Mutation>,
    start_version: u64,
    rpc: Arc<PdRpcClient>,
}

impl TwoPhaseCommitter {
    async fn commit(mut self) -> Result<()> {
        if self.mutations.is_empty() {
            return Ok(());
        }
        self.prewrite().await?;
        match self.commit_primary().await {
            // FIXME: commit or rollback in background
            Ok(commit_version) => {
                let _ = self.commit_secondary(commit_version).await;
                Ok(())
            }
            Err(e) => {
                match e.kind() {
                    ErrorKind::Io(_) | ErrorKind::Grpc(_) => {}
                    _ => {
                        let _ = self.rollback_secondary().await;
                    }
                }
                Err(e)
            }
        }
    }

    async fn prewrite(&mut self) -> Result<()> {
        let primary_lock = self.mutations[0].key.clone().into();
        let lock_ttl = calculate_ttl(&self.mutations);
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
            .await?;
        Ok(commit_version)
    }

    async fn commit_secondary(&mut self, commit_version: u64) -> Result<()> {
        let mutations = mem::replace(&mut self.mutations, Vec::default());
        let keys = mutations
            .into_iter()
            .skip(1) // skip primary key
            .map(|mutation| mutation.key.into())
            .collect();
        new_commit_request(keys, self.start_version, commit_version)
            .execute(self.rpc.clone())
            .await
    }

    async fn rollback_secondary(&mut self) -> Result<()> {
        let mutations = mem::replace(&mut self.mutations, Vec::default());
        let keys = mutations
            .into_iter()
            .skip(1) // skip primary key
            .map(|mutation| mutation.key.into())
            .collect();
        new_batch_rollback_request(keys, self.start_version)
            .execute(self.rpc.clone())
            .await
    }
}

fn calculate_ttl(mutations: &[kvrpcpb::Mutation]) -> u64 {
    mutations
        .iter()
        .map(|mutation| mutation.key.len() + mutation.value.len())
        .sum::<usize>() as u64
}
