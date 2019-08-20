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
use std::convert::TryInto;

mod client;
mod requests;
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

#[derive(Debug, Clone)]
enum Mutation {
    Cached(Option<Value>),
    Put(Value),
    Del,
    Lock,
}

impl Mutation {
    fn into_proto_with_key(self, key: Key) -> Option<kvrpcpb::Mutation> {
        let mut pb = kvrpcpb::Mutation {
            key: key.into(),
            ..Default::default()
        };
        match self {
            Mutation::Cached(_) => return None,
            Mutation::Put(v) => {
                pb.set_op(kvrpcpb::Op::Put);
                pb.set_value(v.into());
            }
            Mutation::Del => pb.set_op(kvrpcpb::Op::Del),
            Mutation::Lock => pb.set_op(kvrpcpb::Op::Lock),
        };
        Some(pb)
    }

    fn get_value(&self) -> MutationValue {
        match self {
            Mutation::Cached(value) => MutationValue::Determined(value.clone()),
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
