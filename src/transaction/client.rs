// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::requests::new_scan_lock_request;
use crate::{
    backoff::DEFAULT_REGION_BACKOFF,
    config::Config,
    pd::{PdClient, PdRpcClient},
    request::{plan::CleanupLocksResult, Plan},
    store::RegionStore,
    timestamp::TimestampExt,
    transaction::{
        lock::ResolveLocksOptions, requests, requests::new_unsafe_destroy_range_request,
        ResolveLocksContext, Snapshot, Transaction, TransactionOptions,
    },
    Backoff, BoundRange, Result,
};
use futures::{future::try_join_all, StreamExt};
use slog::{Drain, Logger};
use std::{collections::HashMap, mem, sync::Arc};
use tikv_client_common::Error;
use tikv_client_proto::{metapb::Region, pdpb::Timestamp};

// FIXME: cargo-culted value
const SCAN_LOCK_BATCH_SIZE: u32 = 1024;

/// The TiKV transactional `Client` is used to interact with TiKV using transactional requests.
///
/// Transactions support optimistic and pessimistic modes. For more details see the SIG-transaction
/// [docs](https://github.com/tikv/sig-transaction/tree/master/doc/tikv#optimistic-and-pessimistic-transactions).
///
/// Begin a [`Transaction`] by calling [`begin_optimistic`](Client::begin_optimistic) or
/// [`begin_pessimistic`](Client::begin_pessimistic). A transaction must be rolled back or committed.
///
/// Besides transactions, the client provides some further functionality:
/// - `gc`: trigger a GC process which clears stale data in the cluster.
/// - `current_timestamp`: get the current `Timestamp` from PD.
/// - `snapshot`: get a [`Snapshot`] of the database at a specified timestamp.
/// A `Snapshot` is a read-only transaction.
///
/// The returned results of transactional requests are [`Future`](std::future::Future)s that must be
/// awaited to execute.
pub struct Client {
    pd: Arc<PdRpcClient>,
    logger: Logger,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Self {
            pd: self.pd.clone(),
            logger: self.logger.clone(),
        }
    }
}

impl Client {
    /// Create a transactional [`Client`] and connect to the TiKV cluster.
    ///
    /// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for
    /// PD must be provided, not the TiKV nodes. It's important to include more than one PD endpoint
    /// (include all endpoints, if possible), this helps avoid having a single point of failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"], None)
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub async fn new<S: Into<String>>(
        pd_endpoints: Vec<S>,
        logger: Option<Logger>,
    ) -> Result<Client> {
        // debug!(self.logger, "creating transactional client");
        Self::new_with_config(pd_endpoints, Config::default(), logger).await
    }

    /// Create a transactional [`Client`] with a custom configuration, and connect to the TiKV cluster.
    ///
    /// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for
    /// PD must be provided, not the TiKV nodes. It's important to include more than one PD endpoint
    /// (include all endpoints, if possible), this helps avoid having a single point of failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # use std::time::Duration;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new_with_config(
    ///     vec!["192.168.0.100"],
    ///     Config::default().with_timeout(Duration::from_secs(60)),
    ///     None,
    /// )
    /// .await
    /// .unwrap();
    /// # });
    /// ```
    pub async fn new_with_config<S: Into<String>>(
        pd_endpoints: Vec<S>,
        config: Config,
        optional_logger: Option<Logger>,
    ) -> Result<Client> {
        let logger = optional_logger.unwrap_or_else(|| {
            let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
            Logger::root(
                slog_term::FullFormat::new(plain)
                    .build()
                    .filter_level(slog::Level::Info)
                    .fuse(),
                o!(),
            )
        });
        debug!(logger, "creating new transactional client");
        let pd_endpoints: Vec<String> = pd_endpoints.into_iter().map(Into::into).collect();
        let pd = Arc::new(PdRpcClient::connect(&pd_endpoints, config, true, logger.clone()).await?);
        Ok(Client { pd, logger })
    }

