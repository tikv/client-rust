// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! Transactional related functionality.
//!
//! Using the [`TransactionClient`](client::Client) you can utilize TiKV's transactional interface.
//!
//! This interface offers SQL-like transactions on top of the raw interface.
//!
//! **Warning:** It is not advisable to use both raw and transactional functionality in the same keyspace.

pub use client::Client;
pub(crate) use lock::resolve_locks;
pub(crate) use lock::HasLocks;
pub use snapshot::Snapshot;
pub use transaction::CheckLevel;
#[doc(hidden)]
pub use transaction::HeartbeatOption;
pub use transaction::Mutation;
pub use transaction::Transaction;
pub use transaction::TransactionOptions;

mod buffer;
mod client;
mod lock;
pub mod lowering;
mod requests;
pub use lock::LockResolver;
pub use lock::ResolveLocksContext;
pub use lock::ResolveLocksOptions;
mod snapshot;
#[allow(clippy::module_inception)]
mod transaction;
