// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

pub use cluster::Cluster;
use kvproto::pdpb;

#[macro_use]
extern crate tikv_client_common;
#[macro_use]
extern crate log;

#[macro_use]
pub mod cluster;
mod timestamp;

trait PdResponse {
    fn header(&self) -> &pdpb::ResponseHeader;
}

impl PdResponse for pdpb::GetStoreResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.get_header()
    }
}

impl PdResponse for pdpb::GetRegionResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.get_header()
    }
}

impl PdResponse for pdpb::GetAllStoresResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.get_header()
    }
}
