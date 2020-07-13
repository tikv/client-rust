// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Various mock versions of the various clients and other objects.
//!
//! The goal is to be able to test functionality independently of the rest of
//! the system, in particular without requiring a TiKV or PD server, or RPC layer.

use crate::{
    pd::{PdClient, PdRpcClient, RetryClient},
    request::{DispatchHook, KvRequest},
    Config, Error, Key, Result, Timestamp,
};
use fail::fail_point;
use futures::{
    compat::Compat01As03,
    future::{ready, BoxFuture, FutureExt},
};
use grpcio::CallOption;
use kvproto::{errorpb, kvrpcpb, metapb, tikvpb::TikvClient};
use std::{future::Future, sync::Arc, time::Duration};
use tikv_client_common::{Region, RegionId, StoreBuilder};
use tikv_client_store::{HasError, KvClient, KvConnect};

/// Create a `PdRpcClient` with it's internals replaced with mocks so that the
/// client can be tested without doing any RPC calls.
pub async fn pd_rpc_client() -> PdRpcClient<MockCluster> {
    let config = Config::default();
    PdRpcClient::new(&config, |e, sm| {
        futures::future::ok(RetryClient::new_with_cluster(
            e,
            sm,
            config.timeout,
            MockCluster,
        ))
    })
    .await
    .unwrap()
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct MockKvClient {
    addr: String,
}

pub struct MockKvConnect;

pub struct MockCluster;

pub struct MockPdClient;

impl KvClient for MockKvClient {
    fn dispatch<Resp, RpcFuture>(
        &self,
        request_name: &'static str,
        fut: grpcio::Result<RpcFuture>,
    ) -> BoxFuture<'static, Result<Resp>>
    where
        Compat01As03<RpcFuture>: Future<Output = std::result::Result<Resp, ::grpcio::Error>>,
        Resp: HasError + Sized + Clone + Send + 'static,
        RpcFuture: Send + 'static,
    {
        unimplemented!()
    }

    fn get_rpc_client(&self) -> Arc<TikvClient> {
        unimplemented!()
    }
}

impl KvConnect for MockKvConnect {
    type KvClient = MockKvClient;

    fn connect(&self, address: &str) -> Result<Self::KvClient> {
        Ok(MockKvClient {
            addr: address.to_owned(),
        })
    }
}

impl MockPdClient {
    pub fn region1() -> Region {
        let mut region = Region::default();
        region.region.id = 1;
        region.region.set_start_key(vec![0]);
        region.region.set_end_key(vec![10]);

        let mut leader = metapb::Peer::default();
        leader.store_id = 41;
        region.leader = Some(leader);

        region
    }

    pub fn region2() -> Region {
        let mut region = Region::default();
        region.region.id = 2;
        region.region.set_start_key(vec![10]);
        region.region.set_end_key(vec![250, 250]);

        let mut leader = metapb::Peer::default();
        leader.store_id = 42;
        region.leader = Some(leader);

        region
    }
}

impl PdClient for MockPdClient {
    fn map_region_to_store_builder(
        self: Arc<Self>,
        region: Region,
    ) -> BoxFuture<'static, Result<StoreBuilder>> {
        Box::pin(ready(Ok(StoreBuilder::new(
            region,
            String::new(),
            Duration::from_secs(60),
        ))))
    }

    fn region_for_key(&self, key: &Key) -> BoxFuture<'static, Result<Region>> {
        let bytes: &[_] = key.into();
        let region = if bytes.is_empty() || bytes[0] < 10 {
            Self::region1()
        } else {
            Self::region2()
        };

        Box::pin(ready(Ok(region)))
    }

    fn region_for_id(&self, id: RegionId) -> BoxFuture<'static, Result<Region>> {
        let result = match id {
            1 => Ok(Self::region1()),
            2 => Ok(Self::region2()),
            _ => Err(Error::region_not_found(id)),
        };

        Box::pin(ready(result))
    }

    fn get_timestamp(self: Arc<Self>) -> BoxFuture<'static, Result<Timestamp>> {
        unimplemented!()
    }
}

impl DispatchHook for kvrpcpb::ResolveLockRequest {
    fn dispatch_hook(
        &self,
        _opt: CallOption,
    ) -> Option<BoxFuture<'static, Result<kvrpcpb::ResolveLockResponse>>> {
        fail_point!("region-error", |_| {
            let mut resp = kvrpcpb::ResolveLockResponse::default();
            resp.region_error = Some(errorpb::Error::default());
            Some(ready(Ok(resp)).boxed())
        });
        Some(ready(Ok(kvrpcpb::ResolveLockResponse::default())).boxed())
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::mock::*;

