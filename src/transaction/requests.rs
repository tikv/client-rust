// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cmp;
use std::collections::HashSet;
use std::iter;
use std::sync::Arc;

use either::Either;
use futures::stream::BoxStream;
use futures::stream::{self};
use futures::StreamExt;

use super::transaction::TXN_COMMIT_BATCH_SIZE;
use crate::collect_single;
use crate::common::Error::PessimisticLockError;
use crate::pd::PdClient;
use crate::proto::kvrpcpb::Action;
use crate::proto::kvrpcpb::LockInfo;
use crate::proto::kvrpcpb::TxnHeartBeatResponse;
use crate::proto::kvrpcpb::TxnInfo;
use crate::proto::kvrpcpb::{self};
use crate::proto::pdpb::Timestamp;
use crate::region::RegionWithLeader;
use crate::request::Collect;
use crate::request::CollectSingle;
use crate::request::CollectWithShard;
use crate::request::DefaultProcessor;
use crate::request::HasNextBatch;
use crate::request::KvRequest;
use crate::request::Merge;
use crate::request::NextBatch;
use crate::request::Process;
use crate::request::RangeRequest;
use crate::request::ResponseWithShard;
use crate::request::Shardable;
use crate::request::SingleKey;
use crate::request::{Batchable, StoreRequest};
use crate::reversible_range_request;
use crate::shardable_key;
use crate::shardable_keys;
use crate::shardable_range;
use crate::store::RegionStore;
use crate::store::Request;
use crate::store::Store;
use crate::store::{region_stream_for_keys, region_stream_for_range};
use crate::timestamp::TimestampExt;
use crate::transaction::lock::format_key_for_log;
use crate::transaction::requests::kvrpcpb::prewrite_request::PessimisticAction;
use crate::transaction::HasLocks;
use crate::util::iter::FlatMapOkIterExt;
use crate::Error;
use crate::KvPair;
use crate::Result;
use crate::Value;

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
    req.key = key;
    req.version = timestamp;
    req
}

impl KvRequest for kvrpcpb::GetRequest {
    type Response = kvrpcpb::GetResponse;
}

shardable_key!(kvrpcpb::GetRequest);
collect_single!(kvrpcpb::GetResponse);
impl SingleKey for kvrpcpb::GetRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

impl Process<kvrpcpb::GetResponse> for DefaultProcessor {
    type Out = Option<Value>;

    fn process(&self, input: Result<kvrpcpb::GetResponse>) -> Result<Self::Out> {
        let input = input?;
        Ok(if input.not_found {
            None
        } else {
            Some(input.value)
        })
    }
}

pub fn new_batch_get_request(keys: Vec<Vec<u8>>, timestamp: u64) -> kvrpcpb::BatchGetRequest {
    let mut req = kvrpcpb::BatchGetRequest::default();
    req.keys = keys;
    req.version = timestamp;
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
            .flat_map_ok(|resp| resp.pairs.into_iter().map(Into::into))
            .collect()
    }
}

pub fn new_scan_request(
    start_key: Vec<u8>,
    end_key: Vec<u8>,
    timestamp: u64,
    limit: u32,
    key_only: bool,
    reverse: bool,
) -> kvrpcpb::ScanRequest {
    let mut req = kvrpcpb::ScanRequest::default();
    if !reverse {
        req.start_key = start_key;
        req.end_key = end_key;
    } else {
        req.start_key = end_key;
        req.end_key = start_key;
    }
    req.limit = limit;
    req.key_only = key_only;
    req.version = timestamp;
    req.reverse = reverse;
    req
}

impl KvRequest for kvrpcpb::ScanRequest {
    type Response = kvrpcpb::ScanResponse;
}

reversible_range_request!(kvrpcpb::ScanRequest);
shardable_range!(kvrpcpb::ScanRequest);

impl Merge<kvrpcpb::ScanResponse> for Collect {
    type Out = Vec<KvPair>;

    fn merge(&self, input: Vec<Result<kvrpcpb::ScanResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|resp| resp.pairs.into_iter().map(Into::into))
            .collect()
    }
}

pub fn new_resolve_lock_request(
    start_version: u64,
    commit_version: u64,
    is_txn_file: bool,
) -> kvrpcpb::ResolveLockRequest {
    let mut req = kvrpcpb::ResolveLockRequest::default();
    req.start_version = start_version;
    req.commit_version = commit_version;
    req.is_txn_file = is_txn_file;
    req
}

