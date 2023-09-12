// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::iter::Iterator;

use crate::proto::kvrpcpb;
use crate::proto::pdpb::Timestamp;
/// This module provides constructor functions for requests which take arguments as high-level
/// types (i.e., the types from the client crate) and converts these to the types used in the
/// generated protobuf code, then calls the low-level ctor functions in the requests module.
use crate::timestamp::TimestampExt;
/// This module provides constructor functions for requests which take arguments as high-level
/// types (i.e., the types from the client crate) and converts these to the types used in the
/// generated protobuf code, then calls the low-level ctor functions in the requests module.
use crate::transaction::requests;
/// This module provides constructor functions for requests which take arguments as high-level
/// types (i.e., the types from the client crate) and converts these to the types used in the
/// generated protobuf code, then calls the low-level ctor functions in the requests module.
use crate::BoundRange;
/// This module provides constructor functions for requests which take arguments as high-level
/// types (i.e., the types from the client crate) and converts these to the types used in the
/// generated protobuf code, then calls the low-level ctor functions in the requests module.
use crate::Key;

pub fn new_get_request(key: Key, timestamp: Timestamp) -> kvrpcpb::GetRequest {
    requests::new_get_request(key.into(), timestamp.version())
}

pub fn new_batch_get_request(
    keys: impl Iterator<Item = Key>,
    timestamp: Timestamp,
) -> kvrpcpb::BatchGetRequest {
    requests::new_batch_get_request(keys.map(Into::into).collect(), timestamp.version())
}

pub fn new_scan_request(
    range: BoundRange,
    timestamp: Timestamp,
    limit: u32,
    key_only: bool,
    reverse: bool,
) -> kvrpcpb::ScanRequest {
    let (start_key, end_key) = range.into_keys();
    requests::new_scan_request(
        start_key.into(),
        end_key.unwrap_or_default().into(),
        timestamp.version(),
        limit,
        key_only,
        reverse,
    )
}

pub fn new_prewrite_request(
    mutations: Vec<kvrpcpb::Mutation>,
    primary_lock: Key,
    start_version: Timestamp,
    lock_ttl: u64,
) -> kvrpcpb::PrewriteRequest {
    requests::new_prewrite_request(
        mutations,
        primary_lock.into(),
        start_version.version(),
        lock_ttl,
    )
}

pub fn new_pessimistic_prewrite_request(
    mutations: Vec<kvrpcpb::Mutation>,
    primary_lock: Key,
    start_version: Timestamp,
    lock_ttl: u64,
    for_update_ts: Timestamp,
) -> kvrpcpb::PrewriteRequest {
    requests::new_pessimistic_prewrite_request(
        mutations,
        primary_lock.into(),
        start_version.version(),
        lock_ttl,
        for_update_ts.version(),
    )
}

pub fn new_commit_request(
    keys: impl Iterator<Item = Key>,
    start_version: Timestamp,
    commit_version: Timestamp,
) -> kvrpcpb::CommitRequest {
    requests::new_commit_request(
        keys.map(Into::into).collect(),
        start_version.version(),
        commit_version.version(),
    )
}

pub fn new_batch_rollback_request(
    keys: impl Iterator<Item = Key>,
    start_version: Timestamp,
) -> kvrpcpb::BatchRollbackRequest {
    requests::new_batch_rollback_request(keys.map(Into::into).collect(), start_version.version())
}

pub fn new_pessimistic_rollback_request(
    keys: impl Iterator<Item = Key>,
    start_version: Timestamp,
    for_update_ts: Timestamp,
) -> kvrpcpb::PessimisticRollbackRequest {
    requests::new_pessimistic_rollback_request(
        keys.map(Into::into).collect(),
        start_version.version(),
        for_update_ts.version(),
    )
}

pub trait PessimisticLock: Clone {
    fn key(self) -> Key;

    fn assertion(&self) -> kvrpcpb::Assertion;
}

impl PessimisticLock for Key {
    fn key(self) -> Key {
        self
    }

    fn assertion(&self) -> kvrpcpb::Assertion {
        kvrpcpb::Assertion::None
    }
}

impl PessimisticLock for (Key, kvrpcpb::Assertion) {
    fn key(self) -> Key {
        self.0
    }

    fn assertion(&self) -> kvrpcpb::Assertion {
        self.1
    }
}

pub fn new_pessimistic_lock_request(
    locks: impl Iterator<Item = impl PessimisticLock>,
    primary_lock: Key,
    start_version: Timestamp,
    lock_ttl: u64,
    for_update_ts: Timestamp,
    need_value: bool,
) -> kvrpcpb::PessimisticLockRequest {
    requests::new_pessimistic_lock_request(
        locks
            .map(|pl| {
                let mut mutation = kvrpcpb::Mutation::default();
                mutation.op = kvrpcpb::Op::PessimisticLock.into();
                mutation.assertion = pl.assertion().into();
                mutation.key = pl.key().into();
                mutation
            })
            .collect(),
        primary_lock.into(),
        start_version.version(),
        lock_ttl,
        for_update_ts.version(),
        need_value,
    )
}

pub fn new_scan_lock_request(
    range: BoundRange,
    safepoint: &Timestamp,
    limit: u32,
) -> kvrpcpb::ScanLockRequest {
    let (start_key, end_key) = range.into_keys();
    requests::new_scan_lock_request(
        start_key.into(),
        end_key.unwrap_or_default().into(),
        safepoint.version(),
        limit,
    )
}

pub fn new_heart_beat_request(
    start_ts: Timestamp,
    primary_lock: Key,
    ttl: u64,
) -> kvrpcpb::TxnHeartBeatRequest {
    requests::new_heart_beat_request(start_ts.version(), primary_lock.into(), ttl)
}

pub fn new_unsafe_destroy_range_request(range: BoundRange) -> kvrpcpb::UnsafeDestroyRangeRequest {
    let (start_key, end_key) = range.into_keys();
    requests::new_unsafe_destroy_range_request(start_key.into(), end_key.unwrap_or_default().into())
}
