// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

pub use cluster::{Cluster, Connection};
use kvproto::pdpb;

#[macro_use]
extern crate log;

mod cluster;
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

impl PdResponse for pdpb::UpdateGcSafePointResponse {
    fn header(&self) -> &pdpb::ResponseHeader {
        self.get_header()
    }
}
