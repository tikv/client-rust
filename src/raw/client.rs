// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use core::ops::Range;
use std::str::FromStr;
use std::sync::Arc;
use std::u32;

use futures::StreamExt;
use log::debug;

use crate::backoff::DEFAULT_REGION_BACKOFF;
use crate::common::Error;
use crate::config::Config;
use crate::pd::PdClient;
use crate::pd::PdRpcClient;
use crate::proto::metapb;
use crate::raw::lowering::*;
use crate::request::Collect;
use crate::request::CollectSingle;
use crate::request::EncodeKeyspace;
use crate::request::KeyMode;
use crate::request::Keyspace;
use crate::request::Plan;
use crate::request::TruncateKeyspace;
use crate::Backoff;
use crate::BoundRange;
use crate::ColumnFamily;
use crate::Key;
use crate::KvPair;
use crate::Result;
use crate::Value;

const MAX_RAW_KV_SCAN_LIMIT: u32 = 10240;

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
            Some(keyspace) => {
                let keyspace_id = rpc.get_keyspace_id(&keyspace).await?;
                Keyspace::Enable { keyspace_id }
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
        debug!("invoking raw put request");
        let key = key.into().encode_keyspace(self.keyspace, KeyMode::Raw);
        let request = new_raw_put_request(key, value.into(), self.cf.clone(), self.atomic);
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
        debug!("invoking raw batch_put request");
        let pairs = pairs
            .into_iter()
            .map(|pair| pair.into().encode_keyspace(self.keyspace, KeyMode::Raw));
        let request = new_raw_batch_put_request(pairs, self.cf.clone(), self.atomic);
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
        self.scan_inner(range.into(), limit, false).await
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
            .scan_inner(range, limit, true)
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
            .retry_multi_region(self.backoff.clone())
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
    ) -> Result<Vec<KvPair>> {
        if limit > MAX_RAW_KV_SCAN_LIMIT {
            return Err(Error::MaxScanLimitExceeded {
                limit,
                max_limit: MAX_RAW_KV_SCAN_LIMIT,
            });
        }

        let mut cur_range = range.into().encode_keyspace(self.keyspace, KeyMode::Raw);
        let mut result = Vec::new();
        let mut scan_regions = self.rpc.clone().stores_for_range(cur_range.clone()).boxed();
        let mut region_store =
            scan_regions
                .next()
                .await
                .ok_or(Error::RegionForRangeNotFound {
                    range: (cur_range.clone()),
                })??;
        let mut cur_limit = limit;

        while cur_limit > 0 {
            let request =
                new_raw_scan_request(cur_range.clone(), cur_limit, key_only, self.cf.clone());
            let resp = crate::request::PlanBuilder::new(self.rpc.clone(), self.keyspace, request)
                .single_region_with_store(region_store.clone())
                .await?
                .plan()
                .execute()
                .await?;
            let mut region_scan_res = resp
                .kvs
                .into_iter()
                .map(Into::into)
                .collect::<Vec<KvPair>>();
            let res_len = region_scan_res.len();
            result.append(&mut region_scan_res);

            // if the number of results is less than cur_limit, it means this scan range contains more than one region, so we need to scan next region
            if res_len < cur_limit as usize {
                region_store = match scan_regions.next().await {
                    Some(Ok(rs)) => {
                        cur_range = BoundRange::new(
                            std::ops::Bound::Included(region_store.region_with_leader.range().1),
                            cur_range.to,
                        );
                        rs
                    }
                    Some(Err(e)) => return Err(e),
                    None => break,
                };
                cur_limit -= res_len as u32;
            } else {
                break;
            }
        }

        // limit is a soft limit, so we need check the number of results
        result.truncate(limit as usize);

        // truncate the prefix of keys
        let result = result.truncate_keyspace(self.keyspace);

        Ok(result)
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

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::Arc;

    use super::*;
    use crate::mock::MockKvClient;
    use crate::mock::MockPdClient;
    use crate::proto::kvrpcpb;
    use crate::Result;

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
