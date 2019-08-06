// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{Snapshot, Timestamp, Transaction};
use crate::rpc::RpcClient;
use crate::{Config, Result};

use derive_new::new;
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;
use std::sync::Arc;

/// The TiKV transactional `Client` is used to issue requests to the TiKV server and PD cluster.
pub struct Client {
    rpc: Arc<RpcClient>,
}

impl Client {
    /// Creates a new [`Client`](Client) once the [`Connect`](Connect) resolves.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = Client::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// # });
    /// ```
    pub fn connect(config: Config) -> Connect {
        Connect::new(config)
    }

    /// Creates a new [`Transaction`](Transaction).
    ///
    /// Using the transaction you can issue commands like [`get`](Transaction::get) or [`set`](Transaction::set).
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = Client::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// let mut transaction = client.begin().await.unwrap();
    /// // ... Issue some commands.
    /// let commit = transaction.commit();
    /// let result: () = commit.await.unwrap();
    /// # });
    /// ```
    pub async fn begin(&self) -> Result<Transaction> {
        let snapshot = self.snapshot().await?;
        Ok(Transaction::new(snapshot))
    }

    /// Gets the latest [`Snapshot`](Snapshot).
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = Client::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// let snapshot = client.snapshot().await.unwrap();
    /// // ... Issue some commands.
    /// # });
    /// ```
    pub async fn snapshot(&self) -> Result<Snapshot> {
        let timestamp = self.current_timestamp().await?;
        self.snapshot_at(timestamp).await
    }

    /// Gets a [`Snapshot`](Snapshot) at the given point in time.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// use tikv_client::{Config, transaction::{Client, Timestamp}};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = Client::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// let timestamp = Timestamp { physical: 1564481750172, logical: 1 };
    /// let snapshot = client.snapshot_at(timestamp);
    /// // ... Issue some commands.
    /// # });
    /// ```
    pub async fn snapshot_at(&self, timestamp: Timestamp) -> Result<Snapshot> {
        Ok(Snapshot::new(timestamp))
    }

    /// Retrieves the current [`Timestamp`](Timestamp).
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = Client::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// let timestamp = client.current_timestamp().await.unwrap();
    /// # });
    /// ```
    pub async fn current_timestamp(&self) -> Result<Timestamp> {
        self.rpc.clone().get_timestamp().await
    }
}

/// An unresolved [`Client`](Client) connection to a TiKV cluster.
///
/// Once resolved it will result in a connected [`Client`](Client).
///
/// ```rust,no_run
/// # #![feature(async_await)]
/// use tikv_client::{Config, transaction::{Client, Connect}};
/// use futures::prelude::*;
///
/// # futures::executor::block_on(async {
/// let connect: Connect = Client::connect(Config::default());
/// let client: Client = connect.await.unwrap();
/// # });
/// ```
#[derive(new)]
pub struct Connect {
    config: Config,
}

impl Future for Connect {
    type Output = Result<Client>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        let config = &self.config;
        // TODO: RpcClient::connect currently uses a blocking implementation.
        //       Make it asynchronous later.
        let rpc = Arc::new(RpcClient::connect(config)?);
        Poll::Ready(Ok(Client { rpc }))
    }
}
