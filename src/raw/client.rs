// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.
use core::ops::Range;

use std::str::FromStr;
use std::sync::Arc;

use log::debug;
use tokio::time::sleep;

use crate::backoff::{DEFAULT_REGION_BACKOFF, DEFAULT_STORE_BACKOFF};
use crate::common::Error;
use crate::config::Config;
use crate::pd::PdClient;
use crate::pd::PdRpcClient;
use crate::proto::kvrpcpb::{RawScanRequest, RawScanResponse};
use crate::proto::metapb;
use crate::raw::lowering::*;
use crate::request::CollectSingle;
use crate::request::EncodeKeyspace;
use crate::request::KeyMode;
use crate::request::Keyspace;
use crate::request::Plan;
use crate::request::TruncateKeyspace;
use crate::request::{plan, Collect};
use crate::store::{HasRegionError, RegionStore};
use crate::Backoff;
use crate::BoundRange;
use crate::ColumnFamily;
use crate::Error::RegionError;
use crate::Key;
use crate::KvPair;
use crate::Result;
use crate::Value;

const MAX_RAW_KV_SCAN_LIMIT: u32 = 10240;
const MAX_REVERSE_SCAN_BOUNDARY_RETRIES: usize = 10;

/// The TiKV raw `Client` is used to interact with TiKV using raw requests.
///
/// Raw requests don't need a wrapping transaction.
/// Each request is immediately processed once executed.
///
/// The returned results of raw request methods are [`Future`](std::future::Future)s that must be
/// awaited to execute.
pub struct Client<PdC: PdClient = PdRpcClient> {
    rpc: Arc<PdC>,
    cf: Option<ColumnFamily>,
    backoff: Backoff,
    /// Whether to use the [`atomic mode`](Client::with_atomic_for_cas).
    atomic: bool,
    keyspace: Keyspace,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Self {
            rpc: self.rpc.clone(),
            cf: self.cf.clone(),
            backoff: self.backoff.clone(),
            atomic: self.atomic,
            keyspace: self.keyspace,
        }
    }
}

impl Client<PdRpcClient> {
    /// Create a raw [`Client`] and connect to the TiKV cluster.
    ///
    /// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for
    /// PD must be provided, not the TiKV nodes. It's important to include more than one PD endpoint
    /// (include all endpoints, if possible), this helps avoid having a single point of failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::RawClient;
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// # });
    /// ```
    pub async fn new<S: Into<String>>(pd_endpoints: Vec<S>) -> Result<Self> {
        Self::new_with_config(pd_endpoints, Config::default()).await
    }

    /// Create a raw [`Client`] with a custom configuration, and connect to the TiKV cluster.
    ///
    /// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for
    /// PD must be provided, not the TiKV nodes. It's important to include more than one PD endpoint
    /// (include all endpoints, if possible), this helps avoid having a single point of failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, RawClient};
    /// # use futures::prelude::*;
    /// # use std::time::Duration;
    /// # futures::executor::block_on(async {
    /// let client = RawClient::new_with_config(
    ///     vec!["192.168.0.100"],
    ///     Config::default().with_timeout(Duration::from_secs(60)),
    /// )
    /// .await
    /// .unwrap();
    /// # });
    /// ```
    pub async fn new_with_config<S: Into<String>>(
        pd_endpoints: Vec<S>,
        config: Config,
    ) -> Result<Self> {
        let enable_codec = config.keyspace.is_some();
        let pd_endpoints: Vec<String> = pd_endpoints.into_iter().map(Into::into).collect();
        let rpc =
            Arc::new(PdRpcClient::connect(&pd_endpoints, config.clone(), enable_codec).await?);
        let keyspace = match config.keyspace {
            Some(name) => {
                let keyspace = rpc.load_keyspace(&name).await?;
                Keyspace::Enable {
                    keyspace_id: keyspace.id,
                }
            }
            None => Keyspace::Disable,
        };
        Ok(Client {
            rpc,
            cf: None,
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace,
        })
    }

    /// Create a new client which is a clone of `self`, but which uses an explicit column family for
    /// all requests.
    ///
    /// This function returns a new `Client`; requests created with the new client will use the
    /// supplied column family. The original `Client` can still be used (without the new
    /// column family).
    ///
    /// By default, raw clients use the `Default` column family.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, RawClient, ColumnFamily};
    /// # use futures::prelude::*;
    /// # use std::convert::TryInto;
    /// # futures::executor::block_on(async {
    /// let client = RawClient::new(vec!["192.168.0.100"])
    ///     .await
    ///     .unwrap()
    ///     .with_cf(ColumnFamily::Write);
    /// // Fetch a value at "foo" from the Write CF.
    /// let get_request = client.get("foo".to_owned());
    /// # });
    /// ```
    #[must_use]
    pub fn with_cf(&self, cf: ColumnFamily) -> Self {
        Client {
            rpc: self.rpc.clone(),
            cf: Some(cf),
            backoff: self.backoff.clone(),
            atomic: self.atomic,
            keyspace: self.keyspace,
        }
    }

