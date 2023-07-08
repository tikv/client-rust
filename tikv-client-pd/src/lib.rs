// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

#[doc(inline)]
pub use cluster::Cluster;
#[doc(inline)]
pub use cluster::Connection;
#[doc(inline)]
pub use tikv_client_common::security::SecurityManager;
#[doc(inline)]
pub use tikv_client_common::Error;
#[doc(inline)]
pub use tikv_client_common::Result;

#[macro_use]
extern crate log;

mod cluster;
mod timestamp;
