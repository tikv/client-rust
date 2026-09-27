// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! The protobuf messages and gRPC clients generated from [kvproto], the TiKV, PD and
//! TiCDC protocol definitions this crate is built on.
//!
//! They let applications call RPCs this crate does not wrap, over the same message types:
//! for example TiCDC's `cdcpb::change_data_client::ChangeDataClient` or PD's GC safe
//! point RPCs on `pdpb::pd_client::PdClient`. The clients are [tonic] clients; build
//! their channels with the [`tonic`] re-exported here, which is the version the clients
//! are generated for (and [`prost`] for the message traits).
//!
//! ```no_run
//! use tikv_client::proto::pdpb;
//! use tikv_client::proto::tonic::transport::Channel;
//!
//! # async fn f() -> Result<(), Box<dyn std::error::Error>> {
//! let channel = Channel::from_static("http://127.0.0.1:2379").connect().await?;
//! let mut pd = pdpb::pd_client::PdClient::new(channel);
//! let members = pd.get_members(pdpb::GetMembersRequest::default()).await?;
//! let cluster_id = members.into_inner().header.unwrap().cluster_id;
//! let request = pdpb::GetGcSafePointRequest {
//!     header: Some(pdpb::RequestHeader {
//!         cluster_id,
//!         ..Default::default()
//!     }),
//! };
//! let safe_point = pd.get_gc_safe_point(request).await?.into_inner().safe_point;
//! # let _ = safe_point;
//! # Ok(())
//! # }
//! ```
//!
//! These modules are generated code and follow kvproto: they change when the vendored
//! protos are updated, independently of the rest of the API.
//!
//! [kvproto]: https://github.com/pingcap/kvproto

#![allow(clippy::large_enum_variant)]
#![allow(clippy::enum_variant_names)]

pub use prost;
pub use protos::*;
pub use tonic;

// Rust 1.93's clippy::all flags many protobuf-generated wire types as dead code.
// Prior toolchain (1.84.1) did not fail on these generated definitions.
#[allow(clippy::doc_lazy_continuation)]
#[allow(dead_code)]
#[allow(rustdoc::all)]
mod protos {
    include!("generated/mod.rs");
}
