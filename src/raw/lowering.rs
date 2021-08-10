// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

//! This module provides constructor functions for requests which take arguments as high-level
//! types (i.e., the types from the client crate) and converts these to the types used in the
//! generated protobuf code, then calls the low-level ctor functions in the requests module.

use std::{iter::Iterator, ops::Range, sync::Arc};

use tikv_client_proto::{kvrpcpb, metapb};

use crate::{raw::requests, BoundRange, ColumnFamily, Key, KvPair, Value};

pub fn new_raw_get_request(key: Key, cf: Option<ColumnFamily>) -> kvrpcpb::RawGetRequest {
    requests::new_raw_get_request(key.into(), cf)
}

pub fn new_raw_batch_get_request(
    keys: impl Iterator<Item = Key>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchGetRequest {
    requests::new_raw_batch_get_request(keys.map(Into::into).collect(), cf)
}

pub fn new_raw_put_request(
    key: Key,
    value: Value,
    cf: Option<ColumnFamily>,
    atomic: bool,
) -> kvrpcpb::RawPutRequest {
    requests::new_raw_put_request(key.into(), value, cf, atomic)
}

pub fn new_raw_batch_put_request(
    pairs: impl Iterator<Item = KvPair>,
    cf: Option<ColumnFamily>,
    atomic: bool,
) -> kvrpcpb::RawBatchPutRequest {
    requests::new_raw_batch_put_request(pairs.map(Into::into).collect(), cf, atomic)
}

pub fn new_raw_delete_request(
    key: Key,
    cf: Option<ColumnFamily>,
    atomic: bool,
) -> kvrpcpb::RawDeleteRequest {
    requests::new_raw_delete_request(key.into(), cf, atomic)
}

pub fn new_raw_batch_delete_request(
    keys: impl Iterator<Item = Key>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchDeleteRequest {
    requests::new_raw_batch_delete_request(keys.map(Into::into).collect(), cf)
}

pub fn new_raw_delete_range_request(
    range: BoundRange,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawDeleteRangeRequest {
    let (start_key, end_key) = range.into_keys();
    requests::new_raw_delete_range_request(start_key.into(), end_key.unwrap_or_default().into(), cf)
}

pub fn new_raw_scan_request(
    range: BoundRange,
    limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawScanRequest {
    let (start_key, end_key) = range.into_keys();
    requests::new_raw_scan_request(
        start_key.into(),
        end_key.unwrap_or_default().into(),
        limit,
        key_only,
        cf,
    )
}

pub fn new_raw_batch_scan_request(
    ranges: impl Iterator<Item = BoundRange>,
    each_limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchScanRequest {
    requests::new_raw_batch_scan_request(ranges.map(Into::into).collect(), each_limit, key_only, cf)
}

pub fn new_cas_request(
    key: Key,
    value: Value,
    previous_value: Option<Value>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawCasRequest {
    requests::new_cas_request(key.into(), value, previous_value, cf)
}

pub fn new_raw_coprocessor_request(
    copr_name: String,
    copr_version_req: String,
    ranges: impl Iterator<Item = BoundRange>,
    request_builder: impl Fn(metapb::Region, Vec<Range<Key>>) -> Vec<u8> + Send + Sync + 'static,
) -> requests::RawCoprocessorRequest {
    requests::new_raw_coprocessor_request(
        copr_name,
        copr_version_req,
        ranges.map(Into::into).collect(),
        Arc::new(move |region, ranges| {
            request_builder(
                region,
                ranges
                    .into_iter()
                    .map(|range| range.start_key.into()..range.end_key.into())
                    .collect(),
            )
        }),
    )
}
