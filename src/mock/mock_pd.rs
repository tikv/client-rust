// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::{MockKvClient, MockKvConnect};
use crate::{
    pd::{PdClient, PdRpcClient, RetryClient},
    Config, Error, Key, Result, Timestamp,
};

use futures::future::{ready, BoxFuture};

use kvproto::metapb;
use std::{sync::Arc, time::Duration};

use tikv_client_store::{Region, RegionId, Store};

/// Create a `PdRpcClient` with it's internals replaced with mocks so that the
/// client can be tested without doing any RPC calls.
pub async fn pd_rpc_client() -> PdRpcClient<MockKvConnect, MockCluster> {
    let config = Config::default();
    PdRpcClient::new(
        &config,
        |_, _| MockKvConnect,
        |e, sm| {
            futures::future::ok(RetryClient::new_with_cluster(
                e,
                sm,
                config.timeout,
                MockCluster,
            ))
        },
    )
    .await
    .unwrap()
}
pub struct MockPdClient;

pub struct MockCluster;

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
    type KvClient = MockKvClient;

    fn map_region_to_store(
        self: Arc<Self>,
        region: Region,
    ) -> BoxFuture<'static, Result<Store<Self::KvClient>>> {
        Box::pin(ready(Ok(Store::new(
            region,
            MockKvClient {
                addr: String::new(),
            },
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
