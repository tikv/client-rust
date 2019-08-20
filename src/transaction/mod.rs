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
pub use transaction::Transaction;

use crate::{Key, Value};

use kvproto::kvrpcpb;

mod client;
mod requests;
#[allow(clippy::module_inception)]
mod transaction;

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct Timestamp {
    pub physical: i64,
    pub logical: i64,
}

#[derive(Debug, Clone)]
enum Mutation {
    Put(Value),
    Del,
    Lock,
}

impl Mutation {
    fn into_proto_with_key(self, key: Key) -> kvrpcpb::Mutation {
        let mut pb = kvrpcpb::Mutation {
            key: key.into(),
            ..Default::default()
        };
        match self {
            Mutation::Put(v) => {
                pb.set_op(kvrpcpb::Op::Put);
                pb.set_value(v.into());
            }
            Mutation::Del => pb.set_op(kvrpcpb::Op::Del),
            Mutation::Lock => pb.set_op(kvrpcpb::Op::Lock),
        };
        pb
    }

    fn get_value(&self) -> MutationValue {
        match self {
            Mutation::Put(value) => MutationValue::Determined(Some(value.clone())),
            Mutation::Del => MutationValue::Determined(None),
            Mutation::Lock => MutationValue::Undetermined,
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
enum MutationValue {
    Determined(Option<Value>),
    Undetermined,
}

impl MutationValue {
    fn unwrap(self) -> Option<Value> {
        match self {
            MutationValue::Determined(v) => v,
            MutationValue::Undetermined => unreachable!(),
        }
    }
}
