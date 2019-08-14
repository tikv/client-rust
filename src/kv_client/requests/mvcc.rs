use crate::{
    kv_client::{requests::KvRequest, KvClient, RpcFnType, Store},
    pd::PdClient,
    BoundRange, Error, Key, KvPair, Result, Value,
};
use futures::future::BoxFuture;
use futures::prelude::*;
use futures::stream::BoxStream;
use kvproto::kvrpcpb;
use kvproto::tikvpb::TikvClient;
use std::mem;
use std::sync::Arc;

#[derive(Clone)]
pub struct MvccGet {
    pub key: Key,
    pub version: u64,
}

impl KvRequest for MvccGet {
    type Result = Option<Value>;
    type RpcRequest = kvrpcpb::GetRequest;
    type RpcResponse = kvrpcpb::GetResponse;
    type KeyType = Key;
    const REQUEST_NAME: &'static str = "kv_get";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> = TikvClient::kv_get_async_opt;

    fn into_request<KvC: KvClient>(
        self,
        key: Self::KeyType,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_key(key.into());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyType, Store<PdC::KvClient>)>> {
        let key = self.key.clone();
        pd_client
            .store_for_key(&self.key)
            .map_ok(move |store| (key, store))
            .into_stream()
            .boxed()
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        let result: Value = resp.take_value().into();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .boxed()
    }
}

#[derive(Clone)]
pub struct MvccBatchGet {
    pub keys: Vec<Key>,
    pub version: u64,
}

impl KvRequest for MvccBatchGet {
    type Result = Vec<KvPair>;
    type RpcRequest = kvrpcpb::BatchGetRequest;
    type RpcResponse = kvrpcpb::BatchGetResponse;
    type KeyType = Vec<Key>;
    const REQUEST_NAME: &'static str = "kv_batch_get";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> =
        TikvClient::kv_batch_get_async_opt;

    fn into_request<KvC: KvClient>(
        self,
        keys: Self::KeyType,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_keys(keys.into_iter().map(Into::into).collect());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyType, Store<PdC::KvClient>)>> {
        let mut keys = Vec::new();
        mem::swap(&mut keys, &mut self.keys);

        pd_client
            .clone()
            .group_keys_by_region(keys.into_iter())
            .and_then(move |(region_id, key)| {
                pd_client
                    .clone()
                    .store_for_id(region_id)
                    .map_ok(move |store| (key, store))
            })
            .boxed()
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        resp.take_pairs().into_iter().map(Into::into).collect()
    }

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results.try_concat().boxed()
    }
}

#[derive(Clone)]
pub struct MvccScan {
    pub range: BoundRange,
    // TODO this limit is currently treated as a per-region limit, not a total
    // limit.
    pub limit: u32,
    pub key_only: bool,
    pub reverse: bool,
    pub version: u64,
}

impl KvRequest for MvccScan {
    type Result = Vec<KvPair>;
    type RpcRequest = kvrpcpb::ScanRequest;
    type RpcResponse = kvrpcpb::ScanResponse;
    type KeyType = (Key, Key);
    const REQUEST_NAME: &'static str = "kv_scan";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> = TikvClient::kv_scan_async_opt;

    fn into_request<KvC: KvClient>(
        self,
        (start_key, end_key): Self::KeyType,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());
        req.set_limit(self.limit);
        req.set_key_only(self.key_only);

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        _pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyType, Store<PdC::KvClient>)>> {
        future::err(Error::unimplemented()).into_stream().boxed()
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        resp.take_pairs().into_iter().map(Into::into).collect()
    }

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results.try_concat().boxed()
    }
}
