// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::PdClient,
    request::{KvRequest, RetryOptions},
    store::{store_stream_for_key, store_stream_for_keys, store_stream_for_range, Store},
    timestamp::TimestampExt,
    transaction::HasLocks,
    BoundRange, Error, Key, KvPair, Result, Value,
};
use async_trait::async_trait;
use futures::{prelude::*, stream::BoxStream};
use std::{collections::HashMap, iter, mem, sync::Arc};
use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

#[async_trait]
impl KvRequest for kvrpcpb::GetRequest {
    type Result = Option<Value>;
    type RpcResponse = kvrpcpb::GetResponse;
    type KeyData = Key;
    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let key = mem::take(&mut self.key).into();
        store_stream_for_key(key, pd_client)
    }

    fn make_rpc_request(&self, key: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_key(key.into());
        req.set_version(self.version);

        Ok(req)
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        if resp.not_found {
            None
        } else {
            Some(resp.take_value())
        }
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .await
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
    req.set_version(timestamp.version());
    req
}

#[async_trait]
impl KvRequest for kvrpcpb::BatchGetRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::BatchGetResponse;
    type KeyData = Vec<Key>;
    fn make_rpc_request(&self, keys: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_keys(keys.into_iter().map(Into::into).collect());
        req.set_version(self.version);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        self.keys.sort();
        let keys = mem::take(&mut self.keys);
        store_stream_for_keys(keys, pd_client)
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        resp.take_pairs().into_iter().map(Into::into).collect()
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results.try_concat().await
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
    req.set_version(timestamp.version());
    req
}

#[async_trait]
impl KvRequest for kvrpcpb::ScanRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::ScanResponse;
    type KeyData = (Key, Key);
    fn make_rpc_request(&self, (start_key, end_key): Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());
        req.set_limit(self.limit);
        req.set_key_only(self.key_only);
        req.set_version(self.version);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let start_key = mem::take(&mut self.start_key);
        let end_key = mem::take(&mut self.end_key);
        let range = BoundRange::from((start_key, end_key));
        store_stream_for_range(range, pd_client)
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        resp.take_pairs().into_iter().map(Into::into).collect()
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results.try_concat().await
    }
}

