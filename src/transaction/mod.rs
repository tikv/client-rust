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

use kvproto::kvrpcpb;

use crate::{Key, Value};

mod client;
mod requests;
#[allow(clippy::module_inception)]
mod transaction;

pub enum Mutation {
    Put(Value),
    Del,
    Lock,
    Rollback,
}

impl Mutation {
    fn with_key(self, key: impl Into<Key>) -> kvrpcpb::Mutation {
        let mut pb = kvrpcpb::Mutation {
            key: key.into().into(),
            ..Default::default()
        };
        match self {
            Mutation::Put(v) => {
                pb.set_op(kvrpcpb::Op::Put);
                pb.set_value(v.into());
            }
            Mutation::Del => pb.set_op(kvrpcpb::Op::Del),
            Mutation::Lock => pb.set_op(kvrpcpb::Op::Lock),
            Mutation::Rollback => pb.set_op(kvrpcpb::Op::Rollback),
        };
        pb
    }
}
