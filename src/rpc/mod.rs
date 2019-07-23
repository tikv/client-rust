// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

#[macro_use]
mod util;
mod client;
mod context;
mod pd;
mod security;
mod tikv;

pub(crate) use crate::rpc::client::RpcClient;
pub use crate::rpc::pd::Timestamp;

use derive_new::new;
use kvproto::{kvrpcpb, metapb};
use std::sync::Arc;

use crate::{
    rpc::{pd::Region, tikv::KvClient},
    Key,
};

/// A single KV store.
struct Store {
    region: Region,
    store: metapb::Store,
}

impl Store {
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

/// An object which can be identified by an IP address.
trait Address {
    fn address(&self) -> &str;
}

impl Address for Store {
    fn address(&self) -> &str {
        self.store.get_address()
    }
}

#[cfg(test)]
impl Address for &'static str {
    fn address(&self) -> &str {
        self
    }
}

impl From<Store> for kvrpcpb::Context {
    fn from(mut store: Store) -> kvrpcpb::Context {
        let mut kvctx = kvrpcpb::Context::default();
        kvctx.set_region_id(store.region.id());
        kvctx.set_region_epoch(store.region.region.take_region_epoch());
        kvctx.set_peer(store.region.peer().expect("leader must exist"));
        kvctx
    }
}

#[derive(new)]
struct RawContext {
    store: Store,
    client: Arc<KvClient>,
}

#[derive(new)]
struct TxnContext {
    store: Store,
}