    use futures::{executor, executor::block_on};
    use tikv_client_common::BoundRange;

    // TODO: implement client cache?
    // #[test]
    // fn test_kv_client_caching() {
    //     let client = block_on(pd_rpc_client());
    //
    //     let addr1 = "foo";
    //     let addr2 = "bar";
    //
    //     let kv1 = client.kv_client(&addr1).unwrap();
    //     let kv2 = client.kv_client(&addr2).unwrap();
    //     let kv3 = client.kv_client(&addr2).unwrap();
    //     assert!(kv1 != kv2);
    //     assert_eq!(kv2, kv3);
    // }

    #[test]
    fn test_group_keys_by_region() {
        let client = MockPdClient;

        // FIXME This only works if the keys are in order of regions. Not sure if
        // that is a reasonable constraint.
        let tasks: Vec<Key> = vec![
            vec![1].into(),
            vec![2].into(),
            vec![3].into(),
            vec![5, 2].into(),
            vec![12].into(),
            vec![11, 4].into(),
        ];

        let stream = Arc::new(client).group_keys_by_region(tasks.into_iter());
        let mut stream = executor::block_on_stream(stream);

        assert_eq!(
            stream.next().unwrap().unwrap().1,
            vec![
                vec![1].into(),
                vec![2].into(),
                vec![3].into(),
                vec![5, 2].into()
            ]
        );
        assert_eq!(
            stream.next().unwrap().unwrap().1,
            vec![vec![12].into(), vec![11, 4].into()]
        );
        assert!(stream.next().is_none());
    }

    #[test]
    fn test_stores_for_range() {
        let client = Arc::new(MockPdClient);
        let k1: Key = vec![1].into();
        let k2: Key = vec![5, 2].into();
        let k3: Key = vec![11, 4].into();
        let range1 = (k1, k2.clone()).into();
        let mut stream = executor::block_on_stream(client.clone().stores_for_range(range1));
        assert_eq!(stream.next().unwrap().unwrap().region.id(), 1);
        assert!(stream.next().is_none());

        let range2 = (k2, k3).into();
        let mut stream = executor::block_on_stream(client.stores_for_range(range2));
        assert_eq!(stream.next().unwrap().unwrap().region.id(), 1);
        assert_eq!(stream.next().unwrap().unwrap().region.id(), 2);
        assert!(stream.next().is_none());
    }

    #[test]
    fn test_group_ranges_by_region() {
        let client = Arc::new(MockPdClient);
        let k1: Key = vec![1].into();
        let k2: Key = vec![5, 2].into();
        let k3: Key = vec![11, 4].into();
        let k4: Key = vec![16, 4].into();
        let k_split: Key = vec![10].into();
        let range1 = (k1.clone(), k2.clone()).into();
        let range2 = (k1.clone(), k3.clone()).into();
        let range3 = (k2.clone(), k4.clone()).into();
        let ranges: Vec<BoundRange> = vec![range1, range2, range3];

        let mut stream = executor::block_on_stream(client.group_ranges_by_region(ranges));
        let ranges1 = stream.next().unwrap().unwrap();
        let ranges2 = stream.next().unwrap().unwrap();
        let ranges3 = stream.next().unwrap().unwrap();
        let ranges4 = stream.next().unwrap().unwrap();

        assert_eq!(ranges1.0, 1);
        assert_eq!(
            ranges1.1,
            vec![
                (k1.clone(), k2.clone()).into(),
                (k1.clone(), k_split.clone()).into()
            ] as Vec<BoundRange>
        );
        assert_eq!(ranges2.0, 2);
        assert_eq!(
            ranges2.1,
            vec![(k_split.clone(), k3.clone()).into()] as Vec<BoundRange>
        );
        assert_eq!(ranges3.0, 1);
        assert_eq!(
            ranges3.1,
            vec![(k2.clone(), k_split.clone()).into()] as Vec<BoundRange>
        );
        assert_eq!(ranges4.0, 2);
        assert_eq!(
            ranges4.1,
            vec![(k_split.clone(), k4.clone()).into()] as Vec<BoundRange>
        );
        assert!(stream.next().is_none());
    }
}
