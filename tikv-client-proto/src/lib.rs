// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use protos::*;
pub use protos::{
    coprocessor, coprocessor_v2, errorpb, kvrpcpb, metapb, mpp, pdpb, raft_serverpb, tikvpb,
};

#[allow(dead_code)]
#[allow(clippy::all)]
mod protos {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}
