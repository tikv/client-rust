// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    config::Config, raw::client::Client, BoundRange, ColumnFamily, Key, KvPair, Result, Value,
};
use futures::executor::block_on;
use slog::Logger;
use std::u32;

#[derive(Clone)]
pub struct SyncClient {
    client: Client,
}

impl SyncClient {
    /// The synchronous version of RawClient
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::SyncRawClient;
    /// let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// ```
    pub fn new<S: Into<String>>(
        pd_endpoints: Vec<S>,
        logger: Option<Logger>,
    ) -> Result<SyncClient> {
        Self::new_with_config(pd_endpoints, Config::default(), logger)
    }

    /// Create a raw [`SyncClient`] with a custom configuration, and connect to the TiKV cluster.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, SyncRawClient};
    /// # use std::time::Duration;
    /// let client = SyncRawClient::new_with_config(
    ///     vec!["192.168.0.100"],
    ///     Config::default().with_timeout(Duration::from_secs(60)),
    ///     None,
    /// ).unwrap();
    /// ```
    pub fn new_with_config<S: Into<String>>(
        pd_endpoints: Vec<S>,
        config: Config,
        logger: Option<Logger>,
    ) -> Result<SyncClient> {
        let client = block_on(Client::new_with_config(pd_endpoints, config, logger)).unwrap();
        Ok(SyncClient { client })
    }

    pub fn with_cf(&self, cf: ColumnFamily) -> SyncClient {
        SyncClient {
            client: self.client.with_cf(cf),
        }
    }

    pub fn with_atomic_for_cas(&self) -> SyncClient {
        SyncClient {
            client: self.client.with_atomic_for_cas(),
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
    /// # use tikv_client::{Value, Config, SyncRawClient};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = client.get(key);
    /// let result: Option<Value> = req.unwrap();
    /// ```
    pub fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        block_on(self.client.get(key))
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
    /// # use tikv_client::{KvPair, Config, SyncRawClient};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = client.batch_get(keys);
    /// let result: Vec<KvPair> = req.unwrap();
    /// ```
    pub fn batch_get(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> Result<Vec<KvPair>> {
        block_on(self.client.batch_get(keys))
    }

    /// Create a new 'put' request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Value, Config, SyncRawClient};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// let req = client.put(key, val);
    /// let result: () = req.unwrap();
    /// ```
    pub fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        block_on(self.client.put(key, value))
    }

    /// Create a new 'batch put' request.
    ///
    /// Once resolved this request will result in the setting of the values associated with the given keys.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Result, KvPair, Key, Value, Config, SyncRawClient, IntoOwnedRange};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let kvpair1 = ("PD".to_owned(), "Go".to_owned());
    /// let kvpair2 = ("TiKV".to_owned(), "Rust".to_owned());
    /// let iterable = vec![kvpair1, kvpair2];
    /// let req = client.batch_put(iterable);
    /// let result: () = req.unwrap();
    /// ```    
    pub fn batch_put(&self, pairs: impl IntoIterator<Item = impl Into<KvPair>>) -> Result<()> {
        block_on(self.client.batch_put(pairs))
    }

    /// Create a new 'delete' request.
    ///
    /// Once resolved this request will result in the deletion of the given key.
    ///
    /// It does not return an error if the key does not exist in TiKV.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, SyncRawClient};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = client.delete(key);
    /// let result: () = req.unwrap();
    /// ```
    pub fn delete(&self, key: impl Into<Key>) -> Result<()> {
        block_on(self.client.delete(key))
    }

    /// Create a new 'batch delete' request.
    ///
    /// Once resolved this request will result in the deletion of the given keys.
    ///
    /// It does not return an error if some of the keys do not exist and will delete the others.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Config, SyncRawClient};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = client.batch_delete(keys);
    /// let result: () = req.unwrap();
    /// ```
    pub fn batch_delete(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> Result<()> {
        block_on(self.client.batch_delete(keys))
    }

    /// Create a new 'delete range' request.
    ///
    /// Once resolved this request will result in the deletion of all keys lying in the given range.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, SyncRawClient, IntoOwnedRange};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.delete_range(inclusive_range.into_owned());
    /// let result: () = req.unwrap();
    /// ```
    pub fn delete_range(&self, range: impl Into<BoundRange>) -> Result<()> {
        block_on(self.client.delete_range(range))
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
    /// # use tikv_client::{KvPair, Config, SyncRawClient, IntoOwnedRange};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.scan(inclusive_range.into_owned(), 2);
    /// let result: Vec<KvPair> = req.unwrap();
    /// ```
    pub fn scan(&self, range: impl Into<BoundRange>, limit: u32) -> Result<Vec<KvPair>> {
        block_on(self.client.scan(range, limit))
    }

    /// Create a new 'scan' request that only returns the keys.
    ///
    /// Once resolved this request will result in a `Vec` of keys that lies in the specified range.
    ///
    /// If the number of eligible keys are greater than `limit`,
    /// only the first `limit` pairs are returned, ordered by the key.
    ///
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, SyncRawClient, IntoOwnedRange};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.scan_keys(inclusive_range.into_owned(), 2);
    /// let result: Vec<Key> = req.unwrap();
    /// ```
    pub fn scan_keys(&self, range: impl Into<BoundRange>, limit: u32) -> Result<Vec<Key>> {
        block_on(self.client.scan_keys(range, limit))
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
    /// # use tikv_client::{Key, Config, SyncRawClient, IntoOwnedRange};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let inclusive_range1 = "TiDB"..="TiKV";
    /// let inclusive_range2 = "TiKV"..="TiSpark";
    /// let iterable = vec![inclusive_range1.into_owned(), inclusive_range2.into_owned()];
    /// let result = client.batch_scan(iterable, 2);
    /// ```
    pub fn batch_scan(
        &self,
        ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
        each_limit: u32,
    ) -> Result<Vec<KvPair>> {
        block_on(self.client.batch_scan(ranges, each_limit))
    }

    /// Create a new 'batch scan' request that only returns the keys.
    ///
    /// Once resolved this request will result in a set of scanners over the given keys.
    ///
    /// **Warning**: This method is experimental.
    /// The `each_limit` parameter does not limit the number of results returned of each range,
    /// instead it limits the number of results in each region of each range.
    /// As a result, you may get **more than** `each_limit` key-value pairs for each range,
    /// but you should not miss any entries.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, SyncRawClient, IntoOwnedRange};
    /// # let client = SyncRawClient::new(vec!["192.168.0.100"], None).unwrap();
    /// let inclusive_range1 = "TiDB"..="TiKV";
    /// let inclusive_range2 = "TiKV"..="TiSpark";
    /// let iterable = vec![inclusive_range1.into_owned(), inclusive_range2.into_owned()];
    /// let result = client.batch_scan(iterable, 2);
    /// ```
    pub fn batch_scan_keys(
        &self,
        ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
        each_limit: u32,
    ) -> Result<Vec<Key>> {
        block_on(self.client.batch_scan_keys(ranges, each_limit))
    }

    pub fn compare_and_swap(
        &self,
        key: impl Into<Key>,
        previous_value: impl Into<Option<Value>>,
        new_value: impl Into<Value>,
    ) -> Result<(Option<Value>, bool)> {
        block_on(self.client.compare_and_swap(key, previous_value, new_value))
    }
}
