// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::RawRpcRequest;
use crate::collect_single;
use crate::kv::KvPairTTL;
use crate::pd::PdClient;
use crate::proto::kvrpcpb;
use crate::proto::metapb;
use crate::proto::tikvpb::tikv_client::TikvClient;
use crate::range_request;
use crate::region::RegionWithLeader;
use crate::request::plan::ResponseWithShard;
use crate::request::Collect;
use crate::request::CollectSingle;
use crate::request::DefaultProcessor;
use crate::request::KvRequest;
use crate::request::Merge;
use crate::request::Process;
use crate::request::RangeRequest;
use crate::request::Shardable;
use crate::request::SingleKey;
use crate::shardable_key;
use crate::shardable_keys;
use crate::shardable_range;
use crate::store::store_stream_for_keys;
use crate::store::store_stream_for_ranges;
use crate::store::RegionStore;
use crate::store::Request;
use crate::transaction::HasLocks;
use crate::util::iter::FlatMapOkIterExt;
use crate::ColumnFamily;
use crate::Key;
use crate::KvPair;
use crate::Result;
use crate::Value;
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::any::Any;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;
use tonic::transport::Channel;

pub fn new_raw_get_request(key: Vec<u8>, cf: Option<ColumnFamily>) -> kvrpcpb::RawGetRequest {
    let mut req = kvrpcpb::RawGetRequest::default();
    req.key = key;
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawGetRequest {
    type Response = kvrpcpb::RawGetResponse;
}

shardable_key!(kvrpcpb::RawGetRequest);
collect_single!(kvrpcpb::RawGetResponse);

impl SingleKey for kvrpcpb::RawGetRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

impl Process<kvrpcpb::RawGetResponse> for DefaultProcessor {
    type Out = Option<Value>;

    fn process(&self, input: Result<kvrpcpb::RawGetResponse>) -> Result<Self::Out> {
        let input = input?;
        Ok(if input.not_found {
            None
        } else {
            Some(input.value)
        })
    }
}

pub fn new_raw_batch_get_request(
    keys: Vec<Vec<u8>>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawBatchGetRequest {
    let mut req = kvrpcpb::RawBatchGetRequest::default();
    req.keys = keys;
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
            .flat_map_ok(|resp| resp.pairs.into_iter().map(Into::into))
            .collect()
    }
}

pub fn new_raw_get_key_ttl_request(
    key: Vec<u8>,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawGetKeyTtlRequest {
    let mut req = kvrpcpb::RawGetKeyTtlRequest::default();
    req.key = key;
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawGetKeyTtlRequest {
    type Response = kvrpcpb::RawGetKeyTtlResponse;
}

shardable_key!(kvrpcpb::RawGetKeyTtlRequest);
collect_single!(kvrpcpb::RawGetKeyTtlResponse);

impl SingleKey for kvrpcpb::RawGetKeyTtlRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

impl Process<kvrpcpb::RawGetKeyTtlResponse> for DefaultProcessor {
    type Out = Option<u64>;

    fn process(&self, input: Result<kvrpcpb::RawGetKeyTtlResponse>) -> Result<Self::Out> {
        let input = input?;
        Ok(if input.not_found {
            None
        } else {
            Some(input.ttl)
        })
    }
}

pub fn new_raw_put_request(
    key: Vec<u8>,
    value: Vec<u8>,
    ttl: u64,
    cf: Option<ColumnFamily>,
    atomic: bool,
) -> kvrpcpb::RawPutRequest {
    let mut req = kvrpcpb::RawPutRequest::default();
    req.key = key;
    req.value = value;
    req.ttl = ttl;
    req.maybe_set_cf(cf);
    req.for_cas = atomic;

    req
}

impl KvRequest for kvrpcpb::RawPutRequest {
    type Response = kvrpcpb::RawPutResponse;
}

shardable_key!(kvrpcpb::RawPutRequest);
collect_single!(kvrpcpb::RawPutResponse);
impl SingleKey for kvrpcpb::RawPutRequest {
    fn key(&self) -> &Vec<u8> {
        &self.key
    }
}

pub fn new_raw_batch_put_request(
    pairs: Vec<kvrpcpb::KvPair>,
    ttls: Vec<u64>,
    cf: Option<ColumnFamily>,
    atomic: bool,
) -> kvrpcpb::RawBatchPutRequest {
    let mut req = kvrpcpb::RawBatchPutRequest::default();
    req.pairs = pairs;
    req.ttls = ttls;
    req.maybe_set_cf(cf);
    req.for_cas = atomic;

    req
}

impl KvRequest for kvrpcpb::RawBatchPutRequest {
    type Response = kvrpcpb::RawBatchPutResponse;
}

impl Shardable for kvrpcpb::RawBatchPutRequest {
    type Shard = Vec<(kvrpcpb::KvPair, u64)>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        let kvs = self.pairs.clone();
        let ttls = self.ttls.clone();
        let mut kv_ttl: Vec<KvPairTTL> = kvs
            .into_iter()
            .zip(ttls)
            .map(|(kv, ttl)| KvPairTTL(kv, ttl))
            .collect();
        kv_ttl.sort_by(|a, b| a.0.key.cmp(&b.0.key));
        store_stream_for_keys(kv_ttl.into_iter(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        let (pairs, ttls) = shard.into_iter().unzip();
        self.set_leader(&store.region_with_leader)?;
        self.pairs = pairs;
        self.ttls = ttls;
        Ok(())
    }
}

pub fn new_raw_delete_request(
    key: Vec<u8>,
    cf: Option<ColumnFamily>,
    atomic: bool,
) -> kvrpcpb::RawDeleteRequest {
    let mut req = kvrpcpb::RawDeleteRequest::default();
    req.key = key;
    req.maybe_set_cf(cf);
    req.for_cas = atomic;

    req
}

impl KvRequest for kvrpcpb::RawDeleteRequest {
    type Response = kvrpcpb::RawDeleteResponse;
}

shardable_key!(kvrpcpb::RawDeleteRequest);
collect_single!(kvrpcpb::RawDeleteResponse);
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
    req.keys = keys;
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
    req.start_key = start_key;
    req.end_key = end_key;
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawDeleteRangeRequest {
    type Response = kvrpcpb::RawDeleteRangeResponse;
}

range_request!(kvrpcpb::RawDeleteRangeRequest);
shardable_range!(kvrpcpb::RawDeleteRangeRequest);

pub fn new_raw_scan_request(
    start_key: Vec<u8>,
    end_key: Vec<u8>,
    limit: u32,
    key_only: bool,
    reverse: bool,
    cf: Option<ColumnFamily>,
) -> kvrpcpb::RawScanRequest {
    let mut req = kvrpcpb::RawScanRequest::default();
    if !reverse {
        req.start_key = start_key;
        req.end_key = end_key;
    } else {
        req.start_key = end_key;
        req.end_key = start_key;
    }
    req.limit = limit;
    req.key_only = key_only;
    req.reverse = reverse;
    req.maybe_set_cf(cf);

    req
}

impl KvRequest for kvrpcpb::RawScanRequest {
    type Response = kvrpcpb::RawScanResponse;
}

range_request!(kvrpcpb::RawScanRequest);
shardable_range!(kvrpcpb::RawScanRequest);

impl Merge<kvrpcpb::RawScanResponse> for Collect {
    type Out = Vec<KvPair>;

    fn merge(&self, input: Vec<Result<kvrpcpb::RawScanResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|resp| resp.kvs.into_iter().map(Into::into))
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
    req.ranges = ranges;
    req.each_limit = each_limit;
    req.key_only = key_only;
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
        self.set_leader(&store.region_with_leader)?;
        self.ranges = shard;
        Ok(())
    }
}

impl Merge<kvrpcpb::RawBatchScanResponse> for Collect {
    type Out = Vec<KvPair>;

    fn merge(&self, input: Vec<Result<kvrpcpb::RawBatchScanResponse>>) -> Result<Self::Out> {
        input
            .into_iter()
            .flat_map_ok(|resp| resp.kvs.into_iter().map(Into::into))
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
    req.key = key;
    req.value = value;
    match previous_value {
        Some(v) => req.previous_value = v,
        None => req.previous_not_exist = true,
    }
    req.maybe_set_cf(cf);
    req
}

impl KvRequest for kvrpcpb::RawCasRequest {
    type Response = kvrpcpb::RawCasResponse;
}

shardable_key!(kvrpcpb::RawCasRequest);
collect_single!(kvrpcpb::RawCasResponse);
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

type RawCoprocessorRequestDataBuilder =
    Arc<dyn Fn(metapb::Region, Vec<kvrpcpb::KeyRange>) -> Vec<u8> + Send + Sync>;

pub fn new_raw_coprocessor_request(
    copr_name: String,
    copr_version_req: String,
    ranges: Vec<kvrpcpb::KeyRange>,
    data_builder: RawCoprocessorRequestDataBuilder,
) -> RawCoprocessorRequest {
    let mut inner = kvrpcpb::RawCoprocessorRequest::default();
    inner.copr_name = copr_name;
    inner.copr_version_req = copr_version_req;
    inner.ranges = ranges;
    RawCoprocessorRequest {
        inner,
        data_builder,
    }
}

#[derive(Clone)]
pub struct RawCoprocessorRequest {
    inner: kvrpcpb::RawCoprocessorRequest,
    data_builder: RawCoprocessorRequestDataBuilder,
}

#[async_trait]
impl Request for RawCoprocessorRequest {
    async fn dispatch(
        &self,
        client: &TikvClient<Channel>,
        timeout: Duration,
    ) -> Result<Box<dyn Any>> {
        self.inner.dispatch(client, timeout).await
    }

    fn label(&self) -> &'static str {
        self.inner.label()
    }

    fn as_any(&self) -> &dyn Any {
        self.inner.as_any()
    }

    fn set_leader(&mut self, leader: &RegionWithLeader) -> Result<()> {
        self.inner.set_leader(leader)
    }

    fn set_api_version(&mut self, api_version: kvrpcpb::ApiVersion) {
        self.inner.set_api_version(api_version);
    }
}

impl KvRequest for RawCoprocessorRequest {
    type Response = kvrpcpb::RawCoprocessorResponse;
}

impl Shardable for RawCoprocessorRequest {
    type Shard = Vec<kvrpcpb::KeyRange>;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        store_stream_for_ranges(self.inner.ranges.clone(), pd_client.clone())
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.set_leader(&store.region_with_leader)?;
        self.inner.ranges.clone_from(&shard);
        self.inner.data = (self.data_builder)(store.region_with_leader.region.clone(), shard);
        Ok(())
    }
}

#[allow(clippy::type_complexity)]
impl
    Process<Vec<Result<ResponseWithShard<kvrpcpb::RawCoprocessorResponse, Vec<kvrpcpb::KeyRange>>>>>
    for DefaultProcessor
{
    type Out = Vec<(Vec<Range<Key>>, Vec<u8>)>;

    fn process(
        &self,
        input: Result<
            Vec<Result<ResponseWithShard<kvrpcpb::RawCoprocessorResponse, Vec<kvrpcpb::KeyRange>>>>,
        >,
    ) -> Result<Self::Out> {
        input?
            .into_iter()
            .map(|shard_resp| {
                shard_resp.map(|ResponseWithShard(resp, ranges)| {
                    (
                        ranges
                            .into_iter()
                            .map(|range| range.start_key.into()..range.end_key.into())
                            .collect(),
                        resp.data,
                    )
                })
            })
            .collect::<Result<Vec<_>>>()
    }
}

macro_rules! impl_raw_rpc_request {
    ($name: ident) => {
        impl RawRpcRequest for kvrpcpb::$name {
            fn set_cf(&mut self, cf: String) {
                self.cf = cf;
            }
        }
    };
}

impl_raw_rpc_request!(RawGetRequest);
impl_raw_rpc_request!(RawBatchGetRequest);
impl_raw_rpc_request!(RawGetKeyTtlRequest);
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

impl HasLocks for kvrpcpb::RawGetKeyTtlResponse {}

impl HasLocks for kvrpcpb::RawPutResponse {}

impl HasLocks for kvrpcpb::RawBatchPutResponse {}

impl HasLocks for kvrpcpb::RawDeleteResponse {}

impl HasLocks for kvrpcpb::RawBatchDeleteResponse {}

impl HasLocks for kvrpcpb::RawScanResponse {}

impl HasLocks for kvrpcpb::RawBatchScanResponse {}

impl HasLocks for kvrpcpb::RawDeleteRangeResponse {}

impl HasLocks for kvrpcpb::RawCasResponse {}

impl HasLocks for kvrpcpb::RawCoprocessorResponse {}

#[cfg(test)]
mod test {
    use std::any::Any;
    use std::collections::HashMap;
    use std::ops::Deref;
    use std::sync::Mutex;

    use super::*;
    use crate::backoff::DEFAULT_REGION_BACKOFF;
    use crate::backoff::OPTIMISTIC_BACKOFF;
    use crate::mock::MockKvClient;
    use crate::mock::MockPdClient;
    use crate::proto::kvrpcpb;
    use crate::request::Keyspace;
    use crate::request::Plan;

    #[rstest::rstest]
    #[case(Keyspace::Disable)]
    #[case(Keyspace::Enable { keyspace_id: 0 })]
    #[tokio::test]
    async fn test_raw_scan(#[case] keyspace: Keyspace) {
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
        let plan = crate::request::PlanBuilder::new(client, keyspace, scan)
            .resolve_lock(OPTIMISTIC_BACKOFF, keyspace)
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .merge(Collect)
            .plan();
        let scan = plan.execute().await.unwrap();

        assert_eq!(scan.len(), 49);
        // FIXME test the keys returned.
    }

    #[tokio::test]
    async fn test_raw_batch_put() -> Result<()> {
        let region1_kvs = vec![KvPair(vec![9].into(), vec![12])];
        let region1_ttls = vec![0];
        let region2_kvs = vec![
            KvPair(vec![11].into(), vec![12]),
            KvPair("FFF".to_string().as_bytes().to_vec().into(), vec![12]),
        ];
        let region2_ttls = vec![0, 1];

        let expected_map = HashMap::from([
            (region1_kvs.clone(), region1_ttls.clone()),
            (region2_kvs.clone(), region2_ttls.clone()),
        ]);

        let pairs: Vec<kvrpcpb::KvPair> = [region1_kvs, region2_kvs]
            .concat()
            .into_iter()
            .map(|kv| kv.into())
            .collect();
        let ttls = [region1_ttls, region2_ttls].concat();
        let cf = ColumnFamily::Default;

        let actual_map: Arc<Mutex<HashMap<Vec<KvPair>, Vec<u64>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let fut_actual_map = actual_map.clone();
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                let req: &kvrpcpb::RawBatchPutRequest = req.downcast_ref().unwrap();
                let kv_pair = req
                    .pairs
                    .clone()
                    .into_iter()
                    .map(|p| p.into())
                    .collect::<Vec<KvPair>>();
                let ttls = req.ttls.clone();
                fut_actual_map.lock().unwrap().insert(kv_pair, ttls);
                let resp = kvrpcpb::RawBatchPutResponse::default();
                Ok(Box::new(resp) as Box<dyn Any>)
            },
        )));

        let batch_put_request =
            new_raw_batch_put_request(pairs.clone(), ttls.clone(), Some(cf), false);
        let keyspace = Keyspace::Enable { keyspace_id: 0 };
        let plan = crate::request::PlanBuilder::new(client, keyspace, batch_put_request)
            .resolve_lock(OPTIMISTIC_BACKOFF, keyspace)
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .plan();
        let _ = plan.execute().await;
        assert_eq!(actual_map.lock().unwrap().deref(), &expected_map);
        Ok(())
    }
}
