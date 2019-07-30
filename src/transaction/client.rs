// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{Snapshot, Timestamp, Transaction};
use crate::{Config, Error};

use derive_new::new;
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;

/// The TiKV transactional `Client` is used to issue requests to the TiKV server and PD cluster.
pub struct Client;

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
    /// let transaction = client.begin();
    /// // ... Issue some commands.
    /// let commit = transaction.commit();
    /// let result: () = commit.await.unwrap();
    /// # });
    /// ```
    pub fn begin(&self) -> Transaction {
        unimplemented!()
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
    /// let snapshot = client.snapshot();
    /// // ... Issue some commands.
    /// # });
    /// ```
    pub fn snapshot(&self) -> Snapshot {
        unimplemented!()
    }

    /// Gets a [`Snapshot`](Snapshot) at the given point in time.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// use tikv_client::{Config, transaction::Client};
    /// use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = Client::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// let snapshot = client.snapshot();
    /// // ... Issue some commands.
    /// # });
    /// ```
    pub fn snapshot_at(&self, timestamp: Timestamp) -> Snapshot {
        unimplemented!()
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
    /// let timestamp = client.current_timestamp();
    /// # });
    /// ```
    pub fn current_timestamp(&self) -> Timestamp {
        unimplemented!()
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
    type Output = Result<Client, Error>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        let _config = &self.config;
        unimplemented!()
    }
}
