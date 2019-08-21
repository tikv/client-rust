// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::RawRpcRequest;
use crate::{
    kv_client::{KvClient, RpcFnType, Store},
    pd::PdClient,
    request::KvRequest,
    BoundRange, ColumnFamily, Error, Key, KvPair, Result, Value,
};

use futures::future::BoxFuture;
use futures::prelude::*;
use futures::stream::BoxStream;
use kvproto::kvrpcpb;
use kvproto::tikvpb::TikvClient;
use std::mem;
use std::sync::Arc;

impl KvRequest for kvrpcpb::RawGetRequest {
    type Result = Option<Value>;
    type RpcResponse = kvrpcpb::RawGetResponse;
    type KeyData = Key;
    const REQUEST_NAME: &'static str = "raw_get";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::raw_get_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, key: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_key(key.into());
        req.set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let key = mem::replace(&mut self.key, Default::default()).into();
        pd_client
            .store_for_key(&key)
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

pub fn new_raw_get_request(
    key: impl Into<Key>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawGetRequest {
    let mut req = kvrpcpb::RawGetRequest::default();
    req.set_key(key.into().into());
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawBatchGetRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::RawBatchGetResponse;
    type KeyData = Vec<Key>;
    const REQUEST_NAME: &'static str = "raw_batch_get";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::raw_batch_get_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, keys: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_keys(keys.into_iter().map(Into::into).collect());
        req.set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let keys = mem::replace(&mut self.keys, Default::default());

        pd_client
            .clone()
            .group_keys_by_region(keys.into_iter().map(Into::into))
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

pub fn new_raw_batch_get_request(
    keys: impl IntoIterator<Item = impl Into<Key>>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchGetRequest {
    let mut req = kvrpcpb::RawBatchGetRequest::default();
    req.set_keys(keys.into_iter().map(Into::into).map(Into::into).collect());
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawPutRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawPutResponse;
    type KeyData = KvPair;
    const REQUEST_NAME: &'static str = "raw_put";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::raw_put_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, key: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_key(key.0.into());
        req.set_value(key.1.into());
        req.set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let key = mem::replace(&mut self.key, Default::default());
        let value = mem::replace(&mut self.value, Default::default());
        let pair = KvPair::new(key, value);
        pd_client
            .store_for_key(&pair.key())
            .map_ok(move |store| (pair, store))
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

pub fn new_raw_put_request(
    key: impl Into<Key>,
    value: impl Into<Value>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawPutRequest {
    let mut req = kvrpcpb::RawPutRequest::default();
    req.set_key(key.into().into());
    req.set_value(value.into().into());
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawBatchPutRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawBatchPutResponse;
    type KeyData = Vec<KvPair>;
    const REQUEST_NAME: &'static str = "raw_batch_put";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::raw_batch_put_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, pairs: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_pairs(pairs.into_iter().map(Into::into).collect());
        req.set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let pairs = mem::replace(&mut self.pairs, Default::default());

        pd_client
            .clone()
            .group_keys_by_region(pairs.into_iter().map(Into::into))
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

pub fn new_raw_batch_put_request(
    pairs: impl IntoIterator<Item = impl Into<KvPair>>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchPutRequest {
    let mut req = kvrpcpb::RawBatchPutRequest::default();
    req.set_pairs(pairs.into_iter().map(Into::into).map(Into::into).collect());
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawDeleteRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawDeleteResponse;
    type KeyData = Key;
    const REQUEST_NAME: &'static str = "raw_delete";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::raw_delete_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, key: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_key(key.into());
        req.set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let key = mem::replace(&mut self.key, Default::default()).into();
        pd_client
            .store_for_key(&key)
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

pub fn new_raw_delete_request(
    key: impl Into<Key>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawDeleteRequest {
    let mut req = kvrpcpb::RawDeleteRequest::default();
    req.set_key(key.into().into());
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawBatchDeleteRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawBatchDeleteResponse;
    type KeyData = Vec<Key>;
    const REQUEST_NAME: &'static str = "raw_batch_delete";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::raw_batch_delete_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, keys: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_keys(keys.into_iter().map(Into::into).collect());
        req.set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let keys = mem::replace(&mut self.keys, Default::default());

        pd_client
            .clone()
            .group_keys_by_region(keys.into_iter().map(Into::into))
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

pub fn new_raw_batch_delete_request(
    keys: impl IntoIterator<Item = impl Into<Key>>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchDeleteRequest {
    let mut req = kvrpcpb::RawBatchDeleteRequest::default();
    req.set_keys(keys.into_iter().map(Into::into).map(Into::into).collect());
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawDeleteRangeRequest {
    type Result = ();
    type RpcResponse = kvrpcpb::RawDeleteRangeResponse;
    type KeyData = (Key, Key);
    const REQUEST_NAME: &'static str = "raw_delete_range";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::raw_delete_range_async_opt;

    fn make_rpc_request<KvC: KvClient>(
        &self,
        (start_key, end_key): Self::KeyData,
        store: &Store<KvC>,
    ) -> Self {
        let mut req = store.request::<Self>();
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());
        req.set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let start_key = mem::replace(&mut self.start_key, Default::default());
        let end_key = mem::replace(&mut self.end_key, Default::default());
        let range = BoundRange::from((start_key, end_key));
        pd_client
            .stores_for_range(range)
            .map_ok(move |store| {
                // FIXME should be bounded by self.range
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

pub fn new_raw_delete_range_request(
    range: impl Into<BoundRange>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawDeleteRangeRequest {
    let (start_key, end_key) = range.into().into_keys();
    let mut req = kvrpcpb::RawDeleteRangeRequest::default();
    req.set_start_key(start_key.into());
    req.set_end_key(end_key.unwrap_or_default().into());
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawScanRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::RawScanResponse;
    type KeyData = (Key, Key);
    const REQUEST_NAME: &'static str = "raw_scan";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::raw_scan_async_opt;

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
        req.set_cf(self.cf.clone());

        req
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
        let start_key = mem::replace(&mut self.start_key, Default::default());
        let end_key = mem::replace(&mut self.end_key, Default::default());
        let range = BoundRange::from((start_key, end_key));
        pd_client
            .stores_for_range(range)
            .map_ok(move |store| {
                // FIXME should be bounded by self.range
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

pub fn new_raw_scan_request(
    range: impl Into<BoundRange>,
    limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawScanRequest {
    let (start_key, end_key) = range.into().into_keys();
    let mut req = kvrpcpb::RawScanRequest::default();
    req.set_start_key(start_key.into());
    req.set_end_key(end_key.unwrap_or_default().into());
    req.set_limit(limit);
    req.set_key_only(key_only);
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawBatchScanRequest {
    type Result = Vec<KvPair>;
    type RpcResponse = kvrpcpb::RawBatchScanResponse;
    type KeyData = Vec<BoundRange>;
    const REQUEST_NAME: &'static str = "raw_batch_scan";
    const RPC_FN: RpcFnType<Self, Self::RpcResponse> = TikvClient::raw_batch_scan_async_opt;

    fn make_rpc_request<KvC: KvClient>(&self, ranges: Self::KeyData, store: &Store<KvC>) -> Self {
        let mut req = store.request::<Self>();
        req.set_ranges(ranges.into_iter().map(Into::into).collect());
        req.set_each_limit(self.each_limit);
        req.set_key_only(self.key_only);
        req.set_cf(self.cf.clone());

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

pub fn new_raw_batch_scan_request(
    ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
    each_limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchScanRequest {
    let mut req = kvrpcpb::RawBatchScanRequest::default();
    req.set_ranges(ranges.into_iter().map(Into::into).map(Into::into).collect());
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

#[cfg(test)]
mod test {
    use super::*;

    use crate::mock::MockPdClient;
    use crate::request::DispatchHook;

    use futures::executor;
    use futures::future::{ready, BoxFuture};
    use grpcio::CallOption;
    use kvproto::kvrpcpb;

    impl DispatchHook for kvrpcpb::RawScanRequest {
        fn dispatch_hook(
            &self,
            request: &Self,
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
        let scan = kvrpcpb::RawScanRequest {
            start_key: start.into(),
            end_key: end.into(),
            limit: 10,
            key_only: true,
            ..Default::default()
        };
        let scan = executor::block_on(scan.execute(client)).unwrap();

        assert_eq!(scan.len(), 10);
        // TODO test the keys returned.
    }
}
