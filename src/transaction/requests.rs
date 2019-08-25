use crate::{
    kv_client::{KvClient, RpcFnType, Store},
    pd::PdClient,
    request::KvRequest,
    transaction::{HasLocks, Timestamp},
    Error, Key, KvPair, Result, Value,
};

use futures::future::BoxFuture;
use futures::prelude::*;
use futures::stream::BoxStream;
use kvproto::kvrpcpb;
use kvproto::tikvpb::TikvClient;
use std::mem;
use std::sync::Arc;

impl KvRequest for kvrpcpb::GetRequest {
    type Result = Option<Value>;
    type RpcResponse = kvrpcpb::GetResponse;
    type KeyData = Key;
    const REQUEST_NAME: &'static str = "kv_get";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::kv_get_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, key: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_key(key.into());
        req.set_version(self.version);

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let key = mem::replace(&mut self.key, Vec::default()).into();
        pd_client
            .store_for_key(&key)
            .map_ok(move |store| (key, store))
            .into_stream()
            .boxed()
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        let result: Value = resp.take_value().into();
        if resp.not_found {
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

impl HasLocks for kvrpcpb::GetResponse {
    fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
        self.error
            .as_mut()
            .and_then(|error| error.locked.take())
            .into_iter()
            .collect()
    }
}

pub fn new_mvcc_get_request(key: impl Into<Key>, timestamp: Timestamp) -> kvrpcpb::GetRequest {
    let mut req = kvrpcpb::GetRequest::default();
    req.set_key(key.into().into());
    req.set_version(timestamp.into_version());
    req
}

impl KvRequest for kvrpcpb::BatchGetRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::BatchGetResponse;
    type KeyData = Vec<Key>;
    const REQUEST_NAME: &'static str = "kv_batch_get";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::kv_batch_get_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, keys: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_keys(keys.into_iter().map(Into::into).collect());
        req.set_version(self.version);

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let keys = mem::replace(&mut self.keys, Vec::default());

        pd_client
            .clone()
            .group_keys_by_region(keys.into_iter().map(Into::into))
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

impl HasLocks for kvrpcpb::BatchGetResponse {
    fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
        self.pairs
            .iter_mut()
            .filter_map(|pair| pair.error.as_mut().and_then(|error| error.locked.take()))
            .collect()
    }
}

pub fn new_mvcc_get_batch_request(
    keys: Vec<Key>,
    timestamp: Timestamp,
) -> kvrpcpb::BatchGetRequest {
    let mut req = kvrpcpb::BatchGetRequest::default();
    req.set_keys(keys.into_iter().map(Into::into).collect());
    req.set_version(timestamp.into_version());
    req
}

impl KvRequest for kvrpcpb::ScanRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::ScanResponse;
    type KeyData = (Key, Key);
    const REQUEST_NAME: &'static str = "kv_scan";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::kv_scan_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        (start_key, end_key): Self::KeyData,
        store: &Store<KvC>,
    ) -> Self {
        let mut req = store.request::<Self>();
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());
        req.set_limit(self.limit);
        req.set_key_only(self.key_only);
        req.set_version(self.version);

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        _pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
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

impl HasLocks for kvrpcpb::ScanResponse {
    fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
        self.pairs
            .iter_mut()
            .filter_map(|pair| pair.error.as_mut().and_then(|error| error.locked.take()))
            .collect()
    }
}

impl KvRequest for kvrpcpb::ResolveLockRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::ResolveLockResponse;
    type KeyData = (kvrpcpb::Context, Vec<Vec<u8>>);
    const REQUEST_NAME: &'static str = "kv_resolve_lock";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::kv_resolve_lock_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        (context, keys): Self::KeyData,
        _store: &Store<KvC>,
    ) -> Self {
        let mut req = Self::default();
        req.set_context(context);
        req.set_start_version(self.start_version);
        req.set_commit_version(self.commit_version);
        req.set_txn_infos(self.txn_infos.clone());
        req.set_keys(keys);

        req
    }

    fn on_region_error(
        self,
        region_error: Error,
        _pd_client: Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        stream::once(future::err(region_error)).boxed()
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let context = self
            .context
            .take()
            .expect("ResolveLockRequest context must be given ");
        let keys = mem::replace(&mut self.keys, Vec::default());
        pd_client
            .store_for_id(context.region_id)
            .map_ok(move |store| ((context, keys), store))
            .into_stream()
            .boxed()
    }

    fn map_result(_resp: Self::RpcResponse) -> Self::Result {}

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .boxed()
    }
}

pub fn new_resolve_lock_request(
    context: kvrpcpb::Context,
    start_version: u64,
    commit_version: u64,
) -> kvrpcpb::ResolveLockRequest {
    let mut req = kvrpcpb::ResolveLockRequest::default();
    req.set_context(context);
    req.set_start_version(start_version);
    req.set_commit_version(commit_version);

    req
}

impl KvRequest for kvrpcpb::CleanupRequest {
    /// Commit version if the key is commiitted, 0 otherwise.
    type Result = u64;
    type RpcResponse = kvrpcpb::CleanupResponse;
    type KeyData = Key;
    const REQUEST_NAME: &'static str = "kv_cleanup";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::kv_cleanup_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, key: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_key(key.into());
        req.set_start_version(self.start_version);

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let key = mem::replace(&mut self.key, Default::default()).into();
        pd_client
            .store_for_key(&key)
            .map_ok(move |store| (key, store))
            .into_stream()
            .boxed()
    }

    fn map_result(resp: Self::RpcResponse) -> Self::Result {
        resp.commit_version
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

pub fn new_cleanup_request(key: impl Into<Key>, start_version: u64) -> kvrpcpb::CleanupRequest {
    let mut req = kvrpcpb::CleanupRequest::default();
    req.set_key(key.into().into());
    req.set_start_version(start_version);

    req
}

dummy_impl_has_locks!(PrewriteResponse);
dummy_impl_has_locks!(CommitResponse);
dummy_impl_has_locks!(CleanupResponse);
dummy_impl_has_locks!(BatchRollbackResponse);
dummy_impl_has_locks!(ResolveLockResponse);
