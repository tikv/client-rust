// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{pd::PdClient, Timestamp};
use futures::future::ready;
use kvproto::metapb;
use std::time::Duration;
use tikv_client_store::{KvRpcClient, Region, Store};

// Mocked pd client that always returns a TikvClient
pub struct MockRpcPdClient {
    client: KvRpcClient,
}

impl MockRpcPdClient {
    pub fn new(client: KvRpcClient) -> MockRpcPdClient {
        MockRpcPdClient { client }
    }

    pub fn region() -> Region {
        let mut region = Region::default();
        region.region.id = 1;
        region.region.set_start_key(vec![0]);
        region.region.set_end_key(vec![250, 250]);

        let mut leader = metapb::Peer::default();
        leader.store_id = 41;
        region.leader = Some(leader);

        region
    }
}

impl PdClient for MockRpcPdClient {
    type KvClient = KvRpcClient;

    fn map_region_to_store(
        self: std::sync::Arc<Self>,
        region: tikv_client_store::Region,
    ) -> futures::future::BoxFuture<
        'static,
        tikv_client_common::Result<tikv_client_store::Store<Self::KvClient>>,
    > {
        Box::pin(ready(Ok(Store::new(
            region,
            self.client.clone(),
            Duration::from_secs(60),
        ))))
    }

    fn region_for_key(
        &self,
        _key: &tikv_client_common::Key,
    ) -> futures::future::BoxFuture<'static, tikv_client_common::Result<tikv_client_store::Region>>
    {
        Box::pin(ready(Ok(Self::region())))
    }

    fn region_for_id(
        &self,
        _id: tikv_client_store::RegionId,
    ) -> futures::future::BoxFuture<'static, tikv_client_common::Result<tikv_client_store::Region>>
    {
        Box::pin(ready(Ok(Self::region())))
    }

    fn get_timestamp(
        self: std::sync::Arc<Self>,
    ) -> futures::future::BoxFuture<'static, tikv_client_common::Result<Timestamp>> {
        todo!()
    }
}
