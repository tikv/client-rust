// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.
use std::{collections::HashMap, iter, sync::Arc};

use either::Either;
use futures::stream::BoxStream;

use tikv_client_common::Error::PessimisticLockError;
use tikv_client_proto::{
    kvrpcpb::{self, TxnHeartBeatResponse},
    pdpb::Timestamp,
};

use crate::{
    collect_first,
    pd::PdClient,
    request::{
        codec::RequestCodec, Collect, CollectSingle, CollectWithShard, DefaultProcessor, KvRequest,
        Merge, Process, ResponseWithShard, Shardable, SingleKey,
    },
    store::{store_stream_for_keys, store_stream_for_range_by_start_key, RegionStore},
    timestamp::TimestampExt,
    transaction::HasLocks,
    util::iter::FlatMapOkIterExt,
    Key, KvPair, Result, Value,
};

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

impl_kv_request!(kvrpcpb::GetRequest, key; kvrpcpb::GetResponse, error);
shardable_key!(kvrpcpb::GetRequest);
collect_first!(kvrpcpb::GetResponse);
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

impl_kv_request!(
    kvrpcpb::BatchGetRequest, keys;
    kvrpcpb::BatchGetResponse, pairs, error
);
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
    reverse: bool,
) -> kvrpcpb::ScanRequest {
    let mut req = kvrpcpb::ScanRequest::default();
    if !reverse {
        req.set_start_key(start_key);
        req.set_end_key(end_key);
    } else {
        req.set_start_key(end_key);
        req.set_end_key(start_key);
    }
    req.set_limit(limit);
    req.set_key_only(key_only);
    req.set_version(timestamp);
    req.set_reverse(reverse);
    req
}

impl_kv_request!(kvrpcpb::ScanRequest; kvrpcpb::ScanResponse, pairs, error);
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

// Note: ResolveLockRequest is a special one: it can be sent to a specified
// region without keys. So it's not Shardable. And we don't automatically retry
// on its region errors (in the Plan level). The region error must be manually
// handled (in the upper level).

impl_kv_request!(kvrpcpb::ResolveLockRequest, keys; kvrpcpb::ResolveLockResponse, error);

pub fn new_cleanup_request(key: Vec<u8>, start_version: u64) -> kvrpcpb::CleanupRequest {
    let mut req = kvrpcpb::CleanupRequest::default();
    req.set_key(key);
    req.set_start_version(start_version);

    req
}

impl_kv_request!(
    kvrpcpb::CleanupRequest, key;
    kvrpcpb::CleanupResponse, error
);

shardable_key!(kvrpcpb::CleanupRequest);
collect_first!(kvrpcpb::CleanupResponse);
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

impl_kv_request!(kvrpcpb::PrewriteRequest, mutations; kvrpcpb::PrewriteResponse, errors);

impl Shardable for kvrpcpb::PrewriteRequest {
    type Shard = Vec<kvrpcpb::Mutation>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        let mut mutations = self.mutations.clone();
        mutations.sort_by(|a, b| a.key.cmp(&b.key));
        store_stream_for_keys(mutations.into_iter(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.set_context(store.region_with_leader.context()?);

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

impl_kv_request!(kvrpcpb::CommitRequest, keys; kvrpcpb::CommitResponse, error);
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

impl_kv_request!(kvrpcpb::BatchRollbackRequest, keys; kvrpcpb::BatchRollbackResponse, error);
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

impl_kv_request!(
    kvrpcpb::PessimisticRollbackRequest,
    keys;
    kvrpcpb::PessimisticRollbackResponse,
    errors
);

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

impl_kv_request!(
    kvrpcpb::PessimisticLockRequest,
    mutations, primary_lock;
    kvrpcpb::PessimisticLockResponse,
    errors
);

impl Shardable for kvrpcpb::PessimisticLockRequest {
    type Shard = Vec<kvrpcpb::Mutation>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        let mut mutations = self.mutations.clone();
        mutations.sort_by(|a, b| a.key.cmp(&b.key));
        store_stream_for_keys(mutations.into_iter(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.set_context(store.region_with_leader.context()?);
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
                .flat_map(|ResponseWithShard(mut resp, mutations)| {
                    let values = resp.take_values();
                    let values_len = values.len();
                    let not_founds = resp.take_not_founds();
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
    safepoint: u64,
    limit: u32,
) -> kvrpcpb::ScanLockRequest {
    let mut req = kvrpcpb::ScanLockRequest::default();
    req.set_start_key(start_key);
    req.set_max_version(safepoint);
    req.set_limit(limit);
    req
}

impl_kv_request!(
    kvrpcpb::ScanLockRequest;
    kvrpcpb::ScanLockResponse,
    error, locks
);

impl Shardable for kvrpcpb::ScanLockRequest {
    type Shard = Vec<u8>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        store_stream_for_range_by_start_key(self.start_key.clone().into(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.set_context(store.region_with_leader.context()?);
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

impl_kv_request!(
    kvrpcpb::TxnHeartBeatRequest,
    primary_lock;
    kvrpcpb::TxnHeartBeatResponse,
    error
);

impl Shardable for kvrpcpb::TxnHeartBeatRequest {
    type Shard = Vec<Vec<u8>>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        crate::store::store_stream_for_keys(std::iter::once(self.key().clone()), pd_client.clone())
    }

    fn apply_shard(&mut self, mut shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.set_context(store.region_with_leader.context()?);
        assert!(shard.len() == 1);
        self.primary_lock = shard.pop().unwrap();
        Ok(())
    }
}

collect_first!(TxnHeartBeatResponse);

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

impl_kv_request!(
    kvrpcpb::CheckTxnStatusRequest, primary_key;
    kvrpcpb::CheckTxnStatusResponse, lock_info, error
);

impl Shardable for kvrpcpb::CheckTxnStatusRequest {
    type Shard = Vec<Vec<u8>>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        crate::store::store_stream_for_keys(std::iter::once(self.key().clone()), pd_client.clone())
    }

    fn apply_shard(&mut self, mut shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.set_context(store.region_with_leader.context()?);
        assert!(shard.len() == 1);
        self.set_primary_key(shard.pop().unwrap());
        Ok(())
    }
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

impl_kv_request!(
    kvrpcpb::CheckSecondaryLocksRequest, keys;
    kvrpcpb::CheckSecondaryLocksResponse, locks, error
);

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

#[cfg(test)]
mod tests {
    use tikv_client_common::Error::{PessimisticLockError, ResolveLockError};
    use tikv_client_proto::kvrpcpb;

    use crate::{
        request::{plan::Merge, CollectWithShard, ResponseWithShard},
        KvPair,
    };

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
                op: kvrpcpb::Op::PessimisticLock as i32,
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
                op: kvrpcpb::Op::PessimisticLock as i32,
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
                    op: kvrpcpb::Op::PessimisticLock as i32,
                    key: key3.to_vec(),
                    ..Default::default()
                },
                kvrpcpb::Mutation {
                    op: kvrpcpb::Op::PessimisticLock as i32,
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
                Err(ResolveLockError),
                Ok(resp_not_found),
            ];
            let result = merger.merge(input);

            if let PessimisticLockError {
                inner,
                success_keys,
            } = result.unwrap_err()
            {
                assert!(matches!(*inner, ResolveLockError));
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
