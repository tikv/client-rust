// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::requests;
use crate::{
    config::Config,
    pd::PdRpcClient,
    request::{KvRequest, OPTIMISTIC_BACKOFF},
    BoundRange, ColumnFamily, Error, Key, KvPair, Result, Value,
};
use std::{sync::Arc, u32};

const MAX_RAW_KV_SCAN_LIMIT: u32 = 10240;

/// The TiKV raw `Client` is used to interact with TiKV using raw requests.
///
/// Raw requests don't need a wrapping transaction.
/// Each request is immediately processed once executed.
///
/// The returned results of raw requests are [`Future`](std::future::Future)s that must be awaited to execute.
#[derive(Clone)]
pub struct Client {
    rpc: Arc<PdRpcClient>,
    cf: Option<ColumnFamily>,
}

impl Client {
    /// Create a raw [`Client`](Client).
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = RawClient::new(Config::default()).await.unwrap();
    /// # });
    /// ```
    pub async fn new(config: Config) -> Result<Client> {
        let rpc = Arc::new(PdRpcClient::connect(&config, false).await?);
        Ok(Client { rpc, cf: None })
    }

    /// Set the column family of requests.
    ///
    /// This function returns a new `Client`, requests created with it will have the
    /// supplied column family constraint. The original `Client` can still be used.
    ///
    /// By default, raw client uses the `Default` column family.
    ///
    /// For normal users of the raw API, you don't need to use other column families.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Config, RawClient, ColumnFamily};
    /// # use futures::prelude::*;
    /// # use std::convert::TryInto;
    /// # futures::executor::block_on(async {
    /// let client = RawClient::new(Config::default()).await.unwrap().with_cf(ColumnFamily::Write);
    /// let get_request = client.get("foo".to_owned());
    /// # });
    /// ```
    pub fn with_cf(&self, cf: ColumnFamily) -> Client {
        Client {
            rpc: self.rpc.clone(),
            cf: Some(cf),
        }
    }

    /// Create a new 'get' request.
    ///
    /// Once resolved this request will result in the fetching of the value associated with the
    /// given key.
    ///
    /// Retuning `Ok(None)` indicates the key does not exist in TiKV.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Value, Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = client.get(key);
    /// let result: Option<Value> = req.await.unwrap();
    /// # });
    /// ```
    pub async fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        requests::new_raw_get_request(key, self.cf.clone())
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
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
    /// # use tikv_client::{KvPair, Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = client.batch_get(keys);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// # });
    /// ```
    pub async fn batch_get(
        &self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<Vec<KvPair>> {
        requests::new_raw_batch_get_request(keys, self.cf.clone())
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
    }

    /// Create a new 'put' request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// let req = client.put(key, val);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        requests::new_raw_put_request(key, value, self.cf.clone())
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
    }

    /// Create a new 'batch put' request.
    ///
    /// Once resolved this request will result in the setting of the values associated with the given keys.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Error, Result, KvPair, Key, Value, Config, RawClient, ToOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).await.unwrap();
    /// let kvpair1 = ("PD".to_owned(), "Go".to_owned());
    /// let kvpair2 = ("TiKV".to_owned(), "Rust".to_owned());
    /// let iterable = vec![kvpair1, kvpair2];
    /// let req = client.batch_put(iterable);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn batch_put(
        &self,
        pairs: impl IntoIterator<Item = impl Into<KvPair>>,
    ) -> Result<()> {
        requests::new_raw_batch_put_request(pairs, self.cf.clone())
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
    }

    /// Create a new 'delete' request.
    ///
    /// Once resolved this request will result in the deletion of the given key.
    ///
    /// It does not return an error if the key does not exist in TiKV.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = client.delete(key);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn delete(&self, key: impl Into<Key>) -> Result<()> {
        requests::new_raw_delete_request(key, self.cf.clone())
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
    }

    /// Create a new 'batch delete' request.
    ///
    /// Once resolved this request will result in the deletion of the given keys.
    ///
    /// It does not return an error if some of the keys do not exist and will delete the others.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = client.batch_delete(keys);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn batch_delete(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> Result<()> {
        requests::new_raw_batch_delete_request(keys, self.cf.clone())
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
    }

    /// Create a new 'delete range' request.
    ///
    /// Once resolved this request will result in the deletion of all keys lying in the given range.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, RawClient, ToOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).await.unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.delete_range(inclusive_range.to_owned());
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn delete_range(&self, range: impl Into<BoundRange>) -> Result<()> {
        requests::new_raw_delete_range_request(range, self.cf.clone())
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
    }

    /// Create a new 'scan' request.
    ///
    /// Once resolved this request will result in a `Vec` of key-value pairs that lies in the specified range.
    ///
    /// If the number of eligible key-value pairs are greater than `limit`,
    /// only the first `limit` pairs are returned, ordered by the key.
    ///
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{KvPair, Config, RawClient, ToOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).await.unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.scan(inclusive_range.to_owned(), 2, true);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// # });
    /// ```
    pub async fn scan(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
        key_only: bool,
    ) -> Result<Vec<KvPair>> {
        if limit > MAX_RAW_KV_SCAN_LIMIT {
            return Err(Error::max_scan_limit_exceeded(limit, MAX_RAW_KV_SCAN_LIMIT));
        }

        let res = requests::new_raw_scan_request(range, limit, key_only, self.cf.clone())
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await;
        res.map(|mut s| {
            s.truncate(limit as usize);
            s
        })
    }

    /// Create a new 'batch scan' request.
    ///
    /// Once resolved this request will result in a set of scanners over the given keys.
    ///
    /// **Warning**: This method is experimental. The `each_limit` parameter does not work as expected.
    /// It does not limit the number of results returned of each range,
    /// instead it limits the number of results in each region of each range.
    /// As a result, you may get **more than** `each_limit` key-value pairs for each range.
    /// But you should not miss any entries.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, RawClient, ToOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).await.unwrap();
    /// let inclusive_range1 = "TiDB"..="TiKV";
    /// let inclusive_range2 = "TiKV"..="TiSpark";
    /// let iterable = vec![inclusive_range1.to_owned(), inclusive_range2.to_owned()];
    /// let req = client.batch_scan(iterable, 2, false);
    /// let result = req.await;
    /// # });
    /// ```
    pub async fn batch_scan(
        &self,
        ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
        each_limit: u32,
        key_only: bool,
    ) -> Result<Vec<KvPair>> {
        if each_limit > MAX_RAW_KV_SCAN_LIMIT {
            return Err(Error::max_scan_limit_exceeded(
                each_limit,
                MAX_RAW_KV_SCAN_LIMIT,
            ));
        }

        requests::new_raw_batch_scan_request(ranges, each_limit, key_only, self.cf.clone())
            .execute(self.rpc.clone(), OPTIMISTIC_BACKOFF)
            .await
    }
}
