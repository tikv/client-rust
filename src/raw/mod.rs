// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! Raw related functionality.
//!
//! Using the [`raw::Client`](client::Client) you can utilize TiKV's raw interface.
//!
//! This interface offers optimal performance as it does not require coordination with a timestamp
//! oracle, while the transactional interface does.
//!
//! **Warning:** It is not advisable to use both raw and transactional functionality in the same keyspace.

use std::convert::TryFrom;
use std::fmt;

pub use self::client::Client;
use crate::Error;

mod client;
pub mod lowering;
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
/// # Examples
/// ```rust
/// # use tikv_client::ColumnFamily;
/// # use std::convert::TryFrom;
///
/// let cf = ColumnFamily::try_from("write").unwrap();
/// let cf = ColumnFamily::try_from(String::from("write")).unwrap();
/// ```
///
/// **But, you should not need to worry about all this:** Many functions which accept a
/// `ColumnFamily` accept an `Into<ColumnFamily>`, which means all of the above types can be passed
/// directly to those functions.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ColumnFamily {
    Default,
    Lock,
    Write,
}

impl TryFrom<&str> for ColumnFamily {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "default" => Ok(ColumnFamily::Default),
            "lock" => Ok(ColumnFamily::Lock),
            "write" => Ok(ColumnFamily::Write),
            s => Err(Error::ColumnFamilyError(s.to_owned())),
        }
    }
}

impl TryFrom<String> for ColumnFamily {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        TryFrom::try_from(&*value)
    }
}

impl fmt::Display for ColumnFamily {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ColumnFamily::Default => f.write_str("default"),
            ColumnFamily::Lock => f.write_str("lock"),
            ColumnFamily::Write => f.write_str("write"),
        }
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