pub fn new_batch_resolve_lock_request(txn_infos: Vec<TxnInfo>) -> kvrpcpb::ResolveLockRequest {
    let mut req = kvrpcpb::ResolveLockRequest::default();
    req.txn_infos = txn_infos;
    req
}

// Note: ResolveLockRequest is a special one: it can be sent to a specified
// region without keys. So it's not Shardable. And we don't automatically retry
// on its region errors (in the Plan level). The region error must be manually
// handled (in the upper level).
impl KvRequest for kvrpcpb::ResolveLockRequest {
    type Response = kvrpcpb::ResolveLockResponse;
}

pub fn new_prewrite_request(
    mutations: Vec<kvrpcpb::Mutation>,
    primary_lock: Vec<u8>,
    start_version: u64,
    lock_ttl: u64,
) -> kvrpcpb::PrewriteRequest {
    let mut req = kvrpcpb::PrewriteRequest::default();
    req.mutations = mutations;
    req.primary_lock = primary_lock;
    req.start_version = start_version;
    req.lock_ttl = lock_ttl;
    // FIXME: Lite resolve lock is currently disabled
    req.txn_size = u64::MAX;

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
    req.for_update_ts = for_update_ts;
    req.pessimistic_actions =
        iter::repeat_n(PessimisticAction::DoPessimisticCheck.into(), len).collect();
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
    ) -> BoxStream<'static, Result<(Self::Shard, RegionWithLeader)>> {
        let mut mutations = self.mutations.clone();
        mutations.sort_by(|a, b| a.key.cmp(&b.key));

        region_stream_for_keys(mutations.into_iter(), pd_client.clone())
            .flat_map(|result| match result {
                Ok((mutations, region)) => stream::iter(kvrpcpb::PrewriteRequest::batches(
                    mutations,
                    TXN_COMMIT_BATCH_SIZE,
                ))
                .map(move |batch| Ok((batch, region.clone())))
                .boxed(),
                Err(e) => stream::iter(Err(e)).boxed(),
            })
            .boxed()
    }

    fn apply_shard(&mut self, shard: Self::Shard) {
        // Only need to set secondary keys if we're sending the primary key.
        if self.use_async_commit && !self.mutations.iter().any(|m| m.key == self.primary_lock) {
            self.secondaries = vec![];
        }

        // Only if there is only one request to send
        if self.try_one_pc && shard.len() != self.secondaries.len() + 1 {
            self.try_one_pc = false;
        }

        self.mutations = shard;
    }

    fn apply_store(&mut self, store: &RegionStore) -> Result<()> {
        self.set_leader(&store.region_with_leader)
    }
}

impl Batchable for kvrpcpb::PrewriteRequest {
    type Item = kvrpcpb::Mutation;

    fn item_size(item: &Self::Item) -> u64 {
        let mut size = item.key.len() as u64;
        size += item.value.len() as u64;
        size
    }
}

pub fn new_commit_request(
    keys: Vec<Vec<u8>>,
    start_version: u64,
    commit_version: u64,
) -> kvrpcpb::CommitRequest {
    let mut req = kvrpcpb::CommitRequest::default();
    req.keys = keys;
    req.start_version = start_version;
    req.commit_version = commit_version;

    req
}

impl KvRequest for kvrpcpb::CommitRequest {
    type Response = kvrpcpb::CommitResponse;
}

impl Shardable for kvrpcpb::CommitRequest {
    type Shard = Vec<Vec<u8>>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionWithLeader)>> {
        let mut keys = self.keys.clone();
        keys.sort();

        region_stream_for_keys(keys.into_iter(), pd_client.clone())
            .flat_map(|result| match result {
                Ok((keys, region)) => {
                    stream::iter(kvrpcpb::CommitRequest::batches(keys, TXN_COMMIT_BATCH_SIZE))
                        .map(move |batch| Ok((batch, region.clone())))
                        .boxed()
                }
                Err(e) => stream::iter(Err(e)).boxed(),
            })
            .boxed()
    }

    fn apply_shard(&mut self, shard: Self::Shard) {
        self.keys = shard;
    }

    fn apply_store(&mut self, store: &RegionStore) -> Result<()> {
        self.set_leader(&store.region_with_leader)
    }
}

impl Batchable for kvrpcpb::CommitRequest {
    type Item = Vec<u8>;

    fn item_size(item: &Self::Item) -> u64 {
        item.len() as u64
    }
}

