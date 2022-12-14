// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! Transactional related functionality.
//!
//! Using the [`TransactionClient`](client::Client) you can utilize TiKV's transactional interface.
//!
//! This interface offers SQL-like transactions on top of the raw interface.
//!
//! **Warning:** It is not advisable to use both raw and transactional functionality in the same keyspace.

pub use client::Client;
pub(crate) use lock::{resolve_locks, HasLocks};
pub use snapshot::Snapshot;
#[doc(hidden)]
pub use transaction::HeartbeatOption;
pub use transaction::{CheckLevel, Transaction, TransactionOptions};

mod buffer;
mod client;
pub mod lowering;
#[macro_use]
mod requests;
mod lock;
pub use lock::{LockResolver, ResolveLocksContext, ResolveLocksOptions};
mod snapshot;
#[allow(clippy::module_inception)]
mod transaction;