    /// Set the [`Backoff`] strategy for retrying requests.
    /// The default strategy is [`DEFAULT_REGION_BACKOFF`](crate::backoff::DEFAULT_REGION_BACKOFF).
    /// See [`Backoff`] for more information.
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Config, RawClient, ColumnFamily};
    /// # use tikv_client::backoff::DEFAULT_REGION_BACKOFF;
    /// # use futures::prelude::*;
    /// # use std::convert::TryInto;
    /// # futures::executor::block_on(async {
    /// let client = RawClient::new(vec!["192.168.0.100"])
    ///     .await
    ///     .unwrap()
    ///     .with_backoff(DEFAULT_REGION_BACKOFF);
    /// // Fetch a value at "foo" from the Write CF.
    /// let get_request = client.get("foo".to_owned());
    /// # });
    /// ```
    #[must_use]
    pub fn with_backoff(&self, backoff: Backoff) -> Self {
        Client {
            rpc: self.rpc.clone(),
            cf: self.cf.clone(),
            backoff,
            atomic: self.atomic,
            keyspace: self.keyspace,
        }
    }

    /// Set to use the atomic mode.
    ///
    /// The only reason of using atomic mode is the
    /// [`compare_and_swap`](Client::compare_and_swap) operation. To guarantee
    /// the atomicity of CAS, write operations like [`put`](Client::put) or
    /// [`delete`](Client::delete) in atomic mode are more expensive. Some
    /// operations are not supported in the mode.
    #[must_use]
    pub fn with_atomic_for_cas(&self) -> Self {
        Client {
            rpc: self.rpc.clone(),
            cf: self.cf.clone(),
            backoff: self.backoff.clone(),
            atomic: true,
            keyspace: self.keyspace,
        }
    }
}

impl<PdC: PdClient> Client<PdC> {
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
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = client.get(key);
    /// let result: Option<Value> = req.await.unwrap();
    /// # });
    /// ```
    pub async fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        debug!("invoking raw get request");
        let key = key.into().encode_keyspace(self.keyspace, KeyMode::Raw);
        let request = new_raw_get_request(key, self.cf.clone());
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
            .retry_multi_region(self.backoff.clone())
            .merge(CollectSingle)
            .post_process_default()
            .plan();
        plan.execute().await
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
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = client.batch_get(keys);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// # });
    /// ```
    pub async fn batch_get(
        &self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<Vec<KvPair>> {
        debug!("invoking raw batch_get request");
        let keys = keys
            .into_iter()
            .map(|k| k.into().encode_keyspace(self.keyspace, KeyMode::Raw));
        let request = new_raw_batch_get_request(keys, self.cf.clone());
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
            .retry_multi_region(self.backoff.clone())
            .merge(Collect)
            .plan();
        plan.execute().await.map(|r| {
            r.into_iter()
                .map(|pair| pair.truncate_keyspace(self.keyspace))
                .collect()
        })
    }

    /// Create a new 'get key ttl' request.
    ///
    /// Once resolved this request will result in the fetching of the alive time left for the
    /// given key.
    ///
    /// Retuning `Ok(None)` indicates the key does not exist in TiKV.
    ///
    /// # Examples
    /// # use tikv_client::{Value, Config, RawClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = client.get_key_ttl_secs(key);
    /// let result: Option<Value> = req.await.unwrap();
    /// # });
    pub async fn get_key_ttl_secs(&self, key: impl Into<Key>) -> Result<Option<u64>> {
        debug!("invoking raw get_key_ttl_secs request");
        let key = key.into().encode_keyspace(self.keyspace, KeyMode::Raw);
        let request = new_raw_get_key_ttl_request(key, self.cf.clone());
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
            .retry_multi_region(self.backoff.clone())
            .merge(CollectSingle)
            .post_process_default()
            .plan();
        plan.execute().await
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
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let val = "TiKV".to_owned();
    /// let req = client.put(key, val);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        self.put_with_ttl(key, value, 0).await
    }

    pub async fn put_with_ttl(
        &self,
        key: impl Into<Key>,
        value: impl Into<Value>,
        ttl_secs: u64,
    ) -> Result<()> {
        debug!("invoking raw put request");
        let key = key.into().encode_keyspace(self.keyspace, KeyMode::Raw);
        let request =
            new_raw_put_request(key, value.into(), self.cf.clone(), ttl_secs, self.atomic);
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
            .retry_multi_region(self.backoff.clone())
            .merge(CollectSingle)
            .extract_error()
            .plan();
        plan.execute().await?;
        Ok(())
    }

    /// Create a new 'batch put' request.
    ///
    /// Once resolved this request will result in the setting of the values associated with the given keys.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Result, KvPair, Key, Value, Config, RawClient, IntoOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
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
        self.batch_put_with_ttl(pairs, std::iter::repeat(0)).await
    }

    pub async fn batch_put_with_ttl(
        &self,
        pairs: impl IntoIterator<Item = impl Into<KvPair>>,
        ttls: impl IntoIterator<Item = u64>,
    ) -> Result<()> {
        debug!("invoking raw batch_put request");
        let pairs = pairs
            .into_iter()
            .map(|pair| pair.into().encode_keyspace(self.keyspace, KeyMode::Raw));
        let request =
            new_raw_batch_put_request(pairs, ttls.into_iter(), self.cf.clone(), self.atomic);
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
            .retry_multi_region(self.backoff.clone())
            .extract_error()
            .plan();
        plan.execute().await?;
        Ok(())
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
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let key = "TiKV".to_owned();
    /// let req = client.delete(key);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn delete(&self, key: impl Into<Key>) -> Result<()> {
        debug!("invoking raw delete request");
        let key = key.into().encode_keyspace(self.keyspace, KeyMode::Raw);
        let request = new_raw_delete_request(key, self.cf.clone(), self.atomic);
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
            .retry_multi_region(self.backoff.clone())
            .merge(CollectSingle)
            .extract_error()
            .plan();
        plan.execute().await?;
        Ok(())
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
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let keys = vec!["TiKV".to_owned(), "TiDB".to_owned()];
    /// let req = client.batch_delete(keys);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn batch_delete(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> Result<()> {
        debug!("invoking raw batch_delete request");
        self.assert_non_atomic()?;
        let keys = keys
            .into_iter()
            .map(|k| k.into().encode_keyspace(self.keyspace, KeyMode::Raw));
        let request = new_raw_batch_delete_request(keys, self.cf.clone());
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
            .retry_multi_region(self.backoff.clone())
            .extract_error()
            .plan();
        plan.execute().await?;
        Ok(())
    }