pub fn new_batch_rollback_request(
    keys: Vec<Vec<u8>>,
    start_version: u64,
) -> kvrpcpb::BatchRollbackRequest {
    let mut req = kvrpcpb::BatchRollbackRequest::default();
    req.keys = keys;
    req.start_version = start_version;

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
    req.keys = keys;
    req.start_version = start_version;
    req.for_update_ts = for_update_ts;

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
    req.mutations = mutations;
    req.primary_lock = primary_lock;
    req.start_version = start_version;
    req.lock_ttl = lock_ttl;
    req.for_update_ts = for_update_ts;
    // FIXME: make them configurable
    req.is_first_lock = false;
    req.wait_timeout = 0;
    req.return_values = need_value;
    // FIXME: support large transaction
    req.min_commit_ts = 0;

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
    ) -> BoxStream<'static, Result<(Self::Shard, RegionWithLeader)>> {
        let mut mutations = self.mutations.clone();
        mutations.sort_by(|a, b| a.key.cmp(&b.key));
        region_stream_for_keys(mutations.into_iter(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard) {
        self.mutations = shard;
    }

    fn apply_store(&mut self, store: &RegionStore) -> Result<()> {
        self.set_leader(&store.region_with_leader)
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
        if input.iter().any(Result::is_err) {
            let (success, mut errors): (Vec<_>, Vec<_>) =
                input.into_iter().partition(Result::is_ok);
            let first_err = errors.pop().unwrap();
            let success_keys = success
                .into_iter()
                .map(Result::unwrap)
                .flat_map(|ResponseWithShard(_resp, mutations)| {
                    mutations.into_iter().map(|m| m.key)
                })
                .collect();
            Err(PessimisticLockError {
                inner: Box::new(first_err.unwrap_err()),
                success_keys,
            })
        } else {
            Ok(input
                .into_iter()
                .map(Result::unwrap)
                .flat_map(|ResponseWithShard(resp, mutations)| {
                    let values: Vec<Vec<u8>> = resp.values;
                    let values_len = values.len();
                    let not_founds = resp.not_founds;
                    let kvpairs = mutations
                        .into_iter()
                        .map(|m| m.key)
                        .zip(values)
                        .map(KvPair::from);
                    assert_eq!(kvpairs.len(), values_len);
                    if not_founds.is_empty() {
                        // Legacy TiKV does not distinguish not existing key and existing key
                        // that with empty value. We assume that key does not exist if value
                        // is empty.
                        Either::Left(kvpairs.filter(|kvpair| !kvpair.value().is_empty()))
                    } else {
                        assert_eq!(kvpairs.len(), not_founds.len());
                        Either::Right(kvpairs.zip(not_founds).filter_map(|(kvpair, not_found)| {
                            if not_found {
                                None
                            } else {
                                Some(kvpair)
                            }
                        }))
                    }
                })
                .collect())
        }
    }
}

pub fn new_scan_lock_request(
    start_key: Vec<u8>,
    end_key: Vec<u8>,
    safepoint: u64,
    limit: u32,
) -> kvrpcpb::ScanLockRequest {
    let mut req = kvrpcpb::ScanLockRequest::default();
    req.start_key = start_key;
    req.end_key = end_key;
    req.max_version = safepoint;
    req.limit = limit;
    req
}

impl KvRequest for kvrpcpb::ScanLockRequest {
    type Response = kvrpcpb::ScanLockResponse;
}

impl Shardable for kvrpcpb::ScanLockRequest {
    type Shard = (Vec<u8>, Vec<u8>);

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionWithLeader)>> {
        region_stream_for_range(
            (self.start_key.clone(), self.end_key.clone()),
            pd_client.clone(),
        )
    }

    fn apply_shard(&mut self, shard: Self::Shard) {
        self.start_key = shard.0;
    }

    fn apply_store(&mut self, store: &RegionStore) -> Result<()> {
        self.set_leader(&store.region_with_leader)
    }
}

impl HasNextBatch for kvrpcpb::ScanLockResponse {
    fn has_next_batch(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        self.locks.last().map(|lock| {
            // TODO: if last key is larger or equal than ScanLockRequest.end_key, return None.
            let mut start_key: Vec<u8> = lock.key.clone();
            start_key.push(0);
            (start_key, vec![])
        })
    }
}

impl NextBatch for kvrpcpb::ScanLockRequest {
    fn next_batch(&mut self, range: (Vec<u8>, Vec<u8>)) {
        self.start_key = range.0;
    }
}

