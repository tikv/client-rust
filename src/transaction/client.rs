// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::{future::Future, marker::PhantomData, mem, sync::Arc};

use slog::Logger;

use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

use crate::{
    backoff::{DEFAULT_REGION_BACKOFF, OPTIMISTIC_BACKOFF},
    config::Config,
    pd::{PdClient, PdRpcClient, RetryClient},
    request::{codec::TxnCodec, Plan},
    timestamp::TimestampExt,
    transaction::{ApiV2, Snapshot, Transaction, TransactionOptions},
    util::client::ClientContext,
    Result,
};

use super::{requests::new_scan_lock_request, resolve_locks};

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
pub struct Client<C: TxnCodec> {
    pd: Arc<PdRpcClient<C>>,
    logger: Logger,
    _phantom: PhantomData<C>,
}

impl<C: TxnCodec> Clone for Client<C> {
    fn clone(&self) -> Self {
        Self {
            pd: self.pd.clone(),
            logger: self.logger.clone(),
            _phantom: PhantomData,
        }
    }
}

impl Client<ApiV2> {
    /// Create a transaction [`Client`] and connect to the TiKV cluster, with `keyspace` name.
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
    /// use tikv_client::transaction::ApiV2;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::<ApiV2>::new_with_keyspace(
    ///     vec!["192.168.0.100"],
    ///     "space_for_txn",
    ///     Config::default(),
    ///     None,
    /// )
    /// .await
    /// .unwrap();
    /// # });
    /// ```
    pub async fn new_with_keyspace<S, K>(
        pd_endpoints: Vec<S>,
        keyspace: K,
        config: Config,
        optional_logger: Option<Logger>,
    ) -> Result<Self>
    where
        S: Into<String>,
        K: Into<String>,
    {
        let keyspace = &keyspace.into();
        let this = Self::new_with_codec_factory(
            pd_endpoints,
            config,
            async move |pd| ApiV2::with_keyspace(keyspace, pd).await,
            optional_logger,
        )
        .await?;

        info!(this.logger, "created transaction client"; "keyspace" => keyspace);

        Ok(this)
    }
}

impl<C> Client<C>
where
    C: TxnCodec,
{
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
    /// use tikv_client::transaction::ApiV1;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::<ApiV1>::new(vec!["192.168.0.100"], None)
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub async fn new<S: Into<String>>(
        pd_endpoints: Vec<S>,
        logger: Option<Logger>,
    ) -> Result<Client<C>> {
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
    /// use tikv_client::transaction::ApiV1;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::<ApiV1>::new_with_config(
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
    ) -> Result<Client<C>> {
        Self::new_with_codec_factory(
            pd_endpoints,
            config,
            async move |_| Ok(C::default()),
            optional_logger,
        )
        .await
    }

    pub async fn new_with_codec_factory<S, F, Fut>(
        pd_endpoints: Vec<S>,
        config: Config,
        codec_factory: F,
        optional_logger: Option<Logger>,
    ) -> Result<Client<C>>
    where
        S: Into<String>,
        F: Fn(Arc<RetryClient>) -> Fut,
        Fut: Future<Output = Result<C>>,
    {
        let ClientContext { pd, logger } =
            ClientContext::new(pd_endpoints, config, codec_factory, optional_logger).await?;

        Ok(Client {
            pd,
            logger,
            _phantom: PhantomData,
        })
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
    /// use tikv_client::transaction::ApiV1;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::<ApiV1>::new(vec!["192.168.0.100"], None)
    ///     .await
    ///     .unwrap();
    /// let mut transaction = client.begin_optimistic().await.unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn begin_optimistic(&self) -> Result<Transaction<C>> {
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
    /// use tikv_client::transaction::ApiV1;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::<ApiV1>::new(vec!["192.168.0.100"], None)
    ///     .await
    ///     .unwrap();
    /// let mut transaction = client.begin_pessimistic().await.unwrap();
    /// // ... Issue some commands.
    /// transaction.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn begin_pessimistic(&self) -> Result<Transaction<C>> {
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
    /// use tikv_client::transaction::ApiV1;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::<ApiV1>::new(vec!["192.168.0.100"], None)
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
    pub async fn begin_with_options(&self, options: TransactionOptions) -> Result<Transaction<C>> {
        debug!(self.logger, "creating new customized transaction");
        let timestamp = self.current_timestamp().await?;
        Ok(self.new_transaction(timestamp, options))
    }

    /// Create a new [`Snapshot`](Snapshot) at the given [`Timestamp`](Timestamp).
    pub fn snapshot(&self, timestamp: Timestamp, options: TransactionOptions) -> Snapshot<C> {
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
    /// use tikv_client::transaction::ApiV1;
    /// # futures::executor::block_on(async {
    /// let client = TransactionClient::<ApiV1>::new(vec!["192.168.0.100"], None)
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
        // scan all locks with ts <= safepoint
        let mut locks: Vec<kvrpcpb::LockInfo> = vec![];
        let mut start_key = vec![];
        loop {
            let req = new_scan_lock_request(
                mem::take(&mut start_key),
                safepoint.version(),
                SCAN_LOCK_BATCH_SIZE,
            );

            let plan = crate::request::PlanBuilder::new(self.pd.clone(), req)
                .resolve_lock(OPTIMISTIC_BACKOFF)
                .retry_multi_region(DEFAULT_REGION_BACKOFF)
                .merge(crate::request::Collect)
                .plan();
            let res: Vec<kvrpcpb::LockInfo> = plan.execute().await?;

            if res.is_empty() {
                break;
            }
            start_key = res.last().unwrap().key.clone();
            start_key.push(0);
            locks.extend(res);
        }

        // resolve locks
        // FIXME: (1) this is inefficient (2) when region error occurred
        resolve_locks(locks, self.pd.clone()).await?;

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

    fn new_transaction(&self, timestamp: Timestamp, options: TransactionOptions) -> Transaction<C> {
        let logger = self.logger.new(o!("child" => 1));
        Transaction::new(timestamp, self.pd.clone(), options, logger)
    }
}