    /// Create a new 'delete range' request.
    ///
    /// Once resolved this request will result in the deletion of all keys lying in the given range.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, RawClient, IntoOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.delete_range(inclusive_range.into_owned());
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    pub async fn delete_range(&self, range: impl Into<BoundRange>) -> Result<()> {
        debug!("invoking raw delete_range request");
        self.assert_non_atomic()?;
        let range = range.into().encode_keyspace(self.keyspace, KeyMode::Raw);
        let request = new_raw_delete_range_request(range, self.cf.clone());
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
            .retry_multi_region(self.backoff.clone())
            .extract_error()
            .plan();
        plan.execute().await?;
        Ok(())
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
    /// # use tikv_client::{KvPair, Config, RawClient, IntoOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.scan(inclusive_range.into_owned(), 2);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// # });
    /// ```
    pub async fn scan(&self, range: impl Into<BoundRange>, limit: u32) -> Result<Vec<KvPair>> {
        debug!("invoking raw scan request");
        self.scan_inner(range.into(), limit, false, false).await
    }

    /// Create a new 'scan' request but scans in "reverse" direction.
    ///
    /// Once resolved this request will result in a `Vec` of key-value pairs that lies in the specified range.
    ///
    /// If the number of eligible key-value pairs are greater than `limit`,
    /// only the first `limit` pairs are returned, ordered by the key.
    ///
    ///
    /// Reverse Scan queries continuous kv pairs in range [startKey, endKey),
    /// from startKey(lowerBound) to endKey(upperBound) in reverse order, up to limit pairs.
    /// The returned keys are in reversed lexicographical order.
    /// If you want to include the endKey or exclude the startKey, push a '\0' to the key.
    /// An unbounded upper bound is supported by locating the final Region before
    /// traversing Regions backwards toward the lower bound. The normal path uses
    /// paged PD batch-region scans; clients whose PD does not support that RPC
    /// fall back to walking Region boundaries from the lower bound.
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{KvPair, Config, RawClient, IntoOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.scan_reverse(inclusive_range.into_owned(), 2);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// # });
    /// ```
    pub async fn scan_reverse(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<Vec<KvPair>> {
        debug!("invoking raw reverse scan request");
        self.scan_inner(range.into(), limit, false, true).await
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
    /// # use tikv_client::{Key, Config, RawClient, IntoOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.scan_keys(inclusive_range.into_owned(), 2);
    /// let result: Vec<Key> = req.await.unwrap();
    /// # });
    /// ```
    pub async fn scan_keys(&self, range: impl Into<BoundRange>, limit: u32) -> Result<Vec<Key>> {
        debug!("invoking raw scan_keys request");
        Ok(self
            .scan_inner(range, limit, true, false)
            .await?
            .into_iter()
            .map(KvPair::into_key)
            .collect())
    }

    /// Create a new 'scan' request that only returns the keys in reverse order.
    ///
    /// Once resolved this request will result in a `Vec` of keys that lies in the specified range.
    ///
    /// If the number of eligible keys are greater than `limit`,
    /// only the first `limit` pairs are returned, ordered by the key.
    ///
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use tikv_client::{Key, Config, RawClient, IntoOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = client.scan_keys(inclusive_range.into_owned(), 2);
    /// let result: Vec<Key> = req.await.unwrap();
    /// # });
    /// ```
    pub async fn scan_keys_reverse(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
    ) -> Result<Vec<Key>> {
        debug!("invoking raw scan_keys_reverse request");
        Ok(self
            .scan_inner(range, limit, true, true)
            .await?
            .into_iter()
            .map(KvPair::into_key)
            .collect())
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
    /// # use tikv_client::{Key, Config, RawClient, IntoOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let inclusive_range1 = "TiDB"..="TiKV";
    /// let inclusive_range2 = "TiKV"..="TiSpark";
    /// let iterable = vec![inclusive_range1.into_owned(), inclusive_range2.into_owned()];
    /// let req = client.batch_scan(iterable, 2);
    /// let result = req.await;
    /// # });
    /// ```
    pub async fn batch_scan(
        &self,
        ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
        each_limit: u32,
    ) -> Result<Vec<KvPair>> {
        debug!("invoking raw batch_scan request");
        self.batch_scan_inner(ranges, each_limit, false).await
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
    /// # use tikv_client::{Key, Config, RawClient, IntoOwnedRange};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let client = RawClient::new(vec!["192.168.0.100"]).await.unwrap();
    /// let inclusive_range1 = "TiDB"..="TiKV";
    /// let inclusive_range2 = "TiKV"..="TiSpark";
    /// let iterable = vec![inclusive_range1.into_owned(), inclusive_range2.into_owned()];
    /// let req = client.batch_scan(iterable, 2);
    /// let result = req.await;
    /// # });
    /// ```
    pub async fn batch_scan_keys(
        &self,
        ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
        each_limit: u32,
    ) -> Result<Vec<Key>> {
        debug!("invoking raw batch_scan_keys request");
        Ok(self
            .batch_scan_inner(ranges, each_limit, true)
            .await?
            .into_iter()
            .map(KvPair::into_key)
            .collect())
    }

    /// Create a new *atomic* 'compare and set' request.
    ///
    /// Once resolved this request will result in an atomic `compare and set'
    /// operation for the given key.
    ///
    /// If the value retrived is equal to `current_value`, `new_value` is
    /// written.
    ///
    /// # Return Value
    ///
    /// A tuple is returned if successful: the previous value and whether the
    /// value is swapped
    pub async fn compare_and_swap(
        &self,
        key: impl Into<Key>,
        previous_value: impl Into<Option<Value>>,
        new_value: impl Into<Value>,
    ) -> Result<(Option<Value>, bool)> {
        debug!("invoking raw compare_and_swap request");
        self.assert_atomic()?;
        let key = key.into().encode_keyspace(self.keyspace, KeyMode::Raw);
        let req = new_cas_request(
            key,
            new_value.into(),
            previous_value.into(),
            self.cf.clone(),
        );
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, req)
            // CAS is not replay-safe once the outcome is unknown: if the first attempt
            // applied, a retry would compare against the swapped-in value and report
            // `succeed = false` for a write that HAPPENED. Terminal on first sight —
            // and the outcome is unknown BOTH when the server says so
            // (errorpb.UndeterminedResult) and when the dispatch fails after the
            // request may have been sent (a gRPC error), so both are terminal here.
            .retry_multi_region_terminal_on_ambiguous_outcome(self.backoff.clone())
            .merge(CollectSingle)
            .post_process_default()
            .plan();
        plan.execute().await
    }

    pub async fn coprocessor(
        &self,
        copr_name: impl Into<String>,
        copr_version_req: impl Into<String>,
        ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
        request_builder: impl Fn(metapb::Region, Vec<Range<Key>>) -> Vec<u8> + Send + Sync + 'static,
    ) -> Result<Vec<(Vec<Range<Key>>, Vec<u8>)>> {
        let copr_version_req = copr_version_req.into();
        semver::VersionReq::from_str(&copr_version_req)?;
        let ranges = ranges
            .into_iter()
            .map(|range| range.into().encode_keyspace(self.keyspace, KeyMode::Raw));
        let keyspace = self.keyspace;
        let request_builder = move |region, ranges: Vec<Range<Key>>| {
            request_builder(
                region,
                ranges
                    .into_iter()
                    .map(|range| range.truncate_keyspace(keyspace))
                    .collect(),
            )
        };
        let req = new_raw_coprocessor_request(
            copr_name.into(),
            copr_version_req,
            ranges,
            request_builder,
        );
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, req)
            .preserve_shard()
            .retry_multi_region(self.backoff.clone())
            .post_process_default()
            .plan();
        Ok(plan
            .execute()
            .await?
            .into_iter()
            .map(|(ranges, data)| (ranges.truncate_keyspace(keyspace), data))
            .collect())
    }

    async fn scan_inner(
        &self,
        range: impl Into<BoundRange>,
        limit: u32,
        key_only: bool,
        reverse: bool,
    ) -> Result<Vec<KvPair>> {
        if limit > MAX_RAW_KV_SCAN_LIMIT {
            return Err(Error::MaxScanLimitExceeded {
                limit,
                max_limit: MAX_RAW_KV_SCAN_LIMIT,
            });
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let backoff = DEFAULT_STORE_BACKOFF;
        let range = range.into().encode_keyspace(self.keyspace, KeyMode::Raw);
        let mut result = Vec::new();
        let mut current_limit = limit;
        let (range_start, range_end) = range.into_keys();
        // `encode_keyspace` represents an unbounded upper bound as an empty key
        // when keyspace prefixes are disabled. Keyspace-enabled ranges use the
        // next keyspace prefix instead, so their encoded upper bound is non-empty.
        let mut reverse_unbounded = reverse
            && (range_end.is_none()
                || (range_end.as_ref().is_some_and(Key::is_empty)
                    && !matches!(self.keyspace, Keyspace::Enable { .. })));
        let mut current_key = if reverse && !reverse_unbounded {
            range_end.clone().unwrap()
        } else {
            range_start.clone()
        };
        let mut initial_region = if reverse_unbounded {
            Some(self.rpc.last_region_for_key(&range_start).await?)
        } else {
            None
        };

        while current_limit > 0 {
            let scan_args = ScanInnerArgs {
                current_key: current_key.clone(),
                range_start: range_start.clone(),
                range_end: range_end.clone(),
                reverse_unbounded,
                initial_region: initial_region.take(),
                limit: current_limit,
                key_only,
                reverse,
                backoff: backoff.clone(),
            };
            let (res, next_key) = self.retryable_scan(scan_args).await?;

            let mut kvs = res
                .map(|r| r.kvs.into_iter().map(Into::into).collect::<Vec<KvPair>>())
                .unwrap_or(Vec::new());

            if !kvs.is_empty() {
                current_limit -= kvs.len() as u32;
                result.append(&mut kvs);
            }
            if reverse && !reverse_unbounded {
                if next_key.is_empty() || next_key <= range_start {
                    break;
                }
            } else if reverse_unbounded {
                if next_key.is_empty() || next_key <= range_start {
                    break;
                }
                // The first request used the final region with an unbounded
                // upper edge. Subsequent requests walk bounded regions back
                // toward the lower edge just like an ordinary reverse scan.
                reverse_unbounded = false;
            } else if next_key.is_empty() || range_end.clone().is_some_and(|ek| ek <= next_key) {
                break;
            }
            current_key = next_key;
        }

        // limit is a soft limit, so we need check the number of results
        result.truncate(limit as usize);

        // truncate the prefix of keys
        let result = result.truncate_keyspace(self.keyspace);

        Ok(result)
    }

    async fn retryable_scan(
        &self,
        mut scan_args: ScanInnerArgs,
    ) -> Result<(Option<RawScanResponse>, Key)> {
        let current_key = scan_args.current_key;
        let mut region_override = scan_args.initial_region.take();
        let mut boundary_retries = 0;
        loop {
            let region = if let Some(region) = region_override.take() {
                region
            } else if scan_args.reverse && !scan_args.reverse_unbounded {
                let region = self.rpc.clone().region_for_key(&current_key).await?;
                if !current_key.is_empty() && region.start_key() == current_key {
                    let previous = match self.rpc.clone().prev_region_for_key(&current_key).await {
                        Ok(previous) if previous.end_key() == current_key => {
                            boundary_retries = 0;
                            previous
                        }
                        Ok(_) | Err(Error::RegionForKeyNotFound { .. }) => {
                            // The cached containing region is stale (for example,
                            // after a merge), so the predecessor no longer ends
                            // at this cursor. Drop it and resolve the cursor again.
                            if boundary_retries >= MAX_REVERSE_SCAN_BOUNDARY_RETRIES {
                                return Err(Error::InternalError {
                                    message: "failed to resolve the predecessor region after exhausting boundary retries"
                                        .to_owned(),
                                });
                            }
                            boundary_retries += 1;
                            self.rpc.invalidate_region_cache(region.ver_id()).await;
                            continue;
                        }
                        Err(error) => return Err(error),
                    };
                    previous
                } else {
                    boundary_retries = 0;
                    region
                }
            } else {
                boundary_retries = 0;
                self.rpc.clone().region_for_key(&current_key).await?
            };
            let store = self.rpc.clone().store_for_id(region.id()).await?;
            let (start_key, end_key) = if scan_args.reverse {
                let region_start = region.start_key();
                let start_key = if region_start < scan_args.range_start {
                    scan_args.range_start.clone()
                } else {
                    region_start
                };
                if scan_args.reverse_unbounded {
                    (start_key, None)
                } else {
                    (start_key, Some(current_key.clone()))
                }
            } else {
                (current_key.clone(), scan_args.range_end.clone())
            };
            let request = new_raw_scan_request(
                (start_key.clone(), end_key.clone()).into(),
                scan_args.limit,
                scan_args.key_only,
                scan_args.reverse,
                self.cf.clone(),
            );
            let resp = self.do_store_scan(store.clone(), request.clone()).await;
            return match resp {
                Ok(mut r) => {
                    if let Some(err) = r.region_error() {
                        // Resolved Region errors still consume the retry budget, even
                        // when a leader correction lets us retry without sleeping.
                        let Some(duration) = scan_args.backoff.next_delay_duration() else {
                            return Err(RegionError(Box::new(err)));
                        };
                        let status =
                            plan::handle_region_error(self.rpc.clone(), err.clone(), store.clone())
                                .await?;
                        if scan_args.reverse {
                            // Only a known-leader correction preserves the
                            // Region metadata. EpochNotMatch may have removed
                            // this Region ID, so resolve it by key again.
                            if status && err.not_leader.as_ref().is_some_and(|e| e.leader.is_some())
                            {
                                region_override = Some(region.clone());
                            } else if scan_args.reverse_unbounded {
                                region_override = Some(
                                    self.rpc.last_region_for_key(&scan_args.range_start).await?,
                                );
                            }
                        }
                        if !status {
                            sleep(duration).await;
                        }
                        continue;
                    }
                    let next_key = if scan_args.reverse {
                        region.start_key()
                    } else {
                        region.end_key()
                    };
                    Ok((Some(r), next_key))
                }
                Err(err) => Err(err),
            };
        }
    }

    async fn do_store_scan(
        &self,
        store: RegionStore,
        scan_request: RawScanRequest,
    ) -> Result<RawScanResponse> {
        crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, scan_request)
            .single_region_with_store(store.clone())
            .await?
            .plan()
            .execute()
            .await
    }

    async fn batch_scan_inner(
        &self,
        ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
        each_limit: u32,
        key_only: bool,
    ) -> Result<Vec<KvPair>> {
        if each_limit > MAX_RAW_KV_SCAN_LIMIT {
            return Err(Error::MaxScanLimitExceeded {
                limit: each_limit,
                max_limit: MAX_RAW_KV_SCAN_LIMIT,
            });
        }

        let ranges = ranges
            .into_iter()
            .map(|range| range.into().encode_keyspace(self.keyspace, KeyMode::Raw));

        let request = new_raw_batch_scan_request(ranges, each_limit, key_only, self.cf.clone());
        let plan = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
            .retry_multi_region(self.backoff.clone())
            .merge(Collect)
            .plan();
        plan.execute().await.map(|r| {
            r.into_iter()
                .map(|pair| pair.truncate_keyspace(self.keyspace))
                .collect()
        })
    }

    fn assert_non_atomic(&self) -> Result<()> {
        if !self.atomic {
            Ok(())
        } else {
            Err(Error::UnsupportedMode)
        }
    }

    fn assert_atomic(&self) -> Result<()> {
        if self.atomic {
            Ok(())
        } else {
            Err(Error::UnsupportedMode)
        }
    }
}

#[derive(Clone)]
struct ScanInnerArgs {
    current_key: Key,
    range_start: Key,
    range_end: Option<Key>,
    reverse_unbounded: bool,
    initial_region: Option<crate::region::RegionWithLeader>,
    limit: u32,
    key_only: bool,
    reverse: bool,
    backoff: Backoff,
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::sync::Mutex;

    use super::*;
    use crate::mock::MockKvClient;
    use crate::mock::MockPdClient;
    use crate::proto::errorpb;
    use crate::proto::kvrpcpb;
    use crate::Result;

    #[tokio::test]
    async fn reverse_scan_across_regions_uses_previous_regions() -> Result<()> {
        let keys = Arc::new(vec![vec![1], vec![11], vec![251]]);
        let requests = Arc::new(Mutex::new(Vec::<(Vec<u8>, Vec<u8>)>::new()));
        let dispatched_keys = keys.clone();
        let dispatched_requests = requests.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                let req = req.downcast_ref::<kvrpcpb::RawScanRequest>().unwrap();
                assert!(req.reverse);
                dispatched_requests
                    .lock()
                    .unwrap()
                    .push((req.start_key.clone(), req.end_key.clone()));

                let kvs = dispatched_keys
                    .iter()
                    .filter(|key| key.as_slice() >= req.end_key.as_slice())
                    .filter(|key| key.as_slice() < req.start_key.as_slice())
                    .rev()
                    .map(|key| kvrpcpb::KvPair {
                        key: key.clone(),
                        value: key.clone(),
                        ..Default::default()
                    })
                    .collect();
                Ok(Box::new(kvrpcpb::RawScanResponse {
                    kvs,
                    ..Default::default()
                }) as Box<dyn Any>)
            },
        )));
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace: Keyspace::Disable,
        };

        let pairs = client.scan_reverse(vec![1]..vec![255], 2).await?;
        assert_eq!(
            pairs.into_iter().map(|pair| pair.0).collect::<Vec<_>>(),
            vec![Key::from(vec![251]), Key::from(vec![11])]
        );
        assert_eq!(
            requests.lock().unwrap().as_slice(),
            &[(vec![255], vec![250, 250]), (vec![250, 250], vec![10]),]
        );

        requests.lock().unwrap().clear();
        let pairs = client.scan_reverse(vec![1]..vec![250, 250], 2).await?;
        assert_eq!(
            pairs.into_iter().map(|pair| pair.0).collect::<Vec<_>>(),
            vec![Key::from(vec![11]), Key::from(vec![1])]
        );
        assert_eq!(
            requests.lock().unwrap().as_slice(),
            &[(vec![250, 250], vec![10]), (vec![10], vec![1]),]
        );
        Ok(())
    }