impl Merge<kvrpcpb::ScanLockResponse> for Collect {
    type Out = Vec<kvrpcpb::LockInfo>;

    fn merge(&self, input: Vec<Result<kvrpcpb::ScanLockResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|mut resp| resp.take_locks().into_iter())
            .collect()
    }
}

pub fn new_heart_beat_request(
    start_ts: u64,
    primary_lock: Vec<u8>,
    ttl: u64,
) -> kvrpcpb::TxnHeartBeatRequest {
    let mut req = kvrpcpb::TxnHeartBeatRequest::default();
    req.start_version = start_ts;
    req.primary_lock = primary_lock;
    req.advise_lock_ttl = ttl;
    req
}

impl KvRequest for kvrpcpb::TxnHeartBeatRequest {
    type Response = kvrpcpb::TxnHeartBeatResponse;
}

impl Shardable for kvrpcpb::TxnHeartBeatRequest {
    type Shard = Vec<Vec<u8>>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionWithLeader)>> {
        region_stream_for_keys(std::iter::once(self.key().clone()), pd_client.clone())
    }

    fn apply_shard(&mut self, mut shard: Self::Shard) {
        assert!(shard.len() == 1);
        self.primary_lock = shard.pop().unwrap();
    }

    fn apply_store(&mut self, store: &RegionStore) -> Result<()> {
        self.set_leader(&store.region_with_leader)
    }
}

collect_single!(TxnHeartBeatResponse);

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

#[allow(clippy::too_many_arguments)]
pub fn new_check_txn_status_request(
    primary_key: Vec<u8>,
    lock_ts: u64,
    caller_start_ts: u64,
    current_ts: u64,
    rollback_if_not_exist: bool,
    force_sync_commit: bool,
    resolving_pessimistic_lock: bool,
    is_txn_file: bool,
) -> kvrpcpb::CheckTxnStatusRequest {
    let mut req = kvrpcpb::CheckTxnStatusRequest::default();
    req.primary_key = primary_key;
    req.lock_ts = lock_ts;
    req.caller_start_ts = caller_start_ts;
    req.current_ts = current_ts;
    req.rollback_if_not_exist = rollback_if_not_exist;
    req.force_sync_commit = force_sync_commit;
    req.resolving_pessimistic_lock = resolving_pessimistic_lock;
    req.verify_is_primary = true;
    req.is_txn_file = is_txn_file;
    req
}

impl KvRequest for kvrpcpb::CheckTxnStatusRequest {
    type Response = kvrpcpb::CheckTxnStatusResponse;
}

impl Shardable for kvrpcpb::CheckTxnStatusRequest {
    type Shard = Vec<Vec<u8>>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionWithLeader)>> {
        region_stream_for_keys(std::iter::once(self.key().clone()), pd_client.clone())
    }

    fn apply_shard(&mut self, mut shard: Self::Shard) {
        assert!(shard.len() == 1);
        self.primary_key = shard.pop().unwrap();
    }

    fn apply_store(&mut self, store: &RegionStore) -> Result<()> {
        self.set_leader(&store.region_with_leader)
    }
}

impl SingleKey for kvrpcpb::CheckTxnStatusRequest {
    fn key(&self) -> &Vec<u8> {
        &self.primary_key
    }
}

collect_single!(kvrpcpb::CheckTxnStatusResponse);

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
    pub is_expired: bool, // Available only when kind is Locked.
}

