// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! Transactional related functionality.
//!
//! Using the [`transaction::Client`](transaction::Client) you can utilize TiKV's transactional interface.
//!
//! This interface offers SQL-like transactions on top of the raw interface.
//!
//! **Warning:** It is not advisable to use both raw and transactional functionality in the same keyspace.
//!

pub use self::client::{Client, Connect};
pub use self::requests::{BatchGet, Commit, Delete, Get, LockKeys, Rollback, Scanner, Set};
pub use self::transaction::{IsolationLevel, Snapshot, Transaction, TxnInfo};

use crate::{Key, Value};

mod client;
mod requests;
#[allow(clippy::module_inception)]
mod transaction;

pub enum Mutation {
    Put(Key, Value),
    Del(Key),
    Lock(Key),
    Rollback(Key),
}

/// A logical timestamp produced by PD.
#[derive(Copy, Clone)]
pub struct Timestamp(u64);

impl From<u64> for Timestamp {
    fn from(v: u64) -> Self {
        Timestamp(v)
    }
}

impl Timestamp {
    pub fn timestamp(self) -> u64 {
        self.0
    }

    pub fn physical(self) -> i64 {
        (self.0 >> 16) as i64
    }

    pub fn logical(self) -> i64 {
        (self.0 & 0xFFFF as u64) as i64
    }
}
