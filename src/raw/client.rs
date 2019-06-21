// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::ColumnFamily;
use crate::{rpc::RpcClient, BoundRange, Config, Error, Key, KvPair, Result, Value};

use futures::future::Either;
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::{pin::Pin, sync::Arc, u32};

const MAX_RAW_KV_SCAN_LIMIT: u32 = 10240;

/// The TiKV raw [`Client`](Client) is used to issue requests to the TiKV server and PD cluster.
#[derive(Clone)]
pub struct Client {
    rpc: Arc<RpcClient>,
    cf: Option<ColumnFamily>,
    key_only: bool,
}

// The macros below make writing `impl Client` concise and hopefully easy to
// read. Otherwise, the important bits get lost in boilerplate.

// `request` and `scan_request` define public functions which return a future for
// a request. When using them, supply the name of the function and the names of
// arguments, the name of the function on the RPC client, and the `Output` type
// of the returned future.
macro_rules! request {
    ($fn_name:ident ($($args:ident),*), $rpc_name:ident, $output:ty) => {
        pub fn $fn_name(
            &self,
            $($args: arg_type!($args),)*
        ) -> impl Future<Output = Result<$output>> {
            let this = self.clone();
            this.rpc.$rpc_name($(arg_convert!($args, $args)),*, this.cf)
        }
    }
}

macro_rules! scan_request {
    ($fn_name:ident ($($args:ident),*), $rpc_name:ident, $output:ty) => {
        pub fn $fn_name(
            &self,
            $($args: arg_type!($args),)*
            limit: u32,
        ) -> impl Future<Output = Result<$output>> {
            if limit > MAX_RAW_KV_SCAN_LIMIT {
                Either::Right(future::err(Error::max_scan_limit_exceeded(
                    limit,
                    MAX_RAW_KV_SCAN_LIMIT,
                )))
            } else {
                let this = self.clone();
                Either::Left(this.rpc.$rpc_name($(arg_convert!($args, $args)),*, limit, this.key_only, this.cf))
            }
        }
    }
}

// The following macros are used by the above macros to understand how arguments
// should be treated. `self` and `limit` (in scan requests) are treated separately.
// Skip to the use of `args!` to see the definitions.
//
// When using arguments in the `request` macros, we need to use their name, type,
// and be able to convert them for the RPC client functions. There are two kinds
// of argument - individual values, and collections of values. In the first case
// we always use `Into`, and in the second we take an iterator which we also
// also transform using `Into::into`. This gives users maximum flexibility.
//
// `arg_type` defines the type for values (`into`) and collections (`iter`).
// Likewise, `arg_convert` rule defines how to convert the argument into the more
// concrete type required by the RPC client. Both macros are created by `args`.

macro_rules! arg_type_rule {
    (into<$ty:ty>) => (impl Into<$ty>);
    (iter<$ty:ty>) => (impl IntoIterator<Item = impl Into<$ty>>);
}

macro_rules! arg_convert_rule {
    (into $id:ident) => {
        $id.into()
    };
    (iter $id:ident) => {
        $id.into_iter().map(Into::into).collect()
    };
}

// `$i` is the name of the argument (e.g, `key`)
// `$kind` is either `iter` or `into`.
// `$ty` is the concrete type of the argument.
macro_rules! args {
    ($($i:ident: $kind:ident<$ty:ty>;)*) => {
        macro_rules! arg_type {
            $(($i) => (arg_type_rule!($kind<$ty>));)*
        }
        macro_rules! arg_convert {
            $(($i, $id : ident) => (arg_convert_rule!($kind $id));)*
        }
    }
}

args! {
    key: into<Key>;
    keys: iter<Key>;
    value: into<Value>;
    pairs: iter<KvPair>;
    range: into<BoundRange>;
    ranges: iter<BoundRange>;
}

impl Client {
    /// Create a new [`Client`](Client) once the [`Connect`](Connect) resolves.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = Client::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// # });
    /// ```
    pub fn connect(config: Config) -> Connect {
        Connect::new(config)
    }

    /// Set the column family of requests.
    ///
    /// This function returns a new `Client`, requests created with it will have the
    /// supplied column family constraint. The original `Client` can still be used.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = Client::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// let get_request = client.with_cf("write").get("foo");
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
    /// # use tikv_client::{Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// let connect = Client::connect(Config::default());
    /// let client = connect.await.unwrap();
    /// let scan_request = client.with_key_only(true).scan("TiKV"..="TiDB", 2);
    /// # });
    /// ```
    pub fn with_key_only(&self, key_only: bool) -> Client {
        Client {
            rpc: self.rpc.clone(),
            cf: self.cf.clone(),
            key_only,
        }
    }

