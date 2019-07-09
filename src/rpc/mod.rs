// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

#[macro_use]
mod util;
mod client;
mod context;
mod pd;
mod security;
mod tikv;

pub(crate) use crate::rpc::client::RpcClient;

use derive_new::new;
use kvproto::{kvrpcpb, metapb};
use std::sync::Arc;

use crate::{
    rpc::{pd::Region, tikv::KvClient},
    Key,
};

struct RegionContext {
    region: Region,
    store: metapb::Store,
}

impl RegionContext {
    fn address(&self) -> &str {
        self.store.get_address()
    }

    fn start_key(&self) -> Key {
        self.region.start_key().to_vec().into()
    }

    fn end_key(&self) -> Key {
        self.region.end_key().to_vec().into()
    }

    fn range(&self) -> (Key, Key) {
        (self.start_key(), self.end_key())
    }
}

impl From<RegionContext> for kvrpcpb::Context {
    fn from(mut ctx: RegionContext) -> kvrpcpb::Context {
        let mut kvctx = kvrpcpb::Context::default();
        kvctx.set_region_id(ctx.region.id());
        kvctx.set_region_epoch(ctx.region.region.take_region_epoch());
        kvctx.set_peer(ctx.region.peer().expect("leader must exist"));
        kvctx
    }
}

#[derive(new)]
struct RawContext {
    region: RegionContext,
    client: Arc<KvClient>,
}

#[derive(new)]
struct TxnContext {
    region: RegionContext,
}
