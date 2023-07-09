// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

mod client;
mod errors;
mod request;

pub use tikv_client_common::security::SecurityManager;
pub use tikv_client_common::Error;
pub use tikv_client_common::Result;

#[doc(inline)]
pub use crate::client::KvClient;
#[doc(inline)]
pub use crate::client::KvConnect;
#[doc(inline)]
pub use crate::client::TikvConnect;
#[doc(inline)]
pub use crate::errors::HasKeyErrors;
#[doc(inline)]
pub use crate::errors::HasRegionError;
#[doc(inline)]
pub use crate::errors::HasRegionErrors;
#[doc(inline)]
pub use crate::request::Request;