impl From<kvrpcpb::CheckTxnStatusResponse> for TransactionStatus {
    fn from(mut resp: kvrpcpb::CheckTxnStatusResponse) -> TransactionStatus {
        TransactionStatus {
            action: Action::try_from(resp.action).unwrap(),
            kind: (resp.commit_version, resp.lock_ttl, resp.lock_info.take()).into(),
            is_expired: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TransactionStatusKind {
    Committed(Timestamp),
    RolledBack,
    Locked(u64, kvrpcpb::LockInfo), // None of ttl means expired.
}

impl TransactionStatus {
    pub fn check_ttl(&mut self, current: Timestamp) {
        if let TransactionStatusKind::Locked(ref ttl, ref lock_info) = self.kind {
            if current.physical - Timestamp::from_version(lock_info.lock_version).physical
                >= *ttl as i64
            {
                self.is_expired = true
            }
        }
    }

    // Only final states are cacheable. A Locked result is not final even when its TTL expired:
    // async-commit recovery still has to inspect every secondary, and force-sync fallback must be
    // able to issue a fresh CheckTxnStatus request.
    pub fn is_cacheable(&self) -> bool {
        matches!(
            self.kind,
            TransactionStatusKind::RolledBack | TransactionStatusKind::Committed(..)
        )
    }
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

pub fn new_check_secondary_locks_request(
    keys: Vec<Vec<u8>>,
    start_version: u64,
) -> kvrpcpb::CheckSecondaryLocksRequest {
    let mut req = kvrpcpb::CheckSecondaryLocksRequest::default();
    req.keys = keys;
    req.start_version = start_version;
    req
}

impl KvRequest for kvrpcpb::CheckSecondaryLocksRequest {
    type Response = kvrpcpb::CheckSecondaryLocksResponse;
}

shardable_keys!(kvrpcpb::CheckSecondaryLocksRequest);

/// Merge the per-region responses of a sharded `CheckSecondaryLocks` request. Each shard
/// (`Vec<Vec<u8>>`) is the list of secondary keys sent to one region; pairing every response
/// with its own key list is what makes missing-lock detection possible — TiKV only returns
/// the locks it found, never the keys that no longer hold one.
impl Merge<ResponseWithShard<kvrpcpb::CheckSecondaryLocksResponse, Vec<Vec<u8>>>> for Collect {
    type Out = SecondaryLocksStatus;

    fn merge(
        &self,
        input: Vec<Result<ResponseWithShard<kvrpcpb::CheckSecondaryLocksResponse, Vec<Vec<u8>>>>>,
    ) -> Result<Self::Out> {
        let mut out = SecondaryLocksStatus {
            commit_ts: None,
            min_commit_ts: 0,
            fallback_2pc: false,
            missing_lock: false,
            locks: vec![],
        };

        for resp in input {
            let ResponseWithShard(resp, requested_keys) = resp?;
            if resp.locks.len() > requested_keys.len() {
                return Err(Error::ProtocolViolation {
                    message: format!(
                        "CheckSecondaryLocks returned {} locks for {} requested keys",
                        resp.locks.len(),
                        requested_keys.len()
                    ),
                });
            }

            // TiKV checks the requested keys one by one and stops at the first key that no
            // longer holds a lock of this transaction, making the transaction's fate durable
            // on the way: unless that key is already committed, a protected rollback is
            // written for it. The decision is reported through `commit_ts` — the commit TS,
            // or zero for a rollback — and the returned locks then no longer cover every
            // requested key. A short lock list therefore means the transaction is decided.
            let response_missing_lock = resp.locks.len() < requested_keys.len();
            if !response_missing_lock && resp.commit_ts != 0 {
                return Err(Error::ProtocolViolation {
                    message: format!(
                        "CheckSecondaryLocks returned commit TS {} although every requested lock is still present",
                        resp.commit_ts
                    ),
                });
            }

            let requested: HashSet<&Vec<u8>> = requested_keys.iter().collect();
            let mut seen_keys = HashSet::with_capacity(resp.locks.len());
            for lock in &resp.locks {
                if !requested.contains(&lock.key) {
                    return Err(Error::ProtocolViolation {
                        message: format!(
                            "CheckSecondaryLocks returned a lock ({}) that was not requested",
                            format_key_for_log(&lock.key)
                        ),
                    });
                }
                if !seen_keys.insert(lock.key.clone()) {
                    return Err(Error::ProtocolViolation {
                        message: format!(
                            "CheckSecondaryLocks returned key ({}) more than once",
                            format_key_for_log(&lock.key)
                        ),
                    });
                }
                if !lock.use_async_commit {
                    out.fallback_2pc = true;
                }
                out.min_commit_ts = cmp::max(out.min_commit_ts, lock.min_commit_ts);
            }

            if response_missing_lock {
                let response_commit_ts = Timestamp::try_from_version(resp.commit_ts);
                if out.missing_lock && out.commit_ts != response_commit_ts {
                    return Err(Error::ProtocolViolation {
                        message: format!(
                            "CheckSecondaryLocks reported two different commit TS ({} and {}) for one transaction",
                            out.commit_ts.as_ref().map_or(0, TimestampExt::version),
                            resp.commit_ts
                        ),
                    });
                }
                out.missing_lock = true;
                out.commit_ts = response_commit_ts;
            }

            out.locks.extend(resp.locks);
        }

        Ok(out)
    }
}

/// The aggregated outcome of `CheckSecondaryLocks` over the secondary keys of an
/// async-commit transaction.
pub struct SecondaryLocksStatus {
    /// The decision TiKV reported after finding a requested key without a lock:
    /// `Some` when the transaction is committed, `None` when it was rolled back.
    /// Only meaningful while `missing_lock` is true.
    pub commit_ts: Option<Timestamp>,
    /// The maximum `min_commit_ts` across the locks that are still alive.
    pub min_commit_ts: u64,
    /// True when a surviving lock fell back from async commit to 2PC: the transaction's
    /// fate then belongs to its primary lock, not to the secondaries.
    pub fallback_2pc: bool,
    /// True when at least one requested key no longer holds a lock — TiKV has already
    /// decided the transaction and made the decision durable; `commit_ts` carries it.
    pub missing_lock: bool,
    /// Every lock returned by TiKV, for further validation by the caller.
    pub locks: Vec<kvrpcpb::LockInfo>,
}

impl SecondaryLocksStatus {
    /// The version this transaction must be resolved with: a positive commit version to
    /// commit, or zero to roll back — the same encoding `TxnInfo.status` uses on the wire.
    ///
    /// While every lock is still alive the transaction is committable, and the commit
    /// version is the maximum `min_commit_ts` across the primary and all secondary locks —
    /// exactly the value the transaction's own committer would compute. Once a lock is
    /// missing, TiKV has already made the decision durable and `commit_ts` carries it.
    ///
    /// Returns an error when TiKV reports a commit TS below a surviving lock's
    /// `min_commit_ts`: every lock promised its readers no commit below that point.
    pub fn resolved_commit_version(&self, primary_min_commit_ts: u64) -> Result<u64> {
        let min_commit_ts = cmp::max(primary_min_commit_ts, self.min_commit_ts);
        if !self.missing_lock {
            return Ok(min_commit_ts);
        }

        let commit_version = self.commit_ts.as_ref().map_or(0, TimestampExt::version);
        if commit_version != 0 && commit_version < min_commit_ts {
            return Err(Error::ProtocolViolation {
                message: format!(
                    "CheckSecondaryLocks reported commit TS {} below a surviving lock's min_commit_ts {}",
                    commit_version, min_commit_ts
                ),
            });
        }
        Ok(commit_version)
    }
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

impl HasLocks for kvrpcpb::ScanLockResponse {
    fn take_locks(&mut self) -> Vec<LockInfo> {
        std::mem::take(&mut self.locks)
    }
}

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

pub fn new_unsafe_destroy_range_request(
    start_key: Vec<u8>,
    end_key: Vec<u8>,
) -> kvrpcpb::UnsafeDestroyRangeRequest {
    let mut req = kvrpcpb::UnsafeDestroyRangeRequest::default();
    req.start_key = start_key;
    req.end_key = end_key;
    req
}

impl KvRequest for kvrpcpb::UnsafeDestroyRangeRequest {
    type Response = kvrpcpb::UnsafeDestroyRangeResponse;
}

impl StoreRequest for kvrpcpb::UnsafeDestroyRangeRequest {
    fn apply_store(&mut self, _store: &Store) {}
}

impl HasLocks for kvrpcpb::UnsafeDestroyRangeResponse {}

impl Merge<kvrpcpb::UnsafeDestroyRangeResponse> for Collect {
    type Out = ();

    fn merge(&self, input: Vec<Result<kvrpcpb::UnsafeDestroyRangeResponse>>) -> Result<Self::Out> {
        let _: Vec<kvrpcpb::UnsafeDestroyRangeResponse> =
            input.into_iter().collect::<Result<Vec<_>>>()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::common::Error;
    use crate::common::Error::PessimisticLockError;
    use crate::common::Error::ResolveLockError;
    use crate::proto::kvrpcpb;
    use crate::proto::pdpb::Timestamp;
    use crate::request::plan::Merge;
    use crate::request::Collect;
    use crate::request::CollectWithShard;
    use crate::request::ResponseWithShard;
    use crate::timestamp::TimestampExt;
    use crate::KvPair;

    use super::TransactionStatus;
    use super::TransactionStatusKind;

    /// A still-live async-commit lock, as returned inside `CheckSecondaryLocksResponse`.
    fn async_commit_lock(key: &[u8], min_commit_ts: u64) -> kvrpcpb::LockInfo {
        kvrpcpb::LockInfo {
            key: key.to_vec(),
            use_async_commit: true,
            min_commit_ts,
            ..Default::default()
        }
    }

    #[test]
    fn check_secondary_conflicting_commit_ts_is_a_protocol_violation() {
        let result = Collect.merge(vec![
            Ok(ResponseWithShard(
                kvrpcpb::CheckSecondaryLocksResponse {
                    commit_ts: 7,
                    ..Default::default()
                },
                vec![b"a".to_vec()],
            )),
            Ok(ResponseWithShard(
                kvrpcpb::CheckSecondaryLocksResponse {
                    commit_ts: 8,
                    ..Default::default()
                },
                vec![b"b".to_vec()],
            )),
        ]);

        assert!(matches!(result, Err(Error::ProtocolViolation { .. })));
    }

    #[test]
    fn check_secondary_rejects_more_locks_than_requested_keys() {
        let result = Collect.merge(vec![Ok(ResponseWithShard(
            kvrpcpb::CheckSecondaryLocksResponse {
                locks: vec![async_commit_lock(b"a", 1), async_commit_lock(b"b", 2)],
                ..Default::default()
            },
            vec![b"a".to_vec()],
        ))]);

        assert!(matches!(result, Err(Error::ProtocolViolation { .. })));
    }

    #[test]
    fn check_secondary_rejects_a_lock_that_was_not_requested() {
        let result = Collect.merge(vec![Ok(ResponseWithShard(
            kvrpcpb::CheckSecondaryLocksResponse {
                locks: vec![async_commit_lock(b"b", 1)],
                ..Default::default()
            },
            vec![b"a".to_vec()],
        ))]);

        assert!(matches!(result, Err(Error::ProtocolViolation { .. })));
    }

    #[test]
    fn check_secondary_rejects_a_duplicate_lock_key() {
        let result = Collect.merge(vec![Ok(ResponseWithShard(
            kvrpcpb::CheckSecondaryLocksResponse {
                locks: vec![async_commit_lock(b"a", 1), async_commit_lock(b"a", 2)],
                ..Default::default()
            },
            vec![b"a".to_vec(), b"b".to_vec()],
        ))]);

        assert!(matches!(result, Err(Error::ProtocolViolation { .. })));
    }

    #[test]
    fn check_secondary_all_locks_present_uses_max_min_commit_ts() {
        let result = Collect
            .merge(vec![Ok(ResponseWithShard(
                kvrpcpb::CheckSecondaryLocksResponse {
                    locks: vec![async_commit_lock(b"secondary", 70)],
                    ..Default::default()
                },
                vec![b"secondary".to_vec()],
            ))])
            .unwrap();

        assert!(!result.missing_lock);
        assert_eq!(result.resolved_commit_version(80).unwrap(), 80);
    }

    #[test]
    fn check_secondary_missing_lock_preserves_exact_commit_ts() {
        let result = Collect
            .merge(vec![Ok(ResponseWithShard(
                kvrpcpb::CheckSecondaryLocksResponse {
                    commit_ts: 77,
                    ..Default::default()
                },
                vec![b"missing".to_vec()],
            ))])
            .unwrap();

        assert!(result.missing_lock);
        assert_eq!(result.commit_ts.unwrap().version(), 77);
    }

    #[test]
    fn check_secondary_missing_lock_with_zero_commit_ts_resolves_as_rollback() {
        let result = Collect
            .merge(vec![Ok(ResponseWithShard(
                // No lock and no commit TS: TiKV wrote a protected rollback for the key.
                kvrpcpb::CheckSecondaryLocksResponse::default(),
                vec![b"missing".to_vec()],
            ))])
            .unwrap();

        assert!(result.missing_lock);
        assert_eq!(result.commit_ts, None);
        assert_eq!(result.resolved_commit_version(80).unwrap(), 0);
    }

    #[test]
    fn check_secondary_rejects_commit_ts_below_primary_min_commit_ts() {
        let result = Collect
            .merge(vec![Ok(ResponseWithShard(
                kvrpcpb::CheckSecondaryLocksResponse {
                    commit_ts: 77,
                    ..Default::default()
                },
                vec![b"missing".to_vec()],
            ))])
            .unwrap();

        assert!(matches!(
            result.resolved_commit_version(80),
            Err(Error::ProtocolViolation { .. })
        ));
    }

    #[test]
    fn check_secondary_rejects_commit_ts_below_locked_min_commit_ts() {
        let result = Collect
            .merge(vec![
                Ok(ResponseWithShard(
                    kvrpcpb::CheckSecondaryLocksResponse {
                        locks: vec![async_commit_lock(b"locked", 80)],
                        ..Default::default()
                    },
                    vec![b"locked".to_vec()],
                )),
                Ok(ResponseWithShard(
                    kvrpcpb::CheckSecondaryLocksResponse {
                        commit_ts: 77,
                        ..Default::default()
                    },
                    vec![b"missing".to_vec()],
                )),
            ])
            .unwrap();

        // The merge only aggregates; the min_commit_ts gate lives in
        // `resolved_commit_version`, which every commit-path caller goes through.
        assert!(matches!(
            result.resolved_commit_version(0),
            Err(Error::ProtocolViolation { .. })
        ));
    }

    #[test]
    fn only_final_transaction_statuses_are_cacheable() {
        let committed = TransactionStatus {
            kind: TransactionStatusKind::Committed(Timestamp::from_version(5)),
            action: kvrpcpb::Action::NoAction,
            is_expired: false,
        };
        assert!(committed.is_cacheable());

        let rolled_back = TransactionStatus {
            kind: TransactionStatusKind::RolledBack,
            action: kvrpcpb::Action::NoAction,
            is_expired: false,
        };
        assert!(rolled_back.is_cacheable());

        // A `Locked` status is a snapshot, never a fact — not even once the TTL has
        // expired: async-commit recovery must inspect the secondaries afresh, and the
        // force-sync fallback must be able to issue a new CheckTxnStatus request.
        let expired_async_commit_lock = TransactionStatus {
            kind: TransactionStatusKind::Locked(
                1,
                kvrpcpb::LockInfo {
                    use_async_commit: true,
                    ..Default::default()
                },
            ),
            action: kvrpcpb::Action::NoAction,
            is_expired: true,
        };
        assert!(!expired_async_commit_lock.is_cacheable());
    }

    #[tokio::test]
    async fn test_merge_pessimistic_lock_response() {
        let (key1, key2, key3, key4) = (b"key1", b"key2", b"key3", b"key4");
        let (value1, value4) = (b"value1", b"value4");
        let value_empty = b"";

        let resp1 = ResponseWithShard(
            kvrpcpb::PessimisticLockResponse {
                values: vec![value1.to_vec()],
                ..Default::default()
            },
            vec![kvrpcpb::Mutation {
                op: kvrpcpb::Op::PessimisticLock.into(),
                key: key1.to_vec(),
                ..Default::default()
            }],
        );

        let resp_empty_value = ResponseWithShard(
            kvrpcpb::PessimisticLockResponse {
                values: vec![value_empty.to_vec()],
                ..Default::default()
            },
            vec![kvrpcpb::Mutation {
                op: kvrpcpb::Op::PessimisticLock.into(),
                key: key2.to_vec(),
                ..Default::default()
            }],
        );

        let resp_not_found = ResponseWithShard(
            kvrpcpb::PessimisticLockResponse {
                values: vec![value_empty.to_vec(), value4.to_vec()],
                not_founds: vec![true, false],
                ..Default::default()
            },
            vec![
                kvrpcpb::Mutation {
                    op: kvrpcpb::Op::PessimisticLock.into(),
                    key: key3.to_vec(),
                    ..Default::default()
                },
                kvrpcpb::Mutation {
                    op: kvrpcpb::Op::PessimisticLock.into(),
                    key: key4.to_vec(),
                    ..Default::default()
                },
            ],
        );

        let merger = CollectWithShard {};
        {
            // empty values & not founds are filtered.
            let input = vec![
                Ok(resp1.clone()),
                Ok(resp_empty_value.clone()),
                Ok(resp_not_found.clone()),
            ];
            let result = merger.merge(input);

            assert_eq!(
                result.unwrap(),
                vec![
                    KvPair::new(key1.to_vec(), value1.to_vec()),
                    KvPair::new(key4.to_vec(), value4.to_vec()),
                ]
            );
        }
        {
            let input = vec![
                Ok(resp1),
                Ok(resp_empty_value),
                Err(ResolveLockError(vec![])),
                Ok(resp_not_found),
            ];
            let result = merger.merge(input);

            if let PessimisticLockError {
                inner,
                success_keys,
            } = result.unwrap_err()
            {
                assert!(matches!(*inner, ResolveLockError(_)));
                assert_eq!(
                    success_keys,
                    vec![key1.to_vec(), key2.to_vec(), key3.to_vec(), key4.to_vec()]
                );
            } else {
                panic!();
            }
        }
    }
}
