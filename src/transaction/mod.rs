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
pub use snapshot::Snapshot;
pub use transaction::Transaction;

use std::convert::TryInto;

mod buffer;
mod client;
mod requests;
mod snapshot;
#[allow(clippy::module_inception)]
mod transaction;

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct Timestamp {
    pub physical: i64,
    pub logical: i64,
}

impl Timestamp {
    pub fn into_version(self) -> u64 {
        const PHYSICAL_SHIFT_BITS: u64 = 18;

        ((self.physical << PHYSICAL_SHIFT_BITS) + self.logical)
            .try_into()
            .expect("Overflow converting timestamp to version")
    }
}
