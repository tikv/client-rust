// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.
#![feature(type_alias_impl_trait)]

#[macro_use]
extern crate log;

mod client;
mod errors;
pub mod region;
mod request;

#[doc(inline)]
pub use crate::{
    client::{KvClient, KvConnect, Store, TikvConnect},
    errors::{HasError, HasRegionError},
    region::{Region, RegionId, RegionVerId, StoreId},
    request::Request,
};
pub use tikv_client_common::{security::SecurityManager, Error, ErrorKind, Key, Result};
