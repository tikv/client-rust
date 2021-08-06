// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Various mock versions of the various clients and other objects.
//!
//! The goal is to be able to test functionality independently of the rest of
//! the system, in particular without requiring a TiKV or PD server, or RPC layer.

use crate::{
    pd::{PdClient, PdRpcClient, RetryClient},
    region::{RegionId, RegionWithLeader},
    store::RegionStore,
    Config, Error, Key, Result, Timestamp,
};
use async_trait::async_trait;
use derive_new::new;
use slog::{Drain, Logger};
use std::{any::Any, sync::Arc};
use tikv_client_proto::metapb;
use tikv_client_store::{KvClient, KvConnect, Request};

/// Create a `PdRpcClient` with it's internals replaced with mocks so that the
/// client can be tested without doing any RPC calls.
pub async fn pd_rpc_client() -> PdRpcClient<MockKvConnect, MockCluster> {
    let config = Config::default();
    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
    let logger = Logger::root(
        slog_term::FullFormat::new(plain)
            .build()
            .filter_level(slog::Level::Info)
            .fuse(),
        o!(),
    );
    PdRpcClient::new(
        config.clone(),
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
        logger,
    )
    .await
    .unwrap()
}

#[allow(clippy::type_complexity)]
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

    pub fn region1() -> RegionWithLeader {
        let mut region = RegionWithLeader::default();
        region.region.id = 1;
        region.region.set_start_key(vec![]);
        region.region.set_end_key(vec![10]);

        let leader = metapb::Peer {
            store_id: 41,
            ..Default::default()
        };
        region.leader = Some(leader);

        region
    }

    pub fn region2() -> RegionWithLeader {
        let mut region = RegionWithLeader::default();
        region.region.id = 2;
        region.region.set_start_key(vec![10]);
        region.region.set_end_key(vec![250, 250]);

        let leader = metapb::Peer {
            store_id: 42,
            ..Default::default()
        };
        region.leader = Some(leader);

        region
    }

    pub fn region3() -> RegionWithLeader {
        let mut region = RegionWithLeader::default();
        region.region.id = 3;
        region.region.set_start_key(vec![250, 250]);
        region.region.set_end_key(vec![]);

        let leader = metapb::Peer {
            store_id: 43,
            ..Default::default()
        };
        region.leader = Some(leader);

        region
    }
}

#[async_trait]
impl PdClient for MockPdClient {
    type KvClient = MockKvClient;

    async fn map_region_to_store(self: Arc<Self>, region: RegionWithLeader) -> Result<RegionStore> {
        Ok(RegionStore::new(region, Arc::new(self.client.clone())))
    }

    async fn region_for_key(&self, key: &Key) -> Result<RegionWithLeader> {
        let bytes: &[_] = key.into();
        let region = if bytes.is_empty() || bytes < &[10][..] {
            Self::region1()
        } else if bytes >= &[10][..] && bytes < &[250, 250][..] {
            Self::region2()
        } else {
            Self::region3()
        };

        Ok(region)
    }

    async fn region_for_id(&self, id: RegionId) -> Result<RegionWithLeader> {
        match id {
            1 => Ok(Self::region1()),
            2 => Ok(Self::region2()),
            3 => Ok(Self::region3()),
            _ => Err(Error::RegionNotFoundInResponse { region_id: id }),
        }
    }

    async fn get_timestamp(self: Arc<Self>) -> Result<Timestamp> {
        Ok(Timestamp::default())
    }

    async fn update_safepoint(self: Arc<Self>, _safepoint: u64) -> Result<bool> {
        unimplemented!()
    }

    async fn update_leader(
        &self,
        _ver_id: crate::region::RegionVerId,
        _leader: metapb::Peer,
    ) -> Result<()> {
        todo!()
    }

    async fn invalidate_region_cache(&self, _ver_id: crate::region::RegionVerId) {}
}
