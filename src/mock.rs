// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Various mock versions of the various clients and other objects.
//!
//! The goal is to be able to test functionality independently of the rest of
//! the system, in particular without requiring a TiKV or PD server, or RPC layer.

use crate::{
    pd::{PdClient, PdRpcClient, RetryClient},
    store::Store,
    Config, Error, Key, Region, RegionId, Result, Timestamp,
};
use async_trait::async_trait;
use derive_new::new;
use std::{any::Any, sync::Arc};
use tikv_client_proto::metapb;
use tikv_client_store::{KvClient, KvConnect, Request};

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
        false,
    )
    .await
    .unwrap()
}

#[derive(new, Default, Clone)]
pub struct MockKvClient {
    pub addr: String,
    dispatch: Option<Arc<dyn Fn(&dyn Any) -> Result<Box<dyn Any>> + Send + Sync + 'static>>,
}

impl MockKvClient {
    pub fn with_dispatch_hook<F>(dispatch: F) -> MockKvClient
    where
        F: Fn(&dyn Any) -> Result<Box<dyn Any>> + Send + Sync + 'static,
    {
        MockKvClient {
            addr: String::new(),
            dispatch: Some(Arc::new(dispatch)),
        }
    }
}

pub struct MockKvConnect;

pub struct MockCluster;

#[derive(new)]
pub struct MockPdClient {
    client: MockKvClient,
}

#[async_trait]
impl KvClient for MockKvClient {
    async fn dispatch(&self, req: &dyn Request) -> Result<Box<dyn Any>> {
        match &self.dispatch {
            Some(f) => f(req.as_any()),
            None => panic!("no dispatch hook set"),
        }
    }
}

impl KvConnect for MockKvConnect {
    type KvClient = MockKvClient;

    fn connect(&self, address: &str) -> Result<Self::KvClient> {
        Ok(MockKvClient {
            addr: address.to_owned(),
            dispatch: None,
        })
    }
}

impl MockPdClient {
    pub fn default() -> MockPdClient {
        MockPdClient {
            client: MockKvClient::default(),
        }
    }

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

#[async_trait]
impl PdClient for MockPdClient {
    type KvClient = MockKvClient;

    async fn map_region_to_store(self: Arc<Self>, region: Region) -> Result<Store> {
        Ok(Store::new(region, Arc::new(self.client.clone())))
    }

    async fn region_for_key(&self, key: &Key) -> Result<Region> {
        let bytes: &[_] = key.into();
        let region = if bytes.is_empty() || bytes[0] < 10 {
            Self::region1()
        } else {
            Self::region2()
        };

        Ok(region)
    }

    async fn region_for_id(&self, id: RegionId) -> Result<Region> {
        match id {
            1 => Ok(Self::region1()),
            2 => Ok(Self::region2()),
            _ => Err(Error::RegionNotFound { region_id: id }),
        }
    }

    async fn get_timestamp(self: Arc<Self>) -> Result<Timestamp> {
        Ok(Timestamp::default())
    }

    async fn update_safepoint(self: Arc<Self>, _safepoint: u64) -> Result<bool> {
        unimplemented!()
    }
}

pub fn mock_store() -> Store {
    Store {
        region: Region::default(),
        client: Arc::new(MockKvClient::new("foo".to_owned(), None)),
    }
}