    /// Create a new get request.
    ///
    /// Once resolved this request will result in the fetching of the value associated with the
    /// given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Value, Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let key = "TiKV";
    /// let req = connected_client.get(key);
    /// let result: Option<Value> = req.await.unwrap();
    /// # });
    /// ```
    request!(get(key), raw_get, Option<Value>);

    /// Create a new batch get request.
    ///
    /// Once resolved this request will result in the fetching of the values associated with the
    /// given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{KvPair, Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let keys = vec!["TiKV", "TiDB"];
    /// let req = connected_client.batch_get(keys);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// # });
    /// ```
    request!(batch_get(keys), raw_batch_get, Vec<KvPair>);

    /// Create a new [`Put`](Put) request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Value, Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let key = "TiKV";
    /// let val = "TiKV";
    /// let req = connected_client.put(key, val);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    request!(put(key, value), raw_put, ());

    /// Create a new [`BatchPut`](BatchPut) request.
    ///
    /// Once resolved this request will result in the setting of the value associated with the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Error, Result, KvPair, Key, Value, Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let kvpair1 = ("PD", "Go");
    /// let kvpair2 = ("TiKV", "Rust");
    /// let iterable = vec![kvpair1, kvpair2];
    /// let req = connected_client.batch_put(iterable);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    request!(batch_put(pairs), raw_batch_put, ());

    /// Create a new [`Delete`](Delete) request.
    ///
    /// Once resolved this request will result in the deletion of the given key.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let key = "TiKV";
    /// let req = connected_client.delete(key);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    request!(delete(key), raw_delete, ());

    /// Create a new [`BatchDelete`](BatchDelete) request.
    ///
    /// Once resolved this request will result in the deletion of the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let keys = vec!["TiKV", "TiDB"];
    /// let req = connected_client.batch_delete(keys);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    request!(batch_delete(keys), raw_batch_delete, ());

    /// Create a new [`Scan`](Scan) request.
    ///
    /// Once resolved this request will result in a scanner over the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{KvPair, Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = connected_client.scan(inclusive_range, 2);
    /// let result: Vec<KvPair> = req.await.unwrap();
    /// # });
    /// ```
    scan_request!(scan(range), raw_scan, Vec<KvPair>);

    /// Create a new [`BatchScan`](BatchScan) request.
    ///
    /// Once resolved this request will result in a set of scanners over the given keys.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let inclusive_range1 = "TiDB"..="TiKV";
    /// let inclusive_range2 = "TiKV"..="TiSpark";
    /// let iterable = vec![inclusive_range1, inclusive_range2];
    /// let req = connected_client.batch_scan(iterable, 2);
    /// let result = req.await;
    /// # });
    /// ```
    scan_request!(batch_scan(ranges), raw_batch_scan, Vec<KvPair>);

    /// Create a new [`DeleteRange`](DeleteRange) request.
    ///
    /// Once resolved this request will result in the deletion of all keys over the given range.
    ///
    /// ```rust,no_run
    /// # #![feature(async_await)]
    /// # use tikv_client::{Key, Config, raw::Client};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// # let connecting_client = Client::connect(Config::new(vec!["192.168.0.100", "192.168.0.101"]));
    /// # let connected_client = connecting_client.await.unwrap();
    /// let inclusive_range = "TiKV"..="TiDB";
    /// let req = connected_client.delete_range(inclusive_range);
    /// let result: () = req.await.unwrap();
    /// # });
    /// ```
    request!(delete_range(range), raw_delete_range, ());
}

/// An unresolved [`Client`](Client) connection to a TiKV cluster.
///
/// Once resolved it will result in a connected [`Client`](Client).
///
/// ```rust,no_run
/// # #![feature(async_await)]
/// use tikv_client::{Config, raw::{Client, Connect}};
/// use futures::prelude::*;
///
/// # futures::executor::block_on(async {
/// let connect: Connect = Client::connect(Config::default());
/// let client: Client = connect.await.unwrap();
/// # });
/// ```
pub struct Connect {
    config: Config,
}

impl Connect {
    fn new(config: Config) -> Self {
        Connect { config }
    }
}

impl Future for Connect {
    type Output = Result<Client>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        let config = &self.config;
        let rpc = Arc::new(RpcClient::connect(config)?);
        Poll::Ready(Ok(Client {
            rpc,
            cf: None,
            key_only: false,
        }))
    }
}
