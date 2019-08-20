// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    kv_client::{KvClient, KvRequest, RpcFnType, Store},
    pd::PdClient,
    raw::ColumnFamily,
    BoundRange, Error, Key, KvPair, Result, Value,
};
use futures::future::BoxFuture;
use futures::prelude::*;
use futures::stream::BoxStream;
use kvproto::kvrpcpb;
use kvproto::tikvpb::TikvClient;
use std::mem;
use std::sync::Arc;

pub(super) struct RawGet {
    pub key: Key,
    pub cf: Option<ColumnFamily>,
}

impl KvRequest for RawGet {
    type Result = Option<Value>;
    type RpcRequest = kvrpcpb::RawGetRequest;
    type RpcResponse = kvrpcpb::RawGetResponse;
    type KeyData = Key;
    const REQUEST_NAME: &'static str = "raw_get";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> = TikvClient::raw_get_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        key: Self::KeyData,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_key(key.into());
        req.maybe_set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let key = self.key.clone();
        pd_client
            .store_for_key(&self.key)
            .map_ok(move |store| (key, store))
            .into_stream()
            .boxed()
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        let result: Value = resp.take_value().into();
        if result.is_empty() {
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

pub(super) struct RawBatchGet {
    pub keys: Vec<Key>,
    pub cf: Option<ColumnFamily>,
}

impl KvRequest for RawBatchGet {
    type Result = Vec<KvPair>;
    type RpcRequest = kvrpcpb::RawBatchGetRequest;
    type RpcResponse = kvrpcpb::RawBatchGetResponse;
    type KeyData = Vec<Key>;
    const REQUEST_NAME: &'static str = "raw_batch_get";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> =
        TikvClient::raw_batch_get_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        keys: Self::KeyData,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_keys(keys.into_iter().map(Into::into).collect());
        req.maybe_set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let mut keys = Vec::new();
        mem::swap(&mut keys, &mut self.keys);

        pd_client
            .clone()
            .group_keys_by_region(keys.into_iter())
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

pub(super) struct RawPut {
    pub key: Key,
    pub value: Value,
    pub cf: Option<ColumnFamily>,
}

impl RawPut {
    pub fn new(
        key: impl Into<Key>,
        value: impl Into<Value>,
        cf: &Option<ColumnFamily>,
    ) -> Result<RawPut> {
        let value = value.into();
        if value.is_empty() {
            return Err(Error::empty_value());
        }

        let key = key.into();
        Ok(RawPut {
            key,
            value,
            cf: cf.clone(),
        })
    }
}

impl KvRequest for RawPut {
    type Result = ();
    type RpcRequest = kvrpcpb::RawPutRequest;
    type RpcResponse = kvrpcpb::RawPutResponse;
    type KeyData = KvPair;
    const REQUEST_NAME: &'static str = "raw_put";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> = TikvClient::raw_put_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        key: Self::KeyData,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_key(key.0.into());
        req.set_value(key.1.into());
        req.maybe_set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let kv = (self.key.clone(), self.value.clone()).into();
        pd_client
            .store_for_key(&self.key)
            .map_ok(move |store| (kv, store))
            .into_stream()
            .boxed()
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

pub(super) struct RawBatchPut {
    pub pairs: Vec<KvPair>,
    pub cf: Option<ColumnFamily>,
}

impl RawBatchPut {
    pub fn new(
        pairs: impl IntoIterator<Item = impl Into<KvPair>>,
        cf: &Option<ColumnFamily>,
    ) -> Result<RawBatchPut> {
        let pairs: Vec<KvPair> = pairs.into_iter().map(Into::into).collect();
        if pairs.iter().any(|pair| pair.value().is_empty()) {
            return Err(Error::empty_value());
        }

        Ok(RawBatchPut {
            pairs,
            cf: cf.clone(),
        })
    }
}

impl KvRequest for RawBatchPut {
    type Result = ();
    type RpcRequest = kvrpcpb::RawBatchPutRequest;
    type RpcResponse = kvrpcpb::RawBatchPutResponse;
    type KeyData = Vec<KvPair>;
    const REQUEST_NAME: &'static str = "raw_batch_put";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> =
        TikvClient::raw_batch_put_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        pairs: Self::KeyData,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_pairs(pairs.into_iter().map(Into::into).collect());
        req.maybe_set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let mut pairs = Vec::new();
        mem::swap(&mut pairs, &mut self.pairs);

        pd_client
            .clone()
            .group_keys_by_region(pairs.into_iter())
            .and_then(move |(region_id, pair)| {
                pd_client
                    .clone()
                    .store_for_id(region_id)
                    .map_ok(move |store| (pair, store))
            })
            .boxed()
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results.try_collect().boxed()
    }
}

pub(super) struct RawDelete {
    pub key: Key,
    pub cf: Option<ColumnFamily>,
}

impl KvRequest for RawDelete {
    type Result = ();
    type RpcRequest = kvrpcpb::RawDeleteRequest;
    type RpcResponse = kvrpcpb::RawDeleteResponse;
    type KeyData = Key;
    const REQUEST_NAME: &'static str = "raw_delete";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> = TikvClient::raw_delete_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        key: Self::KeyData,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_key(key.into());
        req.maybe_set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let key = self.key.clone();
        pd_client
            .store_for_key(&self.key)
            .map_ok(move |store| (key, store))
            .into_stream()
            .boxed()
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

pub(super) struct RawBatchDelete {
    pub keys: Vec<Key>,
    pub cf: Option<ColumnFamily>,
}

impl KvRequest for RawBatchDelete {
    type Result = ();
    type RpcRequest = kvrpcpb::RawBatchDeleteRequest;
    type RpcResponse = kvrpcpb::RawBatchDeleteResponse;
    type KeyData = Vec<Key>;
    const REQUEST_NAME: &'static str = "raw_batch_delete";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> =
        TikvClient::raw_batch_delete_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        keys: Self::KeyData,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_keys(keys.into_iter().map(Into::into).collect());
        req.maybe_set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let mut keys = Vec::new();
        mem::swap(&mut keys, &mut self.keys);

        pd_client
            .clone()
            .group_keys_by_region(keys.into_iter())
            .and_then(move |(region_id, key)| {
                pd_client
                    .clone()
                    .store_for_id(region_id)
                    .map_ok(move |store| (key, store))
            })
            .boxed()
    }

    fn map_result(_: Self::RpcResponse) -> Self::Result {}

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results.try_collect().boxed()
    }
}

pub(super) struct RawDeleteRange {
    pub range: BoundRange,
    pub cf: Option<ColumnFamily>,
}

impl KvRequest for RawDeleteRange {
    type Result = ();
    type RpcRequest = kvrpcpb::RawDeleteRangeRequest;
    type RpcResponse = kvrpcpb::RawDeleteRangeResponse;
    type KeyData = (Key, Key);
    const REQUEST_NAME: &'static str = "raw_delete_range";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> =
        TikvClient::raw_delete_range_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        (start_key, end_key): Self::KeyData,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());
        req.maybe_set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let range = self.range.clone();
        pd_client
            .stores_for_range(range)
            .map_ok(move |store| {
                // TODO should be bounded by self.range
                let range = store.region.range();
                (range, store)
            })
            .into_stream()
            .boxed()
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

pub(super) struct RawScan {
    pub range: BoundRange,
    // TODO this limit is currently treated as a per-region limit, not a total
    // limit.
    pub limit: u32,
    pub key_only: bool,
    pub cf: Option<ColumnFamily>,
}

impl KvRequest for RawScan {
    type Result = Vec<KvPair>;
    type RpcRequest = kvrpcpb::RawScanRequest;
    type RpcResponse = kvrpcpb::RawScanResponse;
    type KeyData = (Key, Key);
    const REQUEST_NAME: &'static str = "raw_scan";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> = TikvClient::raw_scan_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        (start_key, end_key): Self::KeyData,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());
        req.set_limit(self.limit);
        req.set_key_only(self.key_only);
        req.maybe_set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let range = self.range.clone();
        pd_client
            .stores_for_range(range)
            .map_ok(move |store| {
                // TODO seems like these should be bounded by self.range
                let range = store.region.range();
                (range, store)
            })
            .into_stream()
            .boxed()
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        resp.take_kvs().into_iter().map(Into::into).collect()
    }

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results.try_concat().boxed()
    }
}

