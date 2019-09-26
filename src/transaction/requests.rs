use crate::{
    kv_client::{KvClient, RpcFnType, Store},
    pd::PdClient,
    request::{store_stream_for_key, store_stream_for_keys, KvRequest},
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
        store_stream_for_key(key, pd_client)
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
        store_stream_for_keys(keys, pd_client)
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

// For a ResolveLockRequest, it does not contain enough information about which region it should be
// sent to. Therefore, the context must be specified every time the request is created and we don't
// retry the request automatically on region errors.
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

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results.try_collect().boxed()
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

// TODO: Add lite resolve lock (resolve specified locks only)

impl KvRequest for kvrpcpb::CleanupRequest {
    /// Commit version if the key is committed, 0 otherwise.
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
        store_stream_for_key(key, pd_client)
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

impl AsRef<Key> for kvrpcpb::Mutation {
    fn as_ref(&self) -> &Key {
        self.key.as_ref()
    }
}

impl KvRequest for kvrpcpb::PrewriteRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::PrewriteResponse;
    type KeyData = Vec<kvrpcpb::Mutation>;
    const REQUEST_NAME: &'static str = "kv_prewrite";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::kv_prewrite_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        mutations: Self::KeyData,
        store: &Store<KvC>,
    ) -> Self {
        let mut req = store.request::<Self>();
        req.set_mutations(mutations);
        req.set_primary_lock(self.primary_lock.clone());
        req.set_start_version(self.start_version);
        req.set_lock_ttl(self.lock_ttl);
        req.set_skip_constraint_check(self.skip_constraint_check);
        req.set_txn_size(self.txn_size);

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let mutations = mem::replace(&mut self.mutations, Vec::default());
        store_stream_for_keys(mutations, pd_client)
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .boxed()
    }
}

impl HasLocks for kvrpcpb::PrewriteResponse {
    fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
        self.errors
            .iter_mut()
            .filter_map(|error| error.locked.take())
            .collect()
    }
}

pub fn new_prewrite_request(
    mutations: Vec<kvrpcpb::Mutation>,
    primary_lock: Key,
    start_version: u64,
    lock_ttl: u64,
) -> kvrpcpb::PrewriteRequest {
    let mut req = kvrpcpb::PrewriteRequest::default();
    req.set_mutations(mutations);
    req.set_primary_lock(primary_lock.into());
    req.set_start_version(start_version);
    req.set_lock_ttl(lock_ttl);
    // TODO: Lite resolve lock is currently disabled
    req.set_txn_size(std::u64::MAX);

    req
}

impl KvRequest for kvrpcpb::CommitRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::CommitResponse;
    type KeyData = Vec<Vec<u8>>;
    const REQUEST_NAME: &'static str = "kv_commit";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::kv_commit_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, keys: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_keys(keys);
        req.set_start_version(self.start_version);
        req.set_commit_version(self.commit_version);

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let keys = mem::replace(&mut self.keys, Vec::default());
        store_stream_for_keys(keys, pd_client)
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .boxed()
    }
}

pub fn new_commit_request(
    keys: Vec<Key>,
    start_version: u64,
    commit_version: u64,
) -> kvrpcpb::CommitRequest {
    let mut req = kvrpcpb::CommitRequest::default();
    req.set_keys(keys.into_iter().map(Into::into).collect());
    req.set_start_version(start_version);
    req.set_commit_version(commit_version);

    req
}

impl KvRequest for kvrpcpb::BatchRollbackRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::BatchRollbackResponse;
    type KeyData = Vec<Vec<u8>>;
    const REQUEST_NAME: &'static str = "kv_batch_rollback";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::kv_batch_rollback_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, keys: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_keys(keys);
        req.set_start_version(self.start_version);

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let keys = mem::replace(&mut self.keys, Vec::default());
        store_stream_for_keys(keys, pd_client)
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .boxed()
    }
}

pub fn new_batch_rollback_request(
    keys: Vec<Key>,
    start_version: u64,
) -> kvrpcpb::BatchRollbackRequest {
    let mut req = kvrpcpb::BatchRollbackRequest::default();
    req.set_keys(keys.into_iter().map(Into::into).collect());
    req.set_start_version(start_version);

    req
}

impl HasLocks for kvrpcpb::CommitResponse {}
impl HasLocks for kvrpcpb::CleanupResponse {}
impl HasLocks for kvrpcpb::BatchRollbackResponse {}
impl HasLocks for kvrpcpb::ResolveLockResponse {}
