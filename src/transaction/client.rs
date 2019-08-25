// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::{PdClient, PdRpcClient},
    transaction::{Snapshot, Timestamp, Transaction},
    Config, Result,
};

use derive_new::new;
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;
use std::sync::Arc;

/// The TiKV transactional `Client` is used to issue requests to the TiKV server and PD cluster.
pub struct Client {
    pd: Arc<PdRpcClient>,
}

impl Client {
    /// Creates a new [`Client`](Client) once the [`Connect`](Connect) resolves.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = TransactionClient::connect(Config::default());
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
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = TransactionClient::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// let mut transaction = client.begin().await.unwrap();
    /// // ... Issue some commands.
    /// let commit = transaction.commit();
    /// let result: () = commit.await.unwrap();
    /// # });
    /// ```
    pub async fn begin(&self) -> Result<Transaction> {
        let timestamp = self.current_timestamp().await?;
        Ok(Transaction::new(timestamp, self.pd.clone()))
    }

    /// Creates a new [`Snapshot`](Snapshot) at the given time.
    pub fn snapshot(&self, timestamp: Timestamp) -> Snapshot {
        Snapshot::new(Transaction::new(timestamp, self.pd.clone()))
    }

    /// Retrieves the current [`Timestamp`](Timestamp).
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// use tikv_client::{Config, TransactionClient};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = TransactionClient::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// let timestamp = client.current_timestamp().await.unwrap();
    /// # });
    /// ```
    pub async fn current_timestamp(&self) -> Result<Timestamp> {
        self.pd.clone().get_timestamp().await
    }
}

/// An unresolved [`Client`](Client) connection to a TiKV cluster.
///
/// Once resolved it will result in a connected [`Client`](Client).
///
/// ```rust,no_run
/// # #![feature(async_await)]
/// use tikv_client::{Config, TransactionClient, Connect};
/// use futures::prelude::*;
///
/// # futures::executor::block_on(async {
/// let connect: Connect = TransactionClient::connect(Config::default());
/// let client: TransactionClient = connect.await.unwrap();
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
        // TODO: PdRpcClient::connect currently uses a blocking implementation.
        //       Make it asynchronous later.
        let pd = Arc::new(PdRpcClient::connect(config)?);
        Poll::Ready(Ok(Client { pd }))
    }
}
