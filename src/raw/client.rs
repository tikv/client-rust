// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::requests;
use crate::{
    pd::PdRpcClient, request::KvRequest, BoundRange, ColumnFamily, Config, Error, Key, KvPair,
    Result, Value,
};

use futures::future::Either;
use futures::prelude::*;
use std::{sync::Arc, u32};

const MAX_RAW_KV_SCAN_LIMIT: u32 = 10240;

/// The TiKV raw [`Client`](Client) is used to issue requests to the TiKV server and PD cluster.
#[derive(Clone)]
pub struct Client {
    rpc: Arc<PdRpcClient>,
    cf: Option<ColumnFamily>,
    key_only: bool,
}

impl Client {
    /// Create a new [`Client`](Client).
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = RawClient::new(Config::default()).unwrap();
    /// # });
    /// ```
    pub fn new(config: Config) -> Result<Client> {
        let rpc = Arc::new(PdRpcClient::connect(&config)?);
        Ok(Client {
            rpc,
            cf: None,
            key_only: false,
        })
    }

    /// Set the column family of requests.
    ///
    /// This function returns a new `Client`, requests created with it will have the
    /// supplied column family constraint. The original `Client` can still be used.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = RawClient::new(Config::default()).unwrap();
    /// let get_request = client.with_cf("write").get("foo".to_owned());
    /// # });
    /// ```
    pub fn with_cf(&self, cf: impl Into<ColumnFamily>) -> Client {
        Client {
            rpc: self.rpc.clone(),
            cf: Some(cf.into()),
            key_only: self.key_only,
        }
    }

    /// Set the `key_only` option of requests.
    ///
    /// This function returns a new `Client`, requests created with it will have the
    /// supplied `key_only` option. The original `Client` can still be used. `key_only`
    /// is only relevant for `scan`-like requests, for other kinds of request, it
    /// will be ignored.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, RawClient, ToOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = RawClient::new(Config::default()).unwrap();
    /// let scan_request = client.with_key_only(true).scan(("TiKV"..="TiDB").to_owned(), 2);
    /// # });
    /// ```
    pub fn with_key_only(&self, key_only: bool) -> Client {
        Client {
            rpc: self.rpc.clone(),
            cf: self.cf.clone(),
            key_only,
        }
    }

    /// Create a new 'get' request.
    ///
    /// Once resolved this request will result in the fetching of the value associated with the
    /// given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Value, Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = client.get(key);
    /// let result: Option<Value> = req.await.unwrap();
    /// # });
    /// ```
    pub fn get(&self, key: impl Into<Key>) -> impl Future<Output = Result<Option<Value>>> {
        requests::new_raw_get_request(key, self.cf.clone()).execute(self.rpc.clone())
    }

    /// Create a new 'batch get' request.
    ///
    /// Once resolved this request will result in the fetching of the values associated with the
    /// given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{KvPair, Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = client.batch_get(keys);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// # });
    /// ```
    pub fn batch_get(
        &self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> impl Future<Output = Result<Vec<KvPair>>> {
        requests::new_raw_batch_get_request(keys, self.cf.clone()).execute(self.rpc.clone())
    }

    /// Create a new 'put' request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Value, Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// let req = client.put(key, val);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub fn put(
        &self,
        key: impl Into<Key>,
        value: impl Into<Value>,
    ) -> impl Future<Output = Result<()>> {
        requests::new_raw_put_request(key, value, self.cf.clone()).execute(self.rpc.clone())
    }

    /// Create a new 'batch put' request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Error, Result, KvPair, Key, Value, Config, RawClient, ToOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).unwrap();
    /// let kvpair1 = ("PD".to_owned(), "Go".to_owned());
    /// let kvpair2 = ("TiKV".to_owned(), "Rust".to_owned());
    /// let iterable = vec![kvpair1, kvpair2];
    /// let req = client.batch_put(iterable);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub fn batch_put(
        &self,
        pairs: impl IntoIterator<Item = impl Into<KvPair>>,
    ) -> impl Future<Output = Result<()>> {
        requests::new_raw_batch_put_request(pairs, self.cf.clone()).execute(self.rpc.clone())
    }

    /// Create a new 'delete' request.
    ///
    /// Once resolved this request will result in the deletion of the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = client.delete(key);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub fn delete(&self, key: impl Into<Key>) -> impl Future<Output = Result<()>> {
        requests::new_raw_delete_request(key, self.cf.clone()).execute(self.rpc.clone())
    }

    /// Create a new 'batch delete' request.
    ///
    /// Once resolved this request will result in the deletion of the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = client.batch_delete(keys);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub fn batch_delete(
        &self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> impl Future<Output = Result<()>> {
        requests::new_raw_batch_delete_request(keys, self.cf.clone()).execute(self.rpc.clone())
    }

    /// Create a new 'delete range' request.
    ///
    /// Once resolved this request will result in the deletion of all keys over the given range.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Config, RawClient, ToOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.delete_range(inclusive_range.to_owned());
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub fn delete_range(&self, range: impl Into<BoundRange>) -> impl Future<Output = Result<()>> {
        requests::new_raw_delete_range_request(range, self.cf.clone()).execute(self.rpc.clone())
    }

    /// Create a new 'scan' request.
    ///
    /// Once resolved this request will result in a scanner over the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{KvPair, Config, RawClient, ToOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.scan(inclusive_range.to_owned(), 2);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// # });
    /// ```
    pub fn scan(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<KvPair>>> {
        if limit > MAX_RAW_KV_SCAN_LIMIT {
            Either::Right(future::err(Error::max_scan_limit_exceeded(
                limit,
                MAX_RAW_KV_SCAN_LIMIT,
            )))
        } else {
            Either::Left(
                requests::new_raw_scan_request(range, limit, self.key_only, self.cf.clone())
                    .execute(self.rpc.clone()),
            )
        }
    }

    /// Create a new 'batch scan' request.
    ///
    /// Once resolved this request will result in a set of scanners over the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Config, RawClient, ToOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(Config::default()).unwrap();
    /// let inclusive_range1 = "TiDB"..="TiKV";
    /// let inclusive_range2 = "TiKV"..="TiSpark";
    /// let iterable = vec![inclusive_range1.to_owned(), inclusive_range2.to_owned()];
    /// let req = client.batch_scan(iterable, 2);
    /// let result = req.await;
    /// # });
    /// ```
    pub fn batch_scan(
        &self,
        ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
        each_limit: u32,
    ) -> impl Future<Output = Result<Vec<KvPair>>> {
        if each_limit > MAX_RAW_KV_SCAN_LIMIT {
            Either::Right(future::err(Error::max_scan_limit_exceeded(
                each_limit,
                MAX_RAW_KV_SCAN_LIMIT,
            )))
        } else {
            Either::Left(
                requests::new_raw_batch_scan_request(
                    ranges,
                    each_limit,
                    self.key_only,
                    self.cf.clone(),
                )
                .execute(self.rpc.clone()),
            )
        }
    }
}
