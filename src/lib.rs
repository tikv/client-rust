// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! This crate provides an easy-to-use client for [TiKV](https://github.com/tikv/tikv), a
//! distributed, transactional key-value database written in Rust.
//!
//! This crate lets you connect to a TiKV cluster and use either a transactional or raw (simple
//! get/put style without transactional consistency guarantees) API to access and update your data.
//!
//! ## Choosing an API
//!
//! This crate offers both [**raw**](raw/index.html) and
//! [**transactional**](transaction/index.html) APIs. You should choose just one for your system.
//!
//! The consequence of supporting transactions is increased overhead of coordination with the
//! placement driver and TiKV, and additional code complexity.
//!
//! *While it is possible to use both APIs at the same time, doing so is unsafe and unsupported.*
//!
//! ### Transactional
//!
//! The [transactional](transaction/index.html) API supports **transactions** via multi-version
//! concurrency control (MVCC).
//!
//! Best when you mostly do complex sets of actions, actions which may require a rollback,
//! operations affecting multiple keys or values, or operations that depend on strong consistency.
//!
//!
//! ### Raw
//!
//! The [raw](raw/index.html) API has reduced coordination overhead, but lacks any
//! transactional abilities.
//!
//! Best when you mostly do single value changes, and have very limited cross-value
//! requirements. You will not be able to use transactions with this API.

#[macro_use]
mod request;

#[macro_use]
mod transaction;

mod backoff;
mod compat;
mod config;
mod kv;
mod pd;
mod raw;
mod region;
mod stats;
mod store;
mod timestamp;
mod util;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod proptests;

#[macro_use]
extern crate log;

#[doc(inline)]
pub use crate::backoff::Backoff;
#[doc(inline)]
pub use crate::kv::{BoundRange, IntoOwnedRange, Key, KvPair, Value};
#[doc(inline)]
pub use crate::raw::{lowering::*, Client as RawClient, ColumnFamily};
#[doc(inline)]
pub use crate::request::RetryOptions;
#[doc(inline)]
pub use crate::timestamp::{Timestamp, TimestampExt};
#[doc(inline)]
pub use crate::transaction::{
    lowering::*, CheckLevel, Client as TransactionClient, Snapshot, Transaction, TransactionOptions,
};
#[doc(inline)]
pub use config::Config;
#[doc(inline)]
pub use tikv_client_common::{security::SecurityManager, Error, Result};
