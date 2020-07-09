// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! Transactional related functionality.
//!
//! Using the [`TransactionClient`](TransactionClient) you can utilize TiKV's transactional interface.
//!
//! This interface offers SQL-like transactions on top of the raw interface.
//!
//! **Warning:** It is not advisable to use both raw and transactional functionality in the same keyspace.
//!

pub use client::Client;
pub use lock::{resolve_locks, HasLocks};
pub use snapshot::Snapshot;
pub use transaction::Transaction;

use std::convert::TryInto;

mod buffer;
mod client;
#[macro_use]
mod requests;
mod snapshot;
#[allow(clippy::module_inception)]
mod transaction;

