// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::PdClient,
    request::{
        Collect, CollectWithShard, DefaultProcessor, KvRequest, Merge, Process, ResponseWithShard,
        Shardable, SingleKey,
    },
    store::{store_stream_for_keys, store_stream_for_range_by_start_key, Store},
    timestamp::TimestampExt,
    transaction::HasLocks,
    util::iter::FlatMapOkIterExt,
    Key, KvPair, Result, Value,
};
use either::Either;
use futures::stream::BoxStream;
use std::{collections::HashMap, iter, sync::Arc};
use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

// implement HasLocks for a response type that has a `pairs` field,
// where locks can be extracted from both the `pairs` and `error` fields
macro_rules! pair_locks {
    ($response_type:ty) => {
        impl HasLocks for $response_type {
            fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
                if self.pairs.is_empty() {
                    self.error
                        .as_mut()
                        .and_then(|error| error.locked.take())
                        .into_iter()
                        .collect()
                } else {
                    self.pairs
                        .iter_mut()
                        .filter_map(|pair| {
                            pair.error.as_mut().and_then(|error| error.locked.take())
                        })
                        .collect()
                }
            }
        }
    };
}

// implement HasLocks for a response type that does not have a `pairs` field,
// where locks are only extracted from the `error` field
macro_rules! error_locks {
    ($response_type:ty) => {
        impl HasLocks for $response_type {
            fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
                self.error
                    .as_mut()
                    .and_then(|error| error.locked.take())
                    .into_iter()
                    .collect()
            }
        }
    };
}

pub fn new_get_request(key: Vec<u8>, timestamp: u64) -> kvrpcpb::GetRequest {
    let mut req = kvrpcpb::GetRequest::default();
    req.set_key(key);
    req.set_version(timestamp);
    req
}

impl KvRequest for kvrpcpb::GetRequest {
    type Response = kvrpcpb::GetResponse;
}

impl SingleKey for kvrpcpb::GetRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

impl Process<kvrpcpb::GetResponse> for DefaultProcessor {
    type Out = Option<Value>;

    fn process(&self, input: Result<kvrpcpb::GetResponse>) -> Result<Self::Out> {
        let mut input = input?;
        Ok(if input.not_found {
            None
        } else {
            Some(input.take_value())
        })
    }
}

pub fn new_batch_get_request(keys: Vec<Vec<u8>>, timestamp: u64) -> kvrpcpb::BatchGetRequest {
    let mut req = kvrpcpb::BatchGetRequest::default();
    req.set_keys(keys);
    req.set_version(timestamp);
    req
}

impl KvRequest for kvrpcpb::BatchGetRequest {
    type Response = kvrpcpb::BatchGetResponse;
}

shardable_keys!(kvrpcpb::BatchGetRequest);

impl Merge<kvrpcpb::BatchGetResponse> for Collect {
    type Out = Vec<KvPair>;

    fn merge(&self, input: Vec<Result<kvrpcpb::BatchGetResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|mut resp| resp.take_pairs().into_iter().map(Into::into))
            .collect()
    }
}

pub fn new_scan_request(
    start_key: Vec<u8>,
    end_key: Vec<u8>,
    timestamp: u64,
    limit: u32,
    key_only: bool,
) -> kvrpcpb::ScanRequest {
    let mut req = kvrpcpb::ScanRequest::default();
    req.set_start_key(start_key);
    req.set_end_key(end_key);
    req.set_limit(limit);
    req.set_key_only(key_only);
    req.set_version(timestamp);
    req
}

impl KvRequest for kvrpcpb::ScanRequest {
    type Response = kvrpcpb::ScanResponse;
}

shardable_range!(kvrpcpb::ScanRequest);

impl Merge<kvrpcpb::ScanResponse> for Collect {
    type Out = Vec<KvPair>;

    fn merge(&self, input: Vec<Result<kvrpcpb::ScanResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|mut resp| resp.take_pairs().into_iter().map(Into::into))
            .collect()
    }
}

