// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Various mock versions of the various clients and other objects.
//!
//! The goal is to be able to test functionality independently of the rest of
//! the system, in particular without requiring a TiKV or PD server, or RPC layer.

mod mock_kv;
mod mock_pd;
mod mock_raw;
mod mock_rpcpd;

pub use mock_kv::{MockKvClient, MockKvConnect};
pub use mock_pd::{pd_rpc_client, MockPdClient};
pub use mock_raw::MockRawClient;
pub use mock_rpcpd::MockRpcPdClient;

use crate::{
    pd::{PdClient, PdRpcClient, RetryClient},
    request::DispatchHook,
    Config, Error, Key, Result, Timestamp,
};
use fail::fail_point;
use futures::future::{ready, BoxFuture, FutureExt};
use grpcio::CallOption;
use kvproto::{errorpb, kvrpcpb};

impl DispatchHook for kvrpcpb::ResolveLockRequest {
    fn dispatch_hook(
        &self,
        _opt: CallOption,
    ) -> Option<BoxFuture<'static, Result<kvrpcpb::ResolveLockResponse>>> {
        fail_point!("region-error", |_| {
            let mut resp = kvrpcpb::ResolveLockResponse::default();
            resp.region_error = Some(errorpb::Error::default());
            Some(ready(Ok(resp)).boxed())
        });
        Some(ready(Ok(kvrpcpb::ResolveLockResponse::default())).boxed())
    }
}
