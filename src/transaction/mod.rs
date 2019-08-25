// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! Transactional related functionality.
//!
//! Using the [`TransactionClient`](TransactionClient) you can utilize TiKV's transactional interface.
//!
//! This interface offers SQL-like transactions on top of the raw interface.
//!
//! **Warning:** It is not advisable to use both raw and transactional functionality in the same keyspace.
//!

pub use client::{Client, Connect};
pub use lock::{resolve_locks, HasLocks};
pub use transaction::Transaction;

use std::convert::TryInto;

mod buffer;
mod client;
#[macro_use]
mod lock;
mod requests;
#[allow(clippy::module_inception)]
mod transaction;

const PHYSICAL_SHIFT_BITS: i64 = 18;
const LOGICAL_MASK: i64 = (1 << PHYSICAL_SHIFT_BITS) - 1;

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct Timestamp {
    pub physical: i64,
    pub logical: i64,
}

impl Timestamp {
    pub fn into_version(self) -> u64 {
        ((self.physical << PHYSICAL_SHIFT_BITS) + self.logical)
            .try_into()
            .expect("Overflow converting timestamp to version")
    }

    pub fn from_version(version: u64) -> Self {
        let version = version as i64;
        Self {
            physical: version >> PHYSICAL_SHIFT_BITS,
            logical: version & LOGICAL_MASK,
        }
    }
}
