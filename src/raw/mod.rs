// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! Raw related functionality.
//!
//! Using the [`raw::Client`](raw::Client) you can utilize TiKV's raw interface.
//!
//! This interface offers optimal performance as it does not require coordination with a timestamp
//! oracle, while the transactional interface does.
//!
//! **Warning:** It is not advisable to use both raw and transactional functionality in the same keyspace.
//!

pub use self::client::Client;

use std::fmt;

mod client;
mod requests;

/// A [`ColumnFamily`](ColumnFamily) is an optional parameter for [`raw::Client`](Client) requests.
///
/// TiKV uses RocksDB's `ColumnFamily` support. You can learn more about RocksDB's `ColumnFamily`s [on their wiki](https://github.com/facebook/rocksdb/wiki/Column-Families).
///
/// By default in TiKV data is stored in three different `ColumnFamily` values, configurable in the TiKV server's configuration:
///
/// * Default: Where real user data is stored. Set by `[rocksdb.defaultcf]`.
/// * Write: Where MVCC and index related data are stored. Set by `[rocksdb.writecf]`.
/// * Lock: Where lock information is stored. Set by `[rocksdb.lockcf]`.
///
/// Not providing a call a `ColumnFamily` means it will use the default value of `default`.
///
/// The best (and only) way to create a [`ColumnFamily`](ColumnFamily) is via the `From` implementation:
///
/// ```rust
/// # use tikv_client::ColumnFamily;
///
/// let cf = ColumnFamily::from("write");
/// let cf = ColumnFamily::from(String::from("write"));
/// let cf = ColumnFamily::from(&String::from("write"));
/// ```
///
/// **But, you should not need to worry about all this:** Many functions which accept a
/// `ColumnFamily` accept an `Into<ColumnFamily>`, which means all of the above types can be passed
/// directly to those functions.
#[derive(Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ColumnFamily(String);

impl<T: Into<String>> From<T> for ColumnFamily {
    fn from(i: T) -> ColumnFamily {
        ColumnFamily(i.into())
    }
}

impl fmt::Display for ColumnFamily {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

trait RawRpcRequest: Default {
    fn set_cf(&mut self, cf: String);

    fn maybe_set_cf(&mut self, cf: Option<ColumnFamily>) {
        if let Some(cf) = cf {
            self.set_cf(cf.to_string());
        }
    }
}