pub(super) struct RawBatchScan {
    pub ranges: Vec<BoundRange>,
    pub each_limit: u32,
    pub key_only: bool,
    pub cf: Option<ColumnFamily>,
}

impl KvRequest for RawBatchScan {
    type Result = Vec<KvPair>;
    type RpcRequest = kvrpcpb::RawBatchScanRequest;
    type RpcResponse = kvrpcpb::RawBatchScanResponse;
    type KeyData = Vec<BoundRange>;
    const REQUEST_NAME: &'static str = "raw_batch_scan";
    const RPC_FN: RpcFnType<Self::RpcRequest, Self::RpcResponse> =
        TikvClient::raw_batch_scan_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        ranges: Self::KeyData,
        store: &Store<KvC>,
    ) -> Self::RpcRequest {
        let mut req = store.request::<Self::RpcRequest>();
        req.set_ranges(ranges.into_iter().map(Into::into).collect());
        req.set_each_limit(self.each_limit);
        req.set_key_only(self.key_only);
        req.maybe_set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        _pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        future::err(Error::unimplemented()).into_stream().boxed()
    }

    fn map_result(mut resp: Self::RpcResponse) -> Self::Result {
        resp.take_kvs().into_iter().map(Into::into).collect()
    }

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        results.try_concat().boxed()
    }
}

trait RawRpcRequest {
    fn set_cf(&mut self, cf: String);

    fn maybe_set_cf(&mut self, cf: Option<ColumnFamily>) {
        if let Some(cf) = cf {
            self.set_cf(cf.to_string());
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::kv_client::MockDispatch;
    use crate::mock::MockPdClient;
    use futures::executor;
    use futures::future::{ready, BoxFuture};
    use grpcio::CallOption;
    use kvproto::kvrpcpb;

    impl MockDispatch for RawScan {
        fn mock_dispatch(
            &self,
            request: &kvrpcpb::RawScanRequest,
            _opt: CallOption,
        ) -> Option<BoxFuture<'static, Result<kvrpcpb::RawScanResponse>>> {
            assert!(request.key_only);
            assert_eq!(request.limit, 10);

            let mut resp = kvrpcpb::RawScanResponse::default();
            for i in request.start_key[0]..request.end_key[0] {
                let mut kv = kvrpcpb::KvPair::default();
                kv.key = vec![i];
                resp.kvs.push(kv);
            }

            Some(Box::pin(ready(Ok(resp))))
        }
    }

    #[test]
    #[ignore]
    fn test_raw_scan() {
        let client = Arc::new(MockPdClient);

        let start: Key = vec![1].into();
        let end: Key = vec![50].into();
        let scan = RawScan {
            range: (start, end).into(),
            limit: 10,
            key_only: true,
            cf: None,
        };
        let scan = executor::block_on(scan.execute(client)).unwrap();

        assert_eq!(scan.len(), 10);
        // TODO test the keys returned.
    }
}
