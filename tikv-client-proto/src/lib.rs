// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub use protos::*;

#[allow(dead_code)]
#[allow(clippy::all)]
mod protos {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

    use raft_proto::eraftpb;
}
