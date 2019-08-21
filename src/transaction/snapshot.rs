// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{transaction::Timestamp, Key, KvPair, Result, Value};

use derive_new::new;
use futures::stream::BoxStream;
use std::ops::RangeBounds;

/// A snapshot of dataset at a particular point in time.
#[derive(new)]
pub struct Snapshot {
    pub timestamp: Timestamp,
}

impl Snapshot {
    /// Gets the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = TransactionClient::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let snapshot = connected_client.snapshot().await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let result: Option<Value> = snapshot.get(key).await.unwrap();
    /// # });
    /// ```
    pub async fn get(&self, _key: impl Into<Key>) -> Result<Option<Value>> {
        unimplemented!()
    }

    /// Gets the values associated with the given keys. The returned iterator is in the same order
    /// as the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::collections::HashMap;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = TransactionClient::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let snapshot = connected_client.snapshot().await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let result: HashMap<Key, Value> = snapshot
    ///     .batch_get(keys)
    ///     .await
    ///     .unwrap()
    ///     .filter_map(|(k, v)| v.map(move |v| (k, v))).collect();
    /// # });
    /// ```
    pub async fn batch_get(
        &self,
        _keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<impl Iterator<Item = (Key, Option<Value>)>> {
        Ok(std::iter::repeat_with(|| unimplemented!()))
    }

    pub fn scan(&self, range: impl RangeBounds<Key>) -> BoxStream<Result<KvPair>> {
        drop(range);
        unimplemented!()
    }

    pub fn scan_reverse(&self, range: impl RangeBounds<Key>) -> BoxStream<Result<KvPair>> {
        drop(range);
        unimplemented!()
    }
}