    /// Creates a new optimistic [`Transaction`].
    ///
    /// Use the transaction to issue requests like [`get`](Transaction::get) or
    /// [`put`](Transaction::put).
    ///
    /// Write operations do not lock data in TiKV, thus the commit request may fail due to a write
    /// conflict.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"], None)
    ///     .await
    ///     .unwrap();
    /// let mut transaction = client.begin_optimistic().await.unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn begin_optimistic(&self) -> Result<Transaction> {
        debug!(self.logger, "creating new optimistic transaction");
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, TransactionOptions::new_optimistic()))
    }

    /// Creates a new pessimistic [`Transaction`].
    ///
    /// Write operations will lock the data until committed, thus commit requests should not suffer
    /// from write conflicts.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"], None)
    ///     .await
    ///     .unwrap();
    /// let mut transaction = client.begin_pessimistic().await.unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn begin_pessimistic(&self) -> Result<Transaction> {
        debug!(self.logger, "creating new pessimistic transaction");
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, TransactionOptions::new_pessimistic()))
    }

    /// Create a new customized [`Transaction`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient, TransactionOptions};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"], None)
    ///     .await
    ///     .unwrap();
    /// let mut transaction = client
    ///     .begin_with_options(TransactionOptions::default().use_async_commit())
    ///     .await
    ///     .unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn begin_with_options(&self, options: TransactionOptions) -> Result<Transaction> {
        debug!(self.logger, "creating new customized transaction");
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, options))
    }

    /// Create a new [`Snapshot`](Snapshot) at the given [`Timestamp`](Timestamp).
    pub fn snapshot(&self, timestamp: Timestamp, options: TransactionOptions) -> Snapshot {
        debug!(self.logger, "creating new snapshot");
        let logger = self.logger.new(o!("child" => 1));
        Snapshot::new(self.new_transaction(timestamp, options.read_only()), logger)
    }

    /// Retrieve the current [`Timestamp`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::new(vec!["192.168.0.100"], None)
    ///     .await
    ///     .unwrap();
    /// let timestamp = client.current_timestamp().await.unwrap();
    /// # });
    /// ```
    pub async fn current_timestamp(&self) -> Result<Timestamp> {
        self.pd.clone().get_timestamp().await
    }

    /// Request garbage collection (GC) of the TiKV cluster.
    ///
    /// GC deletes MVCC records whose timestamp is lower than the given `safepoint`. We must guarantee
    ///  that all transactions started before this timestamp had committed. We can keep an active
    /// transaction list in application to decide which is the minimal start timestamp of them.
    ///
    /// For each key, the last mutation record (unless it's a deletion) before `safepoint` is retained.
    ///
    /// GC is performed by:
    /// 1. resolving all locks with timestamp <= `safepoint`
    /// 2. updating PD's known safepoint
    ///
    /// This is a simplified version of [GC in TiDB](https://docs.pingcap.com/tidb/stable/garbage-collection-overview).
    /// We skip the second step "delete ranges" which is an optimization for TiDB.
    pub async fn gc(&self, safepoint: Timestamp) -> Result<bool> {
        debug!(self.logger, "invoking transactional gc request");

        let options = ResolveLocksOptions {
            batch_size: SCAN_LOCK_BATCH_SIZE,
            ..Default::default()
        };
        self.cleanup_locks(&safepoint, options).await?;

        // update safepoint to PD
        let res: bool = self
            .pd
            .clone()
            .update_safepoint(safepoint.version())
            .await?;
        if !res {
            info!(self.logger, "new safepoint != user-specified safepoint");
        }
        Ok(res)
    }

    pub async fn cleanup_locks(
        &self,
        safepoint: &Timestamp,
        options: ResolveLocksOptions,
    ) -> Result<CleanupLocksResult> {
        debug!(self.logger, "invoking cleanup async commit locks");
        // scan all locks with ts <= safepoint
        let mut start_key = vec![];
        let ctx = ResolveLocksContext::default();
        let backoff = Backoff::equal_jitter_backoff(100, 10000, 50);
        let req = new_scan_lock_request(
            mem::take(&mut start_key),
            safepoint.version(),
            options.batch_size,
        );
        let plan = crate::request::PlanBuilder::new(self.pd.clone(), req)
            .cleanup_locks(self.logger.clone(), ctx.clone(), options, backoff)
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .merge(crate::request::Collect)
            .extract_error()
            .plan();
        plan.execute().await
    }

    // For test.
    // Note: `batch_size` must be >= expected number of locks.
    #[cfg(feature = "integration-tests")]
    pub async fn scan_locks(
        &self,
        safepoint: &Timestamp,
        mut start_key: Vec<u8>,
        batch_size: u32,
    ) -> Result<Vec<tikv_client_proto::kvrpcpb::LockInfo>> {
        let req = new_scan_lock_request(mem::take(&mut start_key), safepoint.version(), batch_size);
        let plan = crate::request::PlanBuilder::new(self.pd.clone(), req)
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .merge(crate::request::Collect)
            .plan();
        plan.execute().await
    }

    fn new_transaction(&self, timestamp: Timestamp, options: TransactionOptions) -> Transaction {
        let logger = self.logger.new(o!("child" => 1));
        Transaction::new(timestamp, self.pd.clone(), options, logger)
    }

    pub async fn split_region_with_retry(
        &self,
        #[allow(clippy::ptr_arg)] key: &Vec<u8>,
        split_keys: Vec<Vec<u8>>,
        is_raw_kv: bool,
    ) -> Result<Vec<Region>> {
        debug!(self.logger, "invoking split region with retry");
        let mut backoff = DEFAULT_REGION_BACKOFF;
        let mut i = 0;
        'retry: loop {
            i += 1;
            debug!(self.logger, "split region: attempt {}", i);
            let store = self.pd.clone().store_for_key(key.into()).await?;
            let request = requests::new_split_region_request(split_keys.clone(), is_raw_kv);
            let plan = crate::request::PlanBuilder::new(self.pd.clone(), request)
                .single_region_with_store(store)
                .await?
                .extract_error()
                .plan();
            match plan.execute().await {
                Ok(mut resp) => return Ok(resp.take_regions().into()),
                Err(Error::ExtractedErrors(mut errors)) => match errors.pop() {
                    Some(e @ Error::RegionError(_)) => match backoff.next_delay_duration() {
                        Some(duration) => {
                            futures_timer::Delay::new(duration).await;
                            continue 'retry;
                        }
                        None => return Err(e),
                    },
                    Some(e) => return Err(e),
                    None => unreachable!(),
                },
                Err(e) => return Err(e),
            }
        }
    }

    pub async fn unsafe_destroy_range(&self, start_key: Vec<u8>, end_key: Vec<u8>) -> Result<()> {
        debug!(self.logger, "invoking unsafe destroy range");
        let backoff = DEFAULT_REGION_BACKOFF;
        let stores = self
            .list_stores_for_unsafe_destroy(start_key.clone(), end_key.clone())
            .await?;
        let mut handles = Vec::with_capacity(stores.len());
        for store in stores.into_values() {
            let logger = self.logger.clone();
            let start_key = start_key.clone();
            let end_key = end_key.clone();
            let pd = self.pd.clone();
            let mut backoff = backoff.clone();
            let task = async move {
                let mut i = 0;
                'retry: loop {
                    i += 1;
                    debug!(logger, "unsafe destroy range: attempt {}", i);
                    let request =
                        new_unsafe_destroy_range_request(start_key.clone(), end_key.clone());
                    let plan = crate::request::PlanBuilder::new(pd.clone(), request)
                        .single_region_with_store(store.clone())
                        .await?
                        .extract_error()
                        .plan();
                    match plan.execute().await {
                        Ok(_) => return Ok(()),
                        Err(e) => {
                            warn!(logger, "unsafe destroy range error: {:?}", e);
                            match backoff.next_delay_duration() {
                                Some(duration) => {
                                    futures_timer::Delay::new(duration).await;
                                    continue 'retry;
                                }
                                None => return Err(e),
                            }
                        }
                    }
                }
            };
            handles.push(tokio::spawn(task));
        }

        let results = try_join_all(handles).await?;
        match results.into_iter().find(|x| x.is_err()) {
            Some(r) => r,
            None => Ok(()),
        }
    }

    async fn list_stores_for_unsafe_destroy(
        &self,
        start_key: Vec<u8>,
        end_key: Vec<u8>,
    ) -> Result<HashMap<u64, RegionStore>> {
        let mut stores = HashMap::new();
        let bnd_range = BoundRange::from((start_key, end_key));
        self.pd
            .clone()
            .stores_for_range(bnd_range)
            .map(|store| -> Result<()> {
                let store = store?;
                let store_id = store
                    .region_with_leader
                    .leader
                    .as_ref()
                    .unwrap()
                    .get_store_id();
                stores.entry(store_id).or_insert(store);
                Ok(())
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_>>()?;
        Ok(stores)
    }
}