pub fn new_resolve_lock_request(
    start_version: u64,
    commit_version: u64,
) -> kvrpcpb::ResolveLockRequest {
    let mut req = kvrpcpb::ResolveLockRequest::default();
    req.set_start_version(start_version);
    req.set_commit_version(commit_version);

    req
}

impl KvRequest for kvrpcpb::ResolveLockRequest {
    type Response = kvrpcpb::ResolveLockResponse;
}

pub fn new_cleanup_request(key: Vec<u8>, start_version: u64) -> kvrpcpb::CleanupRequest {
    let mut req = kvrpcpb::CleanupRequest::default();
    req.set_key(key);
    req.set_start_version(start_version);

    req
}

impl KvRequest for kvrpcpb::CleanupRequest {
    type Response = kvrpcpb::CleanupResponse;
}

impl SingleKey for kvrpcpb::CleanupRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

impl Process<kvrpcpb::CleanupResponse> for DefaultProcessor {
    type Out = u64;

    fn process(&self, input: Result<kvrpcpb::CleanupResponse>) -> Result<Self::Out> {
        Ok(input?.commit_version)
    }
}

pub fn new_prewrite_request(
    mutations: Vec<kvrpcpb::Mutation>,
    primary_lock: Vec<u8>,
    start_version: u64,
    lock_ttl: u64,
) -> kvrpcpb::PrewriteRequest {
    let mut req = kvrpcpb::PrewriteRequest::default();
    req.set_mutations(mutations);
    req.set_primary_lock(primary_lock);
    req.set_start_version(start_version);
    req.set_lock_ttl(lock_ttl);
    // FIXME: Lite resolve lock is currently disabled
    req.set_txn_size(std::u64::MAX);

    req
}

