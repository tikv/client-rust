// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::PdRpcClient,
    request::KvRequest,
    transaction::{
        requests::{MvccBatchGet, MvccGet},
        Mutation, MutationValue, Timestamp,
    },
    Key, KvPair, Result, Value,
};

use derive_new::new;
use futures::stream::BoxStream;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::{collections::BTreeMap, ops::RangeBounds};

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
/// # #![feature(async_await)]
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
    pub timestamp: Timestamp,
    #[new(default)]
    buffer: Buffer,
    rpc: Arc<PdRpcClient>,
}

#[derive(Default)]
struct Buffer {
    mutations: Mutex<BTreeMap<Key, Mutation>>,
}

impl Buffer {
    async fn get_or_else<F, Fut>(&self, key: Key, f: F) -> Result<Option<Value>>
    where
        F: FnOnce(Key) -> Fut,
        Fut: Future<Output = Result<Option<Value>>>,
    {
        match self.get_from_mutations(&key) {
            MutationValue::Determined(value) => Ok(value),
            MutationValue::Undetermined => {
                let value = f(key.clone()).await?;
                let mut mutations = self.mutations.lock().unwrap();
                mutations.insert(key, Mutation::Cached(value.clone()));
                Ok(value)
            }
        }
    }

    async fn batch_get_or_else<F, Fut, Iter>(
        &self,
        keys: impl Iterator<Item = Key>,
        f: F,
    ) -> Result<impl Iterator<Item = (Key, Option<Value>)>>
    where
        F: FnOnce(Vec<Key>) -> Fut,
        Fut: Future<Output = Result<Iter>>,
        Iter: Iterator<Item = (Key, Option<Value>)>,
    {
        let mutations = self.mutations.lock().unwrap();
        // Partition the keys into those we have buffered and those we have to
        // get from the store.
        let (undetermined_keys, cached_results): (Vec<(Key, MutationValue)>, _) = keys
            .map(|key| {
                let value = mutations
                    .get(&key)
                    .map(Mutation::get_value)
                    .unwrap_or(MutationValue::Undetermined);
                (key, value)
            })
            .partition(|(_, v)| *v == MutationValue::Undetermined);

        let cached_results = cached_results.into_iter().map(|(k, v)| (k, v.unwrap()));

        let undetermined_keys = undetermined_keys.into_iter().map(|(k, _)| k).collect();
        let fetched_results = f(undetermined_keys).await?;

        let results = cached_results.chain(fetched_results);
        Ok(results)
    }

    fn get_from_mutations(&self, key: &Key) -> MutationValue {
        self.mutations
            .lock()
            .unwrap()
            .get(&key)
            .map(Mutation::get_value)
            .unwrap_or(MutationValue::Undetermined)
    }

    fn lock(&self, key: Key) {
        self.mutations
            .lock()
            .unwrap()
            // Mutated keys don't need a lock.
            .entry(key)
            .or_insert(Mutation::Lock);
    }

    fn put(&self, key: Key, value: Value) {
        self.mutations
            .lock()
            .unwrap()
            .insert(key, Mutation::Put(value));
    }

    fn delete(&self, key: Key) {
        self.mutations.lock().unwrap().insert(key, Mutation::Del);
    }
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
    /// let result: Option<Value> = txn.get(key).await.unwrap();
    /// // Finish the transaction...
    /// txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        let key = key.into();
        self.buffer
            .get_or_else(key, async move |key| {
                MvccGet {
                    key,
                    version: self.timestamp.into_version(),
                }
                .execute(self.rpc.clone())
                .await
            })
            .await
    }

    /// Gets the values associated with the given keys. The returned iterator is in the same order
    /// as the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
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
        let version = self.timestamp.into_version();
        let rpc = self.rpc.clone();
        self.buffer
            .batch_get_or_else(keys.into_iter().map(|k| k.into()), async move |keys| {
                Ok(MvccBatchGet { keys, version }
                    .execute(rpc)
                    .await?
                    .into_iter()
                    .map(|p| (p.0, Some(p.1))))
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
    pub fn set(&self, key: impl Into<Key>, value: impl Into<Value>) {
        self.buffer.put(key.into(), value.into());
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
    pub fn delete(&self, key: impl Into<Key>) {
        self.buffer.delete(key.into());
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
    pub fn lock_keys(&self, keys: impl IntoIterator<Item = impl Into<Key>>) {
        for key in keys {
            self.buffer.lock(key.into());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn set_and_get_from_buffer() {
        let buffer = Buffer::default();
        buffer.put(b"key1".to_vec().into(), b"value1".to_vec().into());
        buffer.put(b"key2".to_vec().into(), b"value2".to_vec().into());
        assert_eq!(
            block_on(buffer.get_or_else(b"key1".to_vec().into(), async move |_| panic!()))
                .unwrap()
                .unwrap(),
            b"value1".to_vec().into()
        );

        buffer.delete(b"key2".to_vec().into());
        buffer.put(b"key1".to_vec().into(), b"value".to_vec().into());
        assert_eq!(
            block_on(buffer.batch_get_or_else(
                vec![b"key2".to_vec().into(), b"key1".to_vec().into()].into_iter(),
                async move |_| Ok(::std::iter::empty())
            ))
            .unwrap()
            .collect::<Vec<_>>(),
            vec![
                (Key::from(b"key2".to_vec()), None),
                (
                    Key::from(b"key1".to_vec()),
                    Some(Value::from(b"value".to_vec()))
                ),
            ]
        );
    }
}