pub fn new_mvcc_scan_request(
    range: impl Into<BoundRange>,
    timestamp: Timestamp,
    limit: u32,
    key_only: bool,
) -> kvrpcpb::ScanRequest {
    let (start_key, end_key) = range.into().into_keys();
    let mut req = kvrpcpb::ScanRequest::default();
    req.set_start_key(start_key.into());
    req.set_end_key(end_key.unwrap_or_default().into());
    req.set_limit(limit);
    req.set_key_only(key_only);
    req.set_version(timestamp.version());
    req
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
#[async_trait]
impl KvRequest for kvrpcpb::ResolveLockRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::ResolveLockResponse;
    type KeyData = (kvrpcpb::Context, Vec<Vec<u8>>);
    fn make_rpc_request(&self, (context, keys): Self::KeyData, _store: &Store) -> Result<Self> {
        let mut req = Self::default();
        req.set_context(context);
        req.set_start_version(self.start_version);
        req.set_commit_version(self.commit_version);
        req.set_txn_infos(self.txn_infos.clone());
        req.set_keys(keys);

        Ok(req)
    }

    fn on_region_error(
        self,
        region_error: Error,
        _pd_client: Arc<impl PdClient>,
        _: RetryOptions,
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        stream::once(future::err(region_error)).boxed()
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let context = self
            .context
            .take()
            .expect("ResolveLockRequest context must be given ");
        let keys = mem::take(&mut self.keys);
        pd_client
            .store_for_id(context.region_id)
            .map_ok(move |store| ((context, keys), store))
            .into_stream()
            .boxed()
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results.try_collect().await
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

#[async_trait]
impl KvRequest for kvrpcpb::CleanupRequest {
    /// Commit version if the key is committed, 0 otherwise.
    type Result = u64;
    type RpcResponse = kvrpcpb::CleanupResponse;
    type KeyData = Key;
    fn make_rpc_request(&self, key: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_key(key.into());
        req.set_start_version(self.start_version);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let key = mem::take(&mut self.key).into();
        store_stream_for_key(key, pd_client)
    }

    fn map_result(resp: Self::RpcResponse) -> Self::Result {
        resp.commit_version
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .await
    }
}

pub fn new_cleanup_request(key: impl Into<Key>, start_version: u64) -> kvrpcpb::CleanupRequest {
    let mut req = kvrpcpb::CleanupRequest::default();
    req.set_key(key.into().into());
    req.set_start_version(start_version);

    req
}

#[async_trait]
impl KvRequest for kvrpcpb::PrewriteRequest {
    type Result = Vec<kvrpcpb::PrewriteResponse>;
    type RpcResponse = kvrpcpb::PrewriteResponse;
    type KeyData = Vec<kvrpcpb::Mutation>;

    fn make_rpc_request(&self, mutations: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;

        if self.use_async_commit {
            // Only need to set secondary keys if we're sending the primary key.
            if mutations.iter().any(|m| m.key == self.primary_lock) {
                req.set_secondaries(self.secondaries.clone());
            }
            req.set_use_async_commit(true);
            req.set_max_commit_ts(self.max_commit_ts);
        }
        // Only if there is only one request to send
        if mutations.len() == self.secondaries.len() + 1 {
            req.set_try_one_pc(self.try_one_pc);
        }

        req.set_mutations(mutations);
        req.set_primary_lock(self.primary_lock.clone());
        req.set_start_version(self.start_version);
        req.set_lock_ttl(self.lock_ttl);
        req.set_skip_constraint_check(self.skip_constraint_check);
        req.set_txn_size(self.txn_size);
        req.set_for_update_ts(self.for_update_ts);
        req.set_is_pessimistic_lock(self.is_pessimistic_lock.clone());
        req.set_min_commit_ts(self.min_commit_ts);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        self.mutations.sort_by(|a, b| a.key.cmp(&b.key));
        let mutations = mem::take(&mut self.mutations);
        store_stream_for_keys(mutations, pd_client)
    }

    fn map_result(response: Self::RpcResponse) -> Self::Result {
        // FIXME this is clown-shoes inefficient.
        vec![response]
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results.try_concat().await
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

pub fn new_pessimistic_prewrite_request(
    mutations: Vec<kvrpcpb::Mutation>,
    primary_lock: Key,
    start_version: u64,
    lock_ttl: u64,
    for_update_ts: u64,
) -> kvrpcpb::PrewriteRequest {
    let len = mutations.len();
    let mut req = new_prewrite_request(mutations, primary_lock, start_version, lock_ttl);
    req.set_for_update_ts(for_update_ts);
    req.set_is_pessimistic_lock(iter::repeat(true).take(len).collect());
    req
}

#[async_trait]
impl KvRequest for kvrpcpb::CommitRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::CommitResponse;
    type KeyData = Vec<Vec<u8>>;
    fn make_rpc_request(&self, keys: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_keys(keys);
        req.set_start_version(self.start_version);
        req.set_commit_version(self.commit_version);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        self.keys.sort();
        let keys = mem::take(&mut self.keys);
        store_stream_for_keys(keys, pd_client)
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .try_for_each_concurrent(None, |_| future::ready(Ok(())))
            .await
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

#[async_trait]
impl KvRequest for kvrpcpb::BatchRollbackRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::BatchRollbackResponse;
    type KeyData = Vec<Vec<u8>>;
    fn make_rpc_request(&self, keys: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_keys(keys);
        req.set_start_version(self.start_version);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        self.keys.sort();
        let keys = mem::take(&mut self.keys);
        store_stream_for_keys(keys, pd_client)
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .try_for_each_concurrent(None, |_| future::ready(Ok(())))
            .await
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

#[async_trait]
impl KvRequest for kvrpcpb::PessimisticRollbackRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::PessimisticRollbackResponse;
    type KeyData = Vec<Vec<u8>>;

    fn make_rpc_request(&self, keys: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_keys(keys);
        req.set_start_version(self.start_version);
        req.set_for_update_ts(self.for_update_ts);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        self.keys.sort();
        let keys = mem::take(&mut self.keys);
        store_stream_for_keys(keys, pd_client)
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .try_for_each_concurrent(None, |_| future::ready(Ok(())))
            .await
    }
}

pub fn new_pessimistic_rollback_request(
    keys: Vec<Key>,
    start_version: u64,
    for_update_ts: u64,
) -> kvrpcpb::PessimisticRollbackRequest {
    let mut req = kvrpcpb::PessimisticRollbackRequest::default();
    req.set_keys(keys.into_iter().map(Into::into).collect());
    req.set_start_version(start_version);
    req.set_for_update_ts(for_update_ts);

    req
}

#[async_trait]
impl KvRequest for kvrpcpb::PessimisticLockRequest {
    // FIXME: PessimisticLockResponse only contains values.
    // We need to pair keys and values returned somewhere.
    // But it's blocked by the structure of the program that `map_result` only accepts the response as input
    // Before we fix this `batch_get_for_update` is problematic.
    type Result = Vec<Vec<u8>>;
    type RpcResponse = kvrpcpb::PessimisticLockResponse;
    type KeyData = Vec<kvrpcpb::Mutation>;

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        self.mutations.sort_by(|a, b| a.key.cmp(&b.key));
        let mutations = mem::take(&mut self.mutations);
        store_stream_for_keys(mutations, pd_client)
    }

    fn make_rpc_request(&self, mutations: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_mutations(mutations);
        req.set_primary_lock(self.primary_lock.clone());
        req.set_start_version(self.start_version);
        req.set_lock_ttl(self.lock_ttl);
        req.set_for_update_ts(self.for_update_ts);
        req.set_is_first_lock(self.is_first_lock);
        req.set_wait_timeout(self.wait_timeout);
        req.set_force(self.force);
        req.set_return_values(self.return_values);
        req.set_min_commit_ts(self.min_commit_ts);

        Ok(req)
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        resp.take_values()
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results.try_concat().await
    }
}

pub fn new_pessimistic_lock_request(
    keys: Vec<impl Into<Key>>,
    primary_lock: Key,
    start_version: u64,
    lock_ttl: u64,
    for_update_ts: u64,
    need_value: bool,
) -> kvrpcpb::PessimisticLockRequest {
    let mut req = kvrpcpb::PessimisticLockRequest::default();
    let mutations = keys
        .into_iter()
        .map(|key| {
            let mut mutation = kvrpcpb::Mutation::default();
            mutation.set_op(kvrpcpb::Op::PessimisticLock);
            mutation.set_key(key.into().into());
            mutation
        })
        .collect();
    req.set_mutations(mutations);
    req.set_primary_lock(primary_lock.into());
    req.set_start_version(start_version);
    req.set_lock_ttl(lock_ttl);
    req.set_for_update_ts(for_update_ts);
    // todo: make them configurable
    req.set_is_first_lock(false);
    req.set_wait_timeout(0);
    req.set_force(false);
    req.set_return_values(need_value);
    // todo: support large transaction
    req.set_min_commit_ts(0);

    req
}

#[async_trait]
impl KvRequest for kvrpcpb::ScanLockRequest {
    type Result = Vec<kvrpcpb::LockInfo>;
    type RpcResponse = kvrpcpb::ScanLockResponse;
    type KeyData = (Key, Key); // end_key should always be empty. Used to satisfy `store_stream_for_range`

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let start_key = mem::take(&mut self.start_key);
        let range = BoundRange::from((start_key, vec![]));
        store_stream_for_range(range, pd_client)
    }

    fn make_rpc_request(&self, (start_key, _): Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_max_version(self.max_version);
        req.set_start_key(start_key.into());
        req.set_limit(self.limit);
        Ok(req)
    }

    fn map_result(mut result: Self::RpcResponse) -> Self::Result {
        result.take_locks()
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results.try_concat().await
    }
}

pub fn new_scan_lock_request(
    start_key: impl Into<Key>,
    safepoint: Timestamp,
    limit: u32,
) -> kvrpcpb::ScanLockRequest {
    let mut req = kvrpcpb::ScanLockRequest::default();
    req.set_start_key(start_key.into().into());
    req.set_max_version(safepoint.version());
    req.set_limit(limit);
    req
}

#[async_trait]
impl KvRequest for kvrpcpb::TxnHeartBeatRequest {
    type Result = u64;
    type RpcResponse = kvrpcpb::TxnHeartBeatResponse;
    type KeyData = Key;

    fn make_rpc_request(&self, key: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_start_version(self.start_version);
        req.set_primary_lock(key.into());
        req.set_advise_lock_ttl(self.advise_lock_ttl);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let key = mem::take(&mut self.primary_lock).into();
        store_stream_for_key(key, pd_client)
    }

    fn map_result(resp: Self::RpcResponse) -> Self::Result {
        resp.lock_ttl
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .await
    }
}

pub fn new_heart_beat_request(
    start_ts: Timestamp,
    primary_lock: Key,
    ttl: u64,
) -> kvrpcpb::TxnHeartBeatRequest {
    let mut req = kvrpcpb::TxnHeartBeatRequest::default();
    req.set_start_version(start_ts.version());
    req.set_primary_lock(primary_lock.into());
    req.set_advise_lock_ttl(ttl);
    req
}

#[async_trait]
impl KvRequest for kvrpcpb::CheckTxnStatusRequest {
    type Result = TransactionStatus;
    type RpcResponse = kvrpcpb::CheckTxnStatusResponse;
    type KeyData = Key;

    fn make_rpc_request(&self, key: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_primary_key(key.into());
        req.set_lock_ts(self.lock_ts);
        req.set_caller_start_ts(self.caller_start_ts);
        req.set_current_ts(self.current_ts);
        req.set_rollback_if_not_exist(self.rollback_if_not_exist);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let key = mem::take(&mut self.primary_key);
        store_stream_for_key(key.into(), pd_client)
    }

    fn map_result(resp: Self::RpcResponse) -> Self::Result {
        resp.into()
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .await
    }
}

#[derive(Debug, Clone)]
pub struct TransactionStatus {
    pub kind: TransactionStatusKind,
    pub action: kvrpcpb::Action,
}

impl From<kvrpcpb::CheckTxnStatusResponse> for TransactionStatus {
    fn from(resp: kvrpcpb::CheckTxnStatusResponse) -> TransactionStatus {
        TransactionStatus {
            action: resp.get_action(),
            kind: (resp.commit_version, resp.lock_ttl, resp.lock_info).into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TransactionStatusKind {
    Committed(Timestamp),
    RolledBack,
    Locked(u64, kvrpcpb::LockInfo),
}

impl From<(u64, u64, Option<kvrpcpb::LockInfo>)> for TransactionStatusKind {
    fn from((ts, ttl, info): (u64, u64, Option<kvrpcpb::LockInfo>)) -> TransactionStatusKind {
        match (ts, ttl, info) {
            (0, 0, None) => TransactionStatusKind::RolledBack,
            (ts, 0, None) => TransactionStatusKind::Committed(Timestamp::from_version(ts)),
            (0, ttl, Some(info)) => TransactionStatusKind::Locked(ttl, info),
            _ => unreachable!(),
        }
    }
}

#[async_trait]
impl KvRequest for kvrpcpb::CheckSecondaryLocksRequest {
    type Result = SecondaryLocksStatus;
    type RpcResponse = kvrpcpb::CheckSecondaryLocksResponse;
    type KeyData = Vec<Key>;

    fn make_rpc_request(&self, keys: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_keys(keys.into_iter().map(Into::into).collect());
        req.set_start_version(self.start_version);

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        self.keys.sort();
        let keys = mem::take(&mut self.keys);
        store_stream_for_keys(keys, pd_client)
    }

    fn map_result(resp: Self::RpcResponse) -> Self::Result {
        SecondaryLocksStatus {
            locks: resp
                .locks
                .into_iter()
                .map(|l| (l.key.clone().into(), l))
                .collect(),
            commit_ts: Timestamp::try_from_version(resp.commit_ts),
        }
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .try_fold(
                SecondaryLocksStatus {
                    locks: HashMap::new(),
                    commit_ts: None,
                },
                |mut a, b| async move {
                    a.merge(b);
                    Ok(a)
                },
            )
            .await
    }
}

pub struct SecondaryLocksStatus {
    pub locks: HashMap<Key, kvrpcpb::LockInfo>,
    pub commit_ts: Option<Timestamp>,
}

impl SecondaryLocksStatus {
    fn merge(&mut self, other: SecondaryLocksStatus) {
        self.commit_ts = match (self.commit_ts.take(), other.commit_ts) {
            (Some(a), Some(b)) => {
                assert_eq!(a, b);
                Some(a)
            }
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        self.locks.extend(other.locks.into_iter());
    }
}

impl HasLocks for kvrpcpb::CommitResponse {}
impl HasLocks for kvrpcpb::CleanupResponse {}
impl HasLocks for kvrpcpb::BatchRollbackResponse {}
impl HasLocks for kvrpcpb::PessimisticRollbackResponse {}
impl HasLocks for kvrpcpb::ResolveLockResponse {}
impl HasLocks for kvrpcpb::ScanLockResponse {}
impl HasLocks for kvrpcpb::PessimisticLockResponse {}
impl HasLocks for kvrpcpb::TxnHeartBeatResponse {}
impl HasLocks for kvrpcpb::CheckTxnStatusResponse {}
impl HasLocks for kvrpcpb::CheckSecondaryLocksResponse {}
