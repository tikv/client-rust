// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

pub use raw::*;

use crate::{
    kv_client::{HasError, KvClient, RpcFnType, Store},
    pd::PdClient,
    Result,
};

use futures::future::BoxFuture;
use futures::prelude::*;
use futures::stream::BoxStream;
use grpcio::CallOption;
use kvproto::kvrpcpb;
use std::sync::Arc;

mod mvcc;
mod raw;

pub trait KvRequest: Sync + Send + 'static + Sized + Clone {
    type Result;
    type RpcRequest;
    type RpcResponse: HasError + Clone + Send + 'static;
    type KeyType;
    const REQUEST_NAME: &'static str;
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse>;

    fn execute(
        mut self,
        pd_client: Arc<impl PdClient>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        let stores = self.store_stream(pd_client);
        Self::reduce(
            stores
                .and_then(move |(key, store)| {
                    let request = self.clone().into_request(key, &store);
                    self.mock_dispatch(&request, store.call_options())
                        .unwrap_or_else(|| store.dispatch::<Self>(&request, store.call_options()))
                })
                .map_ok(move |r| Self::map_result(r))
                .boxed(),
        )
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyType, Store<PdC::KvClient>)>>;

    fn into_request<KvC: KvClient>(
        self,
        key: Self::KeyType,
        store: &Store<KvC>,
    ) -> Self::RpcRequest;

    fn map_result(result: Self::RpcResponse) -> Self::Result;

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>>;
}

/// Permits easy mocking of rpc calls.
pub trait MockDispatch: KvRequest {
    fn mock_dispatch(
        &self,
        _request: &Self::RpcRequest,
        _opt: CallOption,
    ) -> Option<BoxFuture<'static, Result<Self::RpcResponse>>> {
        None
    }
}

impl<T: KvRequest> MockDispatch for T {}

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
