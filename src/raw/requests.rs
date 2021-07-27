// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::RawRpcRequest;
use crate::{
    collect_first,
    pd::PdClient,
    request::{
        Collect, CollectSingle, DefaultProcessor, KvRequest, Merge, Process, Shardable, SingleKey,
    },
    store::{store_stream_for_keys, store_stream_for_ranges, RegionStore},
    transaction::HasLocks,
    util::iter::FlatMapOkIterExt,
    ColumnFamily, KvPair, Result, Value,
};
use futures::stream::BoxStream;
use std::sync::Arc;
use tikv_client_proto::kvrpcpb;

pub fn new_raw_get_request(key: Vec<u8>, cf: Option<ColumnFamily>) -> kvrpcpb::RawGetRequest {
    let mut req = kvrpcpb::RawGetRequest::default();
    req.set_key(key);
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawGetRequest {
    type Response = kvrpcpb::RawGetResponse;
}

shardable_key!(kvrpcpb::RawGetRequest);
collect_first!(kvrpcpb::RawGetResponse);

impl SingleKey for kvrpcpb::RawGetRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

impl Process<kvrpcpb::RawGetResponse> for DefaultProcessor {
    type Out = Option<Value>;

    fn process(&self, input: Result<kvrpcpb::RawGetResponse>) -> Result<Self::Out> {
        let mut input = input?;
        Ok(if input.not_found {
            None
        } else {
            Some(input.take_value())
        })
    }
}

pub fn new_raw_batch_get_request(
    keys: Vec<Vec<u8>>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchGetRequest {
    let mut req = kvrpcpb::RawBatchGetRequest::default();
    req.set_keys(keys);
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawBatchGetRequest {
    type Response = kvrpcpb::RawBatchGetResponse;
}

shardable_keys!(kvrpcpb::RawBatchGetRequest);

impl Merge<kvrpcpb::RawBatchGetResponse> for Collect {
    type Out = Vec<KvPair>;

    fn merge(&self, input: Vec<Result<kvrpcpb::RawBatchGetResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|mut resp| resp.take_pairs().into_iter().map(Into::into))
            .collect()
    }
}

pub fn new_raw_put_request(
    key: Vec<u8>,
    value: Vec<u8>,
    cf: Option<ColumnFamily>,
    atomic: bool,
) -> kvrpcpb::RawPutRequest {
    let mut req = kvrpcpb::RawPutRequest::default();
    req.set_key(key);
    req.set_value(value);
    req.maybe_set_cf(cf);
    req.set_for_cas(atomic);

    req
}

impl KvRequest for kvrpcpb::RawPutRequest {
    type Response = kvrpcpb::RawPutResponse;
}

shardable_key!(kvrpcpb::RawPutRequest);
collect_first!(kvrpcpb::RawPutResponse);
impl SingleKey for kvrpcpb::RawPutRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

pub fn new_raw_batch_put_request(
    pairs: Vec<kvrpcpb::KvPair>,
    cf: Option<ColumnFamily>,
    atomic: bool,
) -> kvrpcpb::RawBatchPutRequest {
    let mut req = kvrpcpb::RawBatchPutRequest::default();
    req.set_pairs(pairs);
    req.maybe_set_cf(cf);
    req.set_for_cas(atomic);

    req
}

impl KvRequest for kvrpcpb::RawBatchPutRequest {
    type Response = kvrpcpb::RawBatchPutResponse;
}

impl Shardable for kvrpcpb::RawBatchPutRequest {
    type Shard = Vec<kvrpcpb::KvPair>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        let mut pairs = self.pairs.clone();
        pairs.sort_by(|a, b| a.key.cmp(&b.key));
        store_stream_for_keys(
            pairs.into_iter().map(Into::<KvPair>::into),
            pd_client.clone(),
        )
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.set_context(store.region_with_leader.context()?);
        self.set_pairs(shard);
        Ok(())
    }
}

pub fn new_raw_delete_request(
    key: Vec<u8>,
    cf: Option<ColumnFamily>,
    atomic: bool,
) -> kvrpcpb::RawDeleteRequest {
    let mut req = kvrpcpb::RawDeleteRequest::default();
    req.set_key(key);
    req.maybe_set_cf(cf);
    req.set_for_cas(atomic);

    req
}

impl KvRequest for kvrpcpb::RawDeleteRequest {
    type Response = kvrpcpb::RawDeleteResponse;
}

shardable_key!(kvrpcpb::RawDeleteRequest);
collect_first!(kvrpcpb::RawDeleteResponse);
impl SingleKey for kvrpcpb::RawDeleteRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

pub fn new_raw_batch_delete_request(
    keys: Vec<Vec<u8>>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchDeleteRequest {
    let mut req = kvrpcpb::RawBatchDeleteRequest::default();
    req.set_keys(keys);
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawBatchDeleteRequest {
    type Response = kvrpcpb::RawBatchDeleteResponse;
}

shardable_keys!(kvrpcpb::RawBatchDeleteRequest);

pub fn new_raw_delete_range_request(
    start_key: Vec<u8>,
    end_key: Vec<u8>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawDeleteRangeRequest {
    let mut req = kvrpcpb::RawDeleteRangeRequest::default();
    req.set_start_key(start_key);
    req.set_end_key(end_key);
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawDeleteRangeRequest {
    type Response = kvrpcpb::RawDeleteRangeResponse;
}

shardable_range!(kvrpcpb::RawDeleteRangeRequest);

pub fn new_raw_scan_request(
    start_key: Vec<u8>,
    end_key: Vec<u8>,
    limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawScanRequest {
    let mut req = kvrpcpb::RawScanRequest::default();
    req.set_start_key(start_key);
    req.set_end_key(end_key);
    req.set_limit(limit);
    req.set_key_only(key_only);
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawScanRequest {
    type Response = kvrpcpb::RawScanResponse;
}

shardable_range!(kvrpcpb::RawScanRequest);

impl Merge<kvrpcpb::RawScanResponse> for Collect {
    type Out = Vec<KvPair>;

    fn merge(&self, input: Vec<Result<kvrpcpb::RawScanResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|mut resp| resp.take_kvs().into_iter().map(Into::into))
            .collect()
    }
}

pub fn new_raw_batch_scan_request(
    ranges: Vec<kvrpcpb::KeyRange>,
    each_limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchScanRequest {
    let mut req = kvrpcpb::RawBatchScanRequest::default();
    req.set_ranges(ranges);
    req.set_each_limit(each_limit);
    req.set_key_only(key_only);
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawBatchScanRequest {
    type Response = kvrpcpb::RawBatchScanResponse;
}

impl Shardable for kvrpcpb::RawBatchScanRequest {
    type Shard = Vec<kvrpcpb::KeyRange>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        store_stream_for_ranges(self.ranges.clone(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.set_context(store.region_with_leader.context()?);
        self.set_ranges(shard);
        Ok(())
    }
}

impl Merge<kvrpcpb::RawBatchScanResponse> for Collect {
    type Out = Vec<KvPair>;

    fn merge(&self, input: Vec<Result<kvrpcpb::RawBatchScanResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|mut resp| resp.take_kvs().into_iter().map(Into::into))
            .collect()
    }
}

pub fn new_cas_request(
    key: Vec<u8>,
    value: Vec<u8>,
    previous_value: Option<Vec<u8>>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawCasRequest {
    let mut req = kvrpcpb::RawCasRequest::default();
    req.set_key(key);
    req.set_value(value);
    match previous_value {
        Some(v) => req.set_previous_value(v),
        None => req.set_previous_not_exist(true),
    }
    req.maybe_set_cf(cf);
    req
}

impl KvRequest for kvrpcpb::RawCasRequest {
    type Response = kvrpcpb::RawCasResponse;
}

shardable_key!(kvrpcpb::RawCasRequest);
collect_first!(kvrpcpb::RawCasResponse);
impl SingleKey for kvrpcpb::RawCasRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

impl Process<kvrpcpb::RawCasResponse> for DefaultProcessor {
    type Out = (Option<Value>, bool); // (previous_value, swapped)

    fn process(&self, input: Result<kvrpcpb::RawCasResponse>) -> Result<Self::Out> {
        let input = input?;
        if input.previous_not_exist {
            Ok((None, input.succeed))
        } else {
            Ok((Some(input.previous_value), input.succeed))
        }
    }
}

macro_rules! impl_raw_rpc_request {
    ($name: ident) => {
        impl RawRpcRequest for kvrpcpb::$name {
            fn set_cf(&mut self, cf: String) {
                self.set_cf(cf);
            }
        }
    };
}

impl_raw_rpc_request!(RawGetRequest);
impl_raw_rpc_request!(RawBatchGetRequest);
impl_raw_rpc_request!(RawPutRequest);
impl_raw_rpc_request!(RawBatchPutRequest);
impl_raw_rpc_request!(RawDeleteRequest);
impl_raw_rpc_request!(RawBatchDeleteRequest);
impl_raw_rpc_request!(RawScanRequest);
impl_raw_rpc_request!(RawBatchScanRequest);
impl_raw_rpc_request!(RawDeleteRangeRequest);
impl_raw_rpc_request!(RawCasRequest);

impl HasLocks for kvrpcpb::RawGetResponse {}
impl HasLocks for kvrpcpb::RawBatchGetResponse {}
impl HasLocks for kvrpcpb::RawPutResponse {}
impl HasLocks for kvrpcpb::RawBatchPutResponse {}
impl HasLocks for kvrpcpb::RawDeleteResponse {}
impl HasLocks for kvrpcpb::RawBatchDeleteResponse {}
impl HasLocks for kvrpcpb::RawScanResponse {}
impl HasLocks for kvrpcpb::RawBatchScanResponse {}
impl HasLocks for kvrpcpb::RawDeleteRangeResponse {}
impl HasLocks for kvrpcpb::RawCasResponse {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        backoff::{DEFAULT_REGION_BACKOFF, OPTIMISTIC_BACKOFF},
        mock::{MockKvClient, MockPdClient},
        request::Plan,
        Key,
    };
    use futures::executor;
    use std::any::Any;
    use tikv_client_proto::kvrpcpb;

    #[test]
    #[ignore]
    fn test_raw_scan() {
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |req: &dyn Any| {
                let req: &kvrpcpb::RawScanRequest = req.downcast_ref().unwrap();
                assert!(req.key_only);
                assert_eq!(req.limit, 10);

                let mut resp = kvrpcpb::RawScanResponse::default();
                for i in req.start_key[0]..req.end_key[0] {
                    let kv = kvrpcpb::KvPair {
                        key: vec![i],
                        ..Default::default()
                    };
                    resp.kvs.push(kv);
                }

                Ok(Box::new(resp) as Box<dyn Any>)
            },
        )));

        let start: Key = vec![1].into();
        let end: Key = vec![50].into();
        let scan = kvrpcpb::RawScanRequest {
            start_key: start.into(),
            end_key: end.into(),
            limit: 10,
            key_only: true,
            ..Default::default()
        };
        let plan = crate::request::PlanBuilder::new(client, scan)
            .resolve_lock(OPTIMISTIC_BACKOFF)
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .merge(Collect)
            .plan();
        let scan = executor::block_on(async { plan.execute().await }).unwrap();

        assert_eq!(scan.len(), 10);
        // FIXME test the keys returned.
    }
}
