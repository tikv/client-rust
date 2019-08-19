// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

// FIXME: Remove this when txn is done.
#![allow(dead_code)]

use derive_new::new;
use futures::compat::Compat01As03;
use futures::future::BoxFuture;
use futures::prelude::*;
use grpcio::CallOption;
use kvproto::kvrpcpb;
use kvproto::tikvpb::TikvClient;
use std::sync::Arc;

use crate::{
    kv_client::{request::KvRequest, HasError},
    stats::tikv_stats,
    transaction::TxnInfo,
    ErrorKind, Result,
};

/// This client handles requests for a single TiKV node. It converts the data
/// types and abstractions of the client program into the grpc data types.
#[derive(new, Clone)]
pub struct KvRpcClient {
    rpc_client: Arc<TikvClient>,
}

impl super::KvClient for KvRpcClient {
    fn dispatch<T: KvRequest>(
        &self,
        request: &T::RpcRequest,
        opt: CallOption,
    ) -> BoxFuture<'static, Result<T::RpcResponse>> {
        map_errors_and_trace(T::REQUEST_NAME, T::RPC_FN(&self.rpc_client, request, opt)).boxed()
    }
}

fn map_errors_and_trace<Resp, RpcFuture>(
    request_name: &'static str,
    fut: ::grpcio::Result<RpcFuture>,
) -> impl Future<Output = Result<Resp>>
where
    Compat01As03<RpcFuture>: Future<Output = std::result::Result<Resp, ::grpcio::Error>>,
    Resp: HasError + Sized + Clone + Send + 'static,
{
    let context = tikv_stats(request_name);

    // FIXME should handle the error, not unwrap.
    Compat01As03::new(fut.unwrap())
        .map(|r| match r {
            Err(e) => Err(ErrorKind::Grpc(e).into()),
            Ok(mut r) => {
                if let Some(e) = r.region_error() {
                    Err(e)
                } else if let Some(e) = r.error() {
                    Err(e)
                } else {
                    Ok(r)
                }
            }
        })
        .map(move |r| context.done(r))
}

impl From<TxnInfo> for kvrpcpb::TxnInfo {
    fn from(txn_info: TxnInfo) -> kvrpcpb::TxnInfo {
        let mut pb = kvrpcpb::TxnInfo::default();
        pb.set_txn(txn_info.txn);
        pb.set_status(txn_info.status);
        pb
    }
}
