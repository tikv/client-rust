// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    kv_client::{HasError, HasRegionError, KvClient, RpcFnType, Store},
    pd::PdClient,
    Result,
};

use futures::future::{BoxFuture, Either};
use futures::prelude::*;
use futures::stream::BoxStream;
use grpcio::CallOption;
use kvproto::kvrpcpb;
use std::sync::Arc;

pub trait KvRequest: Sync + Send + 'static + Sized {
    type Result;
    type RpcResponse: HasError + Clone + Send + 'static;
    /// A single `KvRequest` can be divided into a number of RPC requests because the keys span
    /// several regions or a single RPC request is too large. Most of the fields in these requests
    /// share the same content while `KeyData`, which contains keys (and associated data if any),
    /// is the part which differs among the requests.
    type KeyData;
    const REQUEST_NAME: &'static str;
    const RPC_FN: RpcFnType<Self, Self::RpcResponse>;

    fn execute(self, pd_client: Arc<impl PdClient>) -> BoxFuture<'static, Result<Self::Result>> {
        Self::reduce(
            self.response_stream(pd_client)
                .and_then(|mut response| match response.error() {
                    Some(e) => future::err(e),
                    None => future::ok(response),
                })
                .map_ok(Self::map_result)
                .boxed(),
        )
    }

    fn response_stream(
        mut self,
        pd_client: Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        let stores = self.store_stream(pd_client.clone());
        stores
            .and_then(move |(key_data, store)| {
                let request = self.make_rpc_request(key_data, &store);
                self.dispatch_hook(&request, store.call_options())
                    .unwrap_or_else(|| store.dispatch::<Self>(&request, store.call_options()))
                    .map_ok(move |response| (request, response))
            })
            .map_ok(
                // Retry on region errors
                move |(request, mut response)| match response.region_error() {
                    Some(_) => Either::Left(request.response_stream(pd_client.clone())),
                    None => Either::Right(stream::once(future::ok(response))),
                },
            )
            .try_flatten()
            .boxed()
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>>;

    fn make_rpc_request<KvC: KvClient>(&self, key_data: Self::KeyData, store: &Store<KvC>) -> Self;

    fn map_result(result: Self::RpcResponse) -> Self::Result;

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>>;
}

/// Permits easy mocking of rpc calls.
pub trait DispatchHook: KvRequest {
    fn dispatch_hook(
        &self,
        _request: &Self,
        _opt: CallOption,
    ) -> Option<BoxFuture<'static, Result<Self::RpcResponse>>> {
        None
    }
}

impl<T: KvRequest> DispatchHook for T {}

pub trait KvRpcRequest: Default {
    fn set_context(&mut self, context: kvrpcpb::Context);
}

macro_rules! impl_kv_rpc_request {
    ($name: ident) => {
        impl KvRpcRequest for kvrpcpb::$name {
            fn set_context(&mut self, context: kvrpcpb::Context) {
                self.set_context(context);
            }
        }
    };
}

impl_kv_rpc_request!(RawGetRequest);
impl_kv_rpc_request!(RawBatchGetRequest);
impl_kv_rpc_request!(RawPutRequest);
impl_kv_rpc_request!(RawBatchPutRequest);
impl_kv_rpc_request!(RawDeleteRequest);
impl_kv_rpc_request!(RawBatchDeleteRequest);
impl_kv_rpc_request!(RawScanRequest);
impl_kv_rpc_request!(RawBatchScanRequest);
impl_kv_rpc_request!(RawDeleteRangeRequest);
impl_kv_rpc_request!(GetRequest);
impl_kv_rpc_request!(ScanRequest);
impl_kv_rpc_request!(PrewriteRequest);
impl_kv_rpc_request!(CommitRequest);
impl_kv_rpc_request!(CleanupRequest);
impl_kv_rpc_request!(BatchGetRequest);
impl_kv_rpc_request!(BatchRollbackRequest);
impl_kv_rpc_request!(ResolveLockRequest);
