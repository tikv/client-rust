// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::RawRpcRequest;
use crate::{
    pd::PdClient,
    request::KvRequest,
    store::{
        store_stream_for_key, store_stream_for_keys, store_stream_for_range,
        store_stream_for_ranges, Store,
    },
    transaction::HasLocks,
    BoundRange, ColumnFamily, Key, KvPair, Result, Value,
};
use async_trait::async_trait;
use futures::{prelude::*, stream::BoxStream};
use std::{mem, sync::Arc};
use tikv_client_proto::kvrpcpb;

#[async_trait]
impl KvRequest for kvrpcpb::RawGetRequest {
    type Result = Option<Value>;
    type RpcResponse = kvrpcpb::RawGetResponse;
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
        req.set_cf(self.cf.clone());

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

pub fn new_raw_get_request(key: Vec<u8>, cf: Option<ColumnFamily>) -> kvrpcpb::RawGetRequest {
    let mut req = kvrpcpb::RawGetRequest::default();
    req.set_key(key);
    req.maybe_set_cf(cf);

    req
}

#[async_trait]
impl KvRequest for kvrpcpb::RawBatchGetRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::RawBatchGetResponse;
    type KeyData = Vec<Key>;

    fn make_rpc_request(&self, keys: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_keys(keys.into_iter().map(Into::into).collect());
        req.set_cf(self.cf.clone());

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

pub fn new_raw_batch_get_request(
    keys: Vec<Vec<u8>>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchGetRequest {
    let mut req = kvrpcpb::RawBatchGetRequest::default();
    req.set_keys(keys);
    req.maybe_set_cf(cf);

    req
}

#[async_trait]
impl KvRequest for kvrpcpb::RawPutRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawPutResponse;
    type KeyData = KvPair;

    fn make_rpc_request(&self, key: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_key(key.0.into());
        req.set_value(key.1);
        req.set_cf(self.cf.clone());

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let key = mem::take(&mut self.key);
        let value = mem::take(&mut self.value);
        let pair = KvPair::new(key, value);
        store_stream_for_key(pair, pd_client)
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .await
    }
}

pub fn new_raw_put_request(
    key: Vec<u8>,
    value: Vec<u8>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawPutRequest {
    let mut req = kvrpcpb::RawPutRequest::default();
    req.set_key(key);
    req.set_value(value);
    req.maybe_set_cf(cf);

    req
}

#[async_trait]
impl KvRequest for kvrpcpb::RawBatchPutRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawBatchPutResponse;
    type KeyData = Vec<KvPair>;

    fn make_rpc_request(&self, pairs: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_pairs(pairs.into_iter().map(Into::into).collect());
        req.set_cf(self.cf.clone());

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        self.pairs.sort_by(|a, b| a.key.cmp(&b.key));
        let pairs = mem::take(&mut self.pairs);
        store_stream_for_keys(pairs, pd_client)
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results.try_collect().await
    }
}

pub fn new_raw_batch_put_request(
    pairs: Vec<kvrpcpb::KvPair>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchPutRequest {
    let mut req = kvrpcpb::RawBatchPutRequest::default();
    req.set_pairs(pairs);
    req.maybe_set_cf(cf);

    req
}

#[async_trait]
impl KvRequest for kvrpcpb::RawDeleteRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawDeleteResponse;
    type KeyData = Key;

    fn make_rpc_request(&self, key: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_key(key.into());
        req.set_cf(self.cf.clone());

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let key = mem::take(&mut self.key).into();
        store_stream_for_key(key, pd_client)
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .into_future()
            .map(|(f, _)| f.expect("no results should be impossible"))
            .await
    }
}

pub fn new_raw_delete_request(key: Vec<u8>, cf: Option<ColumnFamily>) -> kvrpcpb::RawDeleteRequest {
    let mut req = kvrpcpb::RawDeleteRequest::default();
    req.set_key(key);
    req.maybe_set_cf(cf);

    req
}

#[async_trait]
impl KvRequest for kvrpcpb::RawBatchDeleteRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawBatchDeleteResponse;
    type KeyData = Vec<Key>;

    fn make_rpc_request(&self, keys: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_keys(keys.into_iter().map(Into::into).collect());
        req.set_cf(self.cf.clone());

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
        results.try_collect().await
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

#[async_trait]
impl KvRequest for kvrpcpb::RawDeleteRangeRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawDeleteRangeResponse;
    type KeyData = (Key, Key);

    fn make_rpc_request(&self, (start_key, end_key): Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());
        req.set_cf(self.cf.clone());

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

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results
            .try_for_each_concurrent(None, |_| future::ready(Ok(())))
            .await
    }
}

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

#[async_trait]
impl KvRequest for kvrpcpb::RawScanRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::RawScanResponse;
    type KeyData = (Key, Key);

    fn make_rpc_request(&self, (start_key, end_key): Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());
        req.set_limit(self.limit);
        req.set_key_only(self.key_only);
        req.set_cf(self.cf.clone());

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
        resp.take_kvs().into_iter().map(Into::into).collect()
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results.try_concat().await
    }
}

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

#[async_trait]
impl KvRequest for kvrpcpb::RawBatchScanRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::RawBatchScanResponse;
    type KeyData = Vec<BoundRange>;

    fn make_rpc_request(&self, ranges: Self::KeyData, store: &Store) -> Result<Self> {
        let mut req = self.request_from_store(store)?;
        req.set_ranges(ranges.into_iter().map(Into::into).collect());
        req.set_each_limit(self.each_limit);
        req.set_key_only(self.key_only);
        req.set_cf(self.cf.clone());

        Ok(req)
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
        let ranges = mem::take(&mut self.ranges)
            .into_iter()
            .map(|range| range.into())
            .collect();
        store_stream_for_ranges(ranges, pd_client)
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        resp.take_kvs().into_iter().map(Into::into).collect()
    }

    async fn reduce(results: BoxStream<'static, Result<Self::Result>>) -> Result<Self::Result> {
        results.try_concat().await
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

impl HasLocks for kvrpcpb::RawGetResponse {}
impl HasLocks for kvrpcpb::RawBatchGetResponse {}
impl HasLocks for kvrpcpb::RawPutResponse {}
impl HasLocks for kvrpcpb::RawBatchPutResponse {}
impl HasLocks for kvrpcpb::RawDeleteResponse {}
impl HasLocks for kvrpcpb::RawBatchDeleteResponse {}
impl HasLocks for kvrpcpb::RawScanResponse {}
impl HasLocks for kvrpcpb::RawBatchScanResponse {}
impl HasLocks for kvrpcpb::RawDeleteRangeResponse {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        mock::{MockKvClient, MockPdClient},
        request::RetryOptions,
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
                    let mut kv = kvrpcpb::KvPair::default();
                    kv.key = vec![i];
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
        let scan =
            executor::block_on(scan.execute(client, RetryOptions::default_optimistic())).unwrap();

        assert_eq!(scan.len(), 10);
        // TODO test the keys returned.
    }
}
