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
pub use self::requests::Scanner;
pub use self::transaction::{Snapshot, Transaction, TxnInfo};
pub use super::rpc::Timestamp;

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