pub fn new_pessimistic_prewrite_request(
    mutations: Vec<kvrpcpb::Mutation>,
    primary_lock: Vec<u8>,
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

impl KvRequest for kvrpcpb::PrewriteRequest {
    type Response = kvrpcpb::PrewriteResponse;
}

impl Shardable for kvrpcpb::PrewriteRequest {
    type Shard = Vec<kvrpcpb::Mutation>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, Store)>> {
        let mut mutations = self.mutations.clone();
        mutations.sort_by(|a, b| a.key.cmp(&b.key));
        store_stream_for_keys(mutations.into_iter(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()> {
        self.set_context(store.region.context()?);

        // Only need to set secondary keys if we're sending the primary key.
        if self.use_async_commit && !self.mutations.iter().any(|m| m.key == self.primary_lock) {
            self.set_secondaries(vec![]);
        }

        // Only if there is only one request to send
        if self.try_one_pc && shard.len() != self.secondaries.len() + 1 {
            self.set_try_one_pc(false);
        }

        self.set_mutations(shard);
        Ok(())
    }
}

pub fn new_commit_request(
    keys: Vec<Vec<u8>>,
    start_version: u64,
    commit_version: u64,
) -> kvrpcpb::CommitRequest {
    let mut req = kvrpcpb::CommitRequest::default();
    req.set_keys(keys);
    req.set_start_version(start_version);
    req.set_commit_version(commit_version);

    req
}

impl KvRequest for kvrpcpb::CommitRequest {
    type Response = kvrpcpb::CommitResponse;
}

shardable_keys!(kvrpcpb::CommitRequest);

pub fn new_batch_rollback_request(
    keys: Vec<Vec<u8>>,
    start_version: u64,
) -> kvrpcpb::BatchRollbackRequest {
    let mut req = kvrpcpb::BatchRollbackRequest::default();
    req.set_keys(keys);
    req.set_start_version(start_version);

    req
}

impl KvRequest for kvrpcpb::BatchRollbackRequest {
    type Response = kvrpcpb::BatchRollbackResponse;
}

shardable_keys!(kvrpcpb::BatchRollbackRequest);

pub fn new_pessimistic_rollback_request(
    keys: Vec<Vec<u8>>,
    start_version: u64,
    for_update_ts: u64,
) -> kvrpcpb::PessimisticRollbackRequest {
    let mut req = kvrpcpb::PessimisticRollbackRequest::default();
    req.set_keys(keys);
    req.set_start_version(start_version);
    req.set_for_update_ts(for_update_ts);

    req
}

impl KvRequest for kvrpcpb::PessimisticRollbackRequest {
    type Response = kvrpcpb::PessimisticRollbackResponse;
}

shardable_keys!(kvrpcpb::PessimisticRollbackRequest);

pub fn new_pessimistic_lock_request(
    mutations: Vec<kvrpcpb::Mutation>,
    primary_lock: Vec<u8>,
    start_version: u64,
    lock_ttl: u64,
    for_update_ts: u64,
    need_value: bool,
) -> kvrpcpb::PessimisticLockRequest {
    let mut req = kvrpcpb::PessimisticLockRequest::default();
    req.set_mutations(mutations);
    req.set_primary_lock(primary_lock);
    req.set_start_version(start_version);
    req.set_lock_ttl(lock_ttl);
    req.set_for_update_ts(for_update_ts);
    // FIXME: make them configurable
    req.set_is_first_lock(false);
    req.set_wait_timeout(0);
    req.set_force(false);
    req.set_return_values(need_value);
    // FIXME: support large transaction
    req.set_min_commit_ts(0);

    req
}

impl KvRequest for kvrpcpb::PessimisticLockRequest {
    type Response = kvrpcpb::PessimisticLockResponse;
}

impl Shardable for kvrpcpb::PessimisticLockRequest {
    type Shard = Vec<kvrpcpb::Mutation>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, Store)>> {
        let mut mutations = self.mutations.clone();
        mutations.sort_by(|a, b| a.key.cmp(&b.key));
        store_stream_for_keys(mutations.into_iter(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()> {
        self.set_context(store.region.context()?);
        self.set_mutations(shard);
        Ok(())
    }
}

// PessimisticLockResponse returns values that preserves the order with keys in request, thus the
// kvpair result should be produced by zipping the keys in request and the values in respponse.
impl Merge<ResponseWithShard<kvrpcpb::PessimisticLockResponse, Vec<kvrpcpb::Mutation>>>
    for CollectWithShard
{
    type Out = Vec<KvPair>;

    fn merge(
        &self,
        input: Vec<
            Result<ResponseWithShard<kvrpcpb::PessimisticLockResponse, Vec<kvrpcpb::Mutation>>>,
        >,
    ) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|ResponseWithShard(mut resp, mutations)| {
                let values = resp.take_values();
                let values_len = values.len();
                let not_founds = resp.take_not_founds();
                let kvparis = mutations
                    .into_iter()
                    .map(|m| m.key)
                    .zip(values)
                    .map(KvPair::from);
                assert_eq!(kvparis.len(), values_len);
                if not_founds.is_empty() {
                    // Legacy TiKV does not distiguish not existing key and existing key
                    // that with empty value. We assume that key does not exist if value
                    // is empty.
                    Either::Left(kvparis.filter(|kvpair| !kvpair.value().is_empty()))
                } else {
                    assert_eq!(kvparis.len(), not_founds.len());
                    Either::Right(kvparis.zip(not_founds).filter_map(|(kvpair, not_found)| {
                        if not_found {
                            None
                        } else {
                            Some(kvpair)
                        }
                    }))
                }
            })
            .collect()
    }
}

pub fn new_scan_lock_request(
    start_key: Vec<u8>,
    safepoint: u64,
    limit: u32,
) -> kvrpcpb::ScanLockRequest {
    let mut req = kvrpcpb::ScanLockRequest::default();
    req.set_start_key(start_key);
    req.set_max_version(safepoint);
    req.set_limit(limit);
    req
}

impl KvRequest for kvrpcpb::ScanLockRequest {
    type Response = kvrpcpb::ScanLockResponse;
}

impl Shardable for kvrpcpb::ScanLockRequest {
    type Shard = Vec<u8>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, Store)>> {
        store_stream_for_range_by_start_key(self.start_key.clone().into(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()> {
        self.set_context(store.region.context()?);
        self.set_start_key(shard);
        Ok(())
    }
}

impl Merge<kvrpcpb::ScanLockResponse> for Collect {
    type Out = Vec<kvrpcpb::LockInfo>;

    fn merge(&self, input: Vec<Result<kvrpcpb::ScanLockResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|mut resp| resp.take_locks().into_iter().map(Into::into))
            .collect()
    }
}

pub fn new_heart_beat_request(
    start_ts: u64,
    primary_lock: Vec<u8>,
    ttl: u64,
) -> kvrpcpb::TxnHeartBeatRequest {
    let mut req = kvrpcpb::TxnHeartBeatRequest::default();
    req.set_start_version(start_ts);
    req.set_primary_lock(primary_lock);
    req.set_advise_lock_ttl(ttl);
    req
}

impl KvRequest for kvrpcpb::TxnHeartBeatRequest {
    type Response = kvrpcpb::TxnHeartBeatResponse;
}

impl SingleKey for kvrpcpb::TxnHeartBeatRequest {
    fn key(&self) -> &Vec<u8> {
        &self.primary_lock
    }
}

impl Process<kvrpcpb::TxnHeartBeatResponse> for DefaultProcessor {
    type Out = u64;

    fn process(&self, input: Result<kvrpcpb::TxnHeartBeatResponse>) -> Result<Self::Out> {
        Ok(input?.lock_ttl)
    }
}

impl KvRequest for kvrpcpb::CheckTxnStatusRequest {
    type Response = kvrpcpb::CheckTxnStatusResponse;
}

impl SingleKey for kvrpcpb::CheckTxnStatusRequest {
    fn key(&self) -> &Vec<u8> {
        &self.primary_key
    }
}

impl Process<kvrpcpb::CheckTxnStatusResponse> for DefaultProcessor {
    type Out = TransactionStatus;

    fn process(&self, input: Result<kvrpcpb::CheckTxnStatusResponse>) -> Result<Self::Out> {
        Ok(input?.into())
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

impl KvRequest for kvrpcpb::CheckSecondaryLocksRequest {
    type Response = kvrpcpb::CheckSecondaryLocksResponse;
}

shardable_keys!(kvrpcpb::CheckSecondaryLocksRequest);

impl Merge<kvrpcpb::CheckSecondaryLocksResponse> for Collect {
    type Out = SecondaryLocksStatus;

    fn merge(&self, input: Vec<Result<kvrpcpb::CheckSecondaryLocksResponse>>) -> Result<Self::Out> {
        let mut out = SecondaryLocksStatus {
            locks: HashMap::new(),
            commit_ts: None,
        };
        for resp in input {
            let resp = resp?;
            out.locks
                .extend(resp.locks.into_iter().map(|l| (l.key.clone().into(), l)));
            out.commit_ts = match (
                out.commit_ts.take(),
                Timestamp::try_from_version(resp.commit_ts),
            ) {
                (Some(a), Some(b)) => {
                    assert_eq!(a, b);
                    Some(a)
                }
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
        }
        Ok(out)
    }
}

pub struct SecondaryLocksStatus {
    pub locks: HashMap<Key, kvrpcpb::LockInfo>,
    pub commit_ts: Option<Timestamp>,
}

pair_locks!(kvrpcpb::BatchGetResponse);
pair_locks!(kvrpcpb::ScanResponse);
error_locks!(kvrpcpb::GetResponse);
error_locks!(kvrpcpb::ResolveLockResponse);
error_locks!(kvrpcpb::CommitResponse);
error_locks!(kvrpcpb::BatchRollbackResponse);
error_locks!(kvrpcpb::TxnHeartBeatResponse);
error_locks!(kvrpcpb::CheckTxnStatusResponse);
error_locks!(kvrpcpb::CheckSecondaryLocksResponse);

impl HasLocks for kvrpcpb::CleanupResponse {}
impl HasLocks for kvrpcpb::ScanLockResponse {}

impl HasLocks for kvrpcpb::PessimisticRollbackResponse {
    fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
        self.errors
            .iter_mut()
            .filter_map(|error| error.locked.take())
            .collect()
    }
}

impl HasLocks for kvrpcpb::PessimisticLockResponse {
    fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
        self.errors
            .iter_mut()
            .filter_map(|error| error.locked.take())
            .collect()
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