    #[tokio::test]
    async fn reverse_scan_from_unbounded_upper_bound_uses_range_start() -> Result<()> {
        let looked_up_keys = Arc::new(Mutex::new(Vec::<Key>::new()));
        let dispatched_lookups = looked_up_keys.clone();
        let pd_client = Arc::new(
            MockPdClient::new(MockKvClient::with_dispatch_hook(move |req: &dyn Any| {
                let req = req.downcast_ref::<kvrpcpb::RawScanRequest>().unwrap();
                assert!(req.reverse);
                assert!(req.start_key.is_empty());
                assert_eq!(req.end_key, vec![251]);
                Ok(Box::new(kvrpcpb::RawScanResponse {
                    kvs: vec![kvrpcpb::KvPair {
                        key: vec![251],
                        value: vec![42],
                        ..Default::default()
                    }],
                    ..Default::default()
                }) as Box<dyn Any>)
            }))
            .with_region_for_key_hook(move |key| {
                dispatched_lookups.lock().unwrap().push(key.clone());
                Ok(MockPdClient::region3())
            }),
        );
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace: Keyspace::Disable,
        };

        assert_eq!(client.scan_reverse(vec![251].., 1).await?.len(), 1);
        assert_eq!(
            looked_up_keys.lock().unwrap().as_slice(),
            &[Key::from(vec![251])]
        );
        Ok(())
    }

    #[tokio::test]
    async fn reverse_scan_with_zero_limit_does_not_resolve_a_region() -> Result<()> {
        let lookups = Arc::new(AtomicUsize::new(0));
        let lookup_count = lookups.clone();
        let pd_client = Arc::new(
            MockPdClient::new(MockKvClient::with_dispatch_hook(|_: &dyn Any| {
                panic!("zero-limit scan must not dispatch")
            }))
            .with_region_for_key_hook(move |_| {
                lookup_count.fetch_add(1, Ordering::SeqCst);
                panic!("zero-limit scan must not resolve a region")
            }),
        );
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace: Keyspace::Disable,
        };

        assert!(client.scan_reverse(vec![1].., 0).await?.is_empty());
        assert_eq!(lookups.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test]
    async fn reverse_scan_stops_after_repeated_stale_boundaries() -> Result<()> {
        let pd_client = Arc::new(
            MockPdClient::new(MockKvClient::with_dispatch_hook(|_: &dyn Any| {
                panic!("stale boundary must not dispatch")
            }))
            .with_region_for_key_hook(|_| Ok(MockPdClient::region2()))
            .with_prev_region_for_key_hook(|_| Ok(MockPdClient::region2())),
        );
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: Backoff::no_backoff(),
            atomic: false,
            keyspace: Keyspace::Disable,
        };

        assert!(matches!(
            client.scan_reverse(vec![1]..vec![10], 1).await,
            Err(Error::InternalError { .. })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn reverse_scan_stops_after_repeated_final_region_boundaries() -> Result<()> {
        let pd_client = Arc::new(
            MockPdClient::new(MockKvClient::with_dispatch_hook(|_: &dyn Any| {
                panic!("stale boundary must not dispatch")
            }))
            .with_region_for_key_hook(|key| {
                let key_bytes: &[u8] = key.into();
                if key_bytes == [1] {
                    Ok(MockPdClient::region1())
                } else {
                    Ok(MockPdClient::region3())
                }
            }),
        );
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: Backoff::no_backoff(),
            atomic: false,
            keyspace: Keyspace::Disable,
        };

        assert!(matches!(
            client.scan_reverse(vec![1].., 1).await,
            Err(Error::InternalError { .. })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn reverse_scan_unbounded_upper_traverses_from_final_region() -> Result<()> {
        let requests = Arc::new(Mutex::new(Vec::<(Vec<u8>, Vec<u8>)>::new()));
        let dispatched_requests = requests.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                let req = req.downcast_ref::<kvrpcpb::RawScanRequest>().unwrap();
                assert!(req.reverse);
                dispatched_requests
                    .lock()
                    .unwrap()
                    .push((req.start_key.clone(), req.end_key.clone()));
                let key = match (req.start_key.as_slice(), req.end_key.as_slice()) {
                    ([], [250, 250]) => vec![251],
                    ([250, 250], [10]) => vec![11],
                    request => panic!("unexpected reverse scan request: {request:?}"),
                };
                Ok(Box::new(kvrpcpb::RawScanResponse {
                    kvs: vec![kvrpcpb::KvPair {
                        key: key.clone(),
                        value: key,
                        ..Default::default()
                    }],
                    ..Default::default()
                }) as Box<dyn Any>)
            },
        )));
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace: Keyspace::Disable,
        };

        assert_eq!(
            client
                .scan_reverse(vec![1].., 2)
                .await?
                .into_iter()
                .map(|pair| pair.0)
                .collect::<Vec<_>>(),
            vec![Key::from(vec![251]), Key::from(vec![11])]
        );
        assert_eq!(
            requests.lock().unwrap().as_slice(),
            &[(vec![], vec![250, 250]), (vec![250, 250], vec![10])]
        );
        Ok(())
    }

    #[tokio::test]
    async fn reverse_scan_refreshes_stale_region_boundary() -> Result<()> {
        let refreshed = Arc::new(AtomicBool::new(false));
        let invalidations = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::<(Vec<u8>, Vec<u8>)>::new()));
        let region_refreshed = refreshed.clone();
        let invalidation_count = invalidations.clone();
        let dispatched_requests = requests.clone();
        let stale_region = MockPdClient::region3();
        let mut merged_region = MockPdClient::region3();
        merged_region.region.start_key = vec![10];
        merged_region.region.region_epoch.as_mut().unwrap().version = 1;
        let merged_region_for_lookup = merged_region.clone();
        let pd_client = Arc::new(
            MockPdClient::new(MockKvClient::with_dispatch_hook(move |req: &dyn Any| {
                let req = req.downcast_ref::<kvrpcpb::RawScanRequest>().unwrap();
                dispatched_requests
                    .lock()
                    .unwrap()
                    .push((req.start_key.clone(), req.end_key.clone()));
                Ok(Box::new(kvrpcpb::RawScanResponse::default()) as Box<dyn Any>)
            }))
            .with_region_for_key_hook(move |key| {
                if region_refreshed.load(Ordering::SeqCst) {
                    let key_bytes: &[u8] = key.into();
                    if key_bytes >= [10].as_slice() {
                        Ok(merged_region_for_lookup.clone())
                    } else {
                        Ok(MockPdClient::region1())
                    }
                } else {
                    Ok(stale_region.clone())
                }
            })
            .with_prev_region_for_key_hook(|_| Ok(MockPdClient::region1()))
            .with_invalidate_region_hook(move |_| {
                invalidation_count.fetch_add(1, Ordering::SeqCst);
                refreshed.store(true, Ordering::SeqCst);
            }),
        );
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace: Keyspace::Disable,
        };

        client.scan_reverse(vec![1]..vec![250, 250], 1).await?;
        assert_eq!(invalidations.load(Ordering::SeqCst), 1);
        assert_eq!(
            requests.lock().unwrap().as_slice(),
            &[(vec![250, 250], vec![10]), (vec![10], vec![1])]
        );
        Ok(())
    }

    #[tokio::test]
    async fn reverse_scan_reloads_region_after_epoch_not_match() -> Result<()> {
        let refreshed = Arc::new(AtomicBool::new(false));
        let dispatch_count = Arc::new(AtomicUsize::new(0));
        let region_refreshed = refreshed.clone();
        let request_count = dispatch_count.clone();
        let region_id_lookups = Arc::new(AtomicUsize::new(0));
        let region_id_lookup_count = region_id_lookups.clone();
        let mut stale_region = MockPdClient::region3();
        stale_region.region.region_epoch.as_mut().unwrap().version = 0;
        let mut refreshed_region = stale_region.clone();
        refreshed_region.region.id = 4;
        refreshed_region.region.start_key = vec![10];
        refreshed_region.region.end_key = vec![];
        refreshed_region
            .region
            .region_epoch
            .as_mut()
            .unwrap()
            .version = 1;
        let refreshed_for_lookup = refreshed_region.clone();
        let refreshed_for_id = refreshed_region.clone();
        let stale_for_id = stale_region.clone();
        let pd_client = Arc::new(
            MockPdClient::new(MockKvClient::with_dispatch_hook(move |req: &dyn Any| {
                let req = req.downcast_ref::<kvrpcpb::RawScanRequest>().unwrap();
                assert!(req.reverse);
                if request_count.fetch_add(1, Ordering::SeqCst) == 0 {
                    Ok(Box::new(kvrpcpb::RawScanResponse {
                        region_error: Some(errorpb::Error {
                            epoch_not_match: Some(errorpb::EpochNotMatch {
                                current_regions: vec![],
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }) as Box<dyn Any>)
                } else {
                    assert_eq!(req.end_key, vec![10]);
                    Ok(Box::new(kvrpcpb::RawScanResponse {
                        kvs: vec![kvrpcpb::KvPair {
                            key: vec![11],
                            value: vec![42],
                            ..Default::default()
                        }],
                        ..Default::default()
                    }) as Box<dyn Any>)
                }
            }))
            .with_region_for_key_hook(move |_| {
                if region_refreshed.load(Ordering::SeqCst) {
                    Ok(refreshed_for_lookup.clone())
                } else {
                    Ok(stale_region.clone())
                }
            })
            .with_region_for_id_hook(move |id| {
                if id == refreshed_for_id.id() {
                    Ok(refreshed_for_id.clone())
                } else if id == stale_for_id.id()
                    && region_id_lookup_count.fetch_add(1, Ordering::SeqCst) == 0
                {
                    Ok(stale_for_id.clone())
                } else {
                    Err(Error::RegionNotFoundInResponse { region_id: id })
                }
            })
            .with_invalidate_region_hook(move |_| {
                refreshed.store(true, Ordering::SeqCst);
            }),
        );
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace: Keyspace::Disable,
        };

        assert_eq!(client.scan_reverse(vec![1]..vec![255], 1).await?.len(), 1);
        assert_eq!(dispatch_count.load(Ordering::SeqCst), 2);
        Ok(())
    }

    #[tokio::test]
    async fn reverse_scan_bounds_immediate_region_error_retries() -> Result<()> {
        let dispatch_count = Arc::new(AtomicUsize::new(0));
        let requests = dispatch_count.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                assert!(req.downcast_ref::<kvrpcpb::RawScanRequest>().is_some());
                assert!(
                    requests.fetch_add(1, Ordering::SeqCst) < 3,
                    "retry limit exceeded"
                );
                Ok(Box::new(kvrpcpb::RawScanResponse {
                    region_error: Some(errorpb::Error {
                        epoch_not_match: Some(errorpb::EpochNotMatch {
                            current_regions: vec![],
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }) as Box<dyn Any>)
            },
        )));
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace: Keyspace::Disable,
        };

        let result = client
            .retryable_scan(ScanInnerArgs {
                current_key: vec![1].into(),
                range_start: vec![1].into(),
                range_end: None,
                reverse_unbounded: true,
                initial_region: Some(MockPdClient::region3()),
                limit: 1,
                key_only: false,
                reverse: true,
                backoff: Backoff::no_jitter_backoff(0, 0, 2),
            })
            .await;
        assert!(
            matches!(result, Err(Error::RegionError(error)) if error.epoch_not_match.is_some())
        );
        assert_eq!(dispatch_count.load(Ordering::SeqCst), 3);
        Ok(())
    }

    #[rstest::rstest]
    #[case::forward(false)]
    #[case::reverse(true)]
    #[tokio::test]
    async fn scan_with_keyspace_stops_after_final_region(#[case] reverse: bool) -> Result<()> {
        let request_count = Arc::new(AtomicUsize::new(0));
        let dispatched = request_count.clone();
        let keyspace = Keyspace::Enable { keyspace_id: 0 };
        // An open-ended keyspace scan has an encoded upper bound, but the final
        // region has an empty end key.
        let encoded_key: Vec<u8> = Key::from(vec![251])
            .encode_keyspace(keyspace, KeyMode::Raw)
            .into();
        let mut final_region = MockPdClient::region3();
        final_region.region.start_key.clear();
        let pd_client = Arc::new(
            MockPdClient::new(MockKvClient::with_dispatch_hook(move |req: &dyn Any| {
                let req = req.downcast_ref::<kvrpcpb::RawScanRequest>().unwrap();
                assert_eq!(
                    dispatched.fetch_add(1, Ordering::SeqCst),
                    0,
                    "scan issued another request"
                );
                assert_eq!(req.reverse, reverse);
                if reverse {
                    assert!(!req.start_key.is_empty());
                    assert_eq!(req.end_key, encoded_key);
                } else {
                    assert_eq!(req.start_key, encoded_key);
                    assert!(!req.end_key.is_empty());
                }
                Ok(Box::new(kvrpcpb::RawScanResponse {
                    kvs: vec![kvrpcpb::KvPair {
                        key: encoded_key.clone(),
                        value: vec![42],
                        ..Default::default()
                    }],
                    ..Default::default()
                }) as Box<dyn Any>)
            }))
            .with_region_for_key_hook(move |_| Ok(final_region.clone())),
        );
        let client = Client {
            rpc: pd_client,
            cf: None,
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace,
        };

        let pairs = if reverse {
            client.scan_reverse(vec![251].., 2).await?
        } else {
            client.scan(vec![251].., 2).await?
        };
        assert_eq!(pairs, vec![KvPair(vec![251].into(), vec![42])]);
        assert_eq!(request_count.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_batch_put_with_ttl() -> Result<()> {
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if req.downcast_ref::<kvrpcpb::RawBatchPutRequest>().is_some() {
                    let resp = kvrpcpb::RawBatchPutResponse {
                        ..Default::default()
                    };
                    Ok(Box::new(resp) as Box<dyn Any>)
                } else {
                    unreachable!()
                }
            },
        )));
        let client = Client {
            rpc: pd_client,
            cf: Some(ColumnFamily::Default),
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace: Keyspace::Enable { keyspace_id: 0 },
        };
        let pairs = vec![
            KvPair(vec![11].into(), vec![12]),
            KvPair(vec![11].into(), vec![12]),
        ];
        let ttls = vec![0, 0];
        assert!(client.batch_put_with_ttl(pairs, ttls).await.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_raw_coprocessor() -> Result<()> {
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if let Some(req) = req.downcast_ref::<kvrpcpb::RawCoprocessorRequest>() {
                    assert_eq!(req.copr_name, "example");
                    assert_eq!(req.copr_version_req, "0.1.0");
                    let resp = kvrpcpb::RawCoprocessorResponse {
                        data: req.data.clone(),
                        ..Default::default()
                    };
                    Ok(Box::new(resp) as Box<dyn Any>)
                } else {
                    unreachable!()
                }
            },
        )));
        let client = Client {
            rpc: pd_client,
            cf: Some(ColumnFamily::Default),
            backoff: DEFAULT_REGION_BACKOFF,
            atomic: false,
            keyspace: Keyspace::Enable { keyspace_id: 0 },
        };
        let resps = client
            .coprocessor(
                "example",
                "0.1.0",
                vec![vec![5]..vec![15], vec![20]..vec![]],
                |region, ranges| format!("{:?}:{:?}", region.id, ranges).into_bytes(),
            )
            .await?;
        let resps: Vec<_> = resps
            .into_iter()
            .map(|(ranges, data)| (ranges, String::from_utf8(data).unwrap()))
            .collect();
        assert_eq!(
            resps,
            vec![(
                vec![
                    Key::from(vec![5])..Key::from(vec![15]),
                    Key::from(vec![20])..Key::from(vec![])
                ],
                "2:[Key(05)..Key(0F), Key(14)..Key()]".to_string(),
            ),]
        );
        Ok(())
    }
}
