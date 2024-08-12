// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![allow(clippy::large_enum_variant)]
#![allow(clippy::enum_variant_names)]

pub use protos::*;

#[allow(clippy::doc_lazy_continuation)]
mod protos {
    include!("generated/mod.rs");
}
