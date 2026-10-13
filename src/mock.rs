// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Various mock versions of the various clients and other objects.
//!
//! The goal is to be able to test functionality independently of the rest of
//! the system, in particular without requiring a TiKV or PD server, or RPC layer.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use derive_new::new;

use crate::pd::PdClient;
use crate::pd::PdRpcClient;
use crate::pd::RetryClient;
use crate::proto::keyspacepb;
use crate::proto::metapb::RegionEpoch;
use crate::proto::metapb::{self};
use crate::region::RegionId;
use crate::region::RegionVerId;
use crate::region::RegionWithLeader;
use crate::store::KvConnect;
use crate::store::RegionStore;
use crate::store::Request;
use crate::store::TxnProtocolVersionRange;
use crate::store::{KvClient, Store};
use crate::Config;
use crate::Error;
use crate::Key;
use crate::Result;
use crate::Timestamp;

/// Create a `PdRpcClient` with it's internals replaced with mocks so that the
/// client can be tested without doing any RPC calls.
pub async fn pd_rpc_client() -> PdRpcClient<MockKvConnect, MockCluster> {
    let config = Config::default();
    PdRpcClient::new(
        config.clone(),
        |_| MockKvConnect,
        |sm| {
            futures::future::ok(RetryClient::new_with_cluster(
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

/// Outcomes used to exercise admission retry, rather than just the selector.
#[derive(Clone, Copy, Debug)]
pub enum ReloadOutcome {
    Changed,
    Unchanged,
    Invalid,
    Missing,
    Failed,
}

pub fn upper_admission_rejection() -> crate::proto::errorpb::IncompatibleRequest {
    crate::proto::errorpb::IncompatibleRequest {
        reason: crate::proto::errorpb::IncompatibleRequestReason::TxnProtocolVersionOutOfRange
            as i32,
        message: "store protocol range changed".into(),
        provided_txn_protocol_version: 1,
        min_compatible_txn_protocol_version: 0,
        max_compatible_txn_protocol_version: 0,
    }
}

/// Models a PD reload updating the metadata read by later routing attempts.
pub struct ProtocolTestClient {
    pub pd_client: Arc<MockPdClient>,
    pub reloads: Arc<Mutex<Vec<u64>>>,
}

impl ProtocolTestClient {
    pub fn new(client: MockKvClient, outcome: ReloadOutcome) -> Self {
        let range = Arc::new(Mutex::new(HashMap::<u64, TxnProtocolVersionRange>::new()));
        let mapped_range = range.clone();
        let mapped_client = client.clone();
        let reloads = Arc::new(Mutex::new(Vec::new()));
        let tracked_reloads = reloads.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client)
                .with_map_region_to_store_hook(move |region| {
                    let store_id = region.get_store_id()?;
                    Ok(RegionStore::with_metadata(
                        region,
                        store_id,
                        mapped_range
                            .lock()
                            .unwrap()
                            .get(&store_id)
                            .copied()
                            .unwrap_or(TxnProtocolVersionRange { min: 0, max: 1 }),
                        Arc::new(mapped_client.clone()),
                    ))
                })
                .with_reload_store_hook(move |store_id| {
                    tracked_reloads.lock().unwrap().push(store_id);
                    let new_range = match outcome {
                        ReloadOutcome::Changed => TxnProtocolVersionRange { min: 0, max: 0 },
                        ReloadOutcome::Unchanged => TxnProtocolVersionRange { min: 0, max: 1 },
                        ReloadOutcome::Invalid => TxnProtocolVersionRange { min: 2, max: 1 },
                        ReloadOutcome::Missing => return Ok(None),
                        ReloadOutcome::Failed => {
                            return Err(Error::StringError("PD unavailable".into()))
                        }
                    };
                    range.lock().unwrap().insert(store_id, new_range);
                    Ok(Some(new_range))
                }),
        );
        Self { pd_client, reloads }
    }
}

#[allow(clippy::type_complexity)]
#[derive(new)]
pub struct MockPdClient {
    client: MockKvClient,
    /// Optional override for `map_region_to_store`.
    #[new(default)]
    map_region_to_store_hook:
        Option<Arc<dyn Fn(RegionWithLeader) -> Result<RegionStore> + Send + Sync + 'static>>,
    /// Optional override for `region_for_key`, e.g. to simulate PD failing to
    /// locate a region for a key.
    #[new(default)]
    region_for_key_hook:
        Option<Arc<dyn Fn(&Key) -> Result<RegionWithLeader> + Send + Sync + 'static>>,
    /// Optional observer/override for leader cache updates.
    #[new(default)]
    update_leader_hook:
        Option<Arc<dyn Fn(RegionVerId, metapb::Peer) -> Result<()> + Send + Sync + 'static>>,
    /// Optional observer for region cache invalidations.
    #[new(default)]
    invalidate_region_hook: Option<Arc<dyn Fn(RegionVerId) + Send + Sync + 'static>>,
    /// Optional override for conditional Store metadata reloads.
    #[new(default)]
    reload_store_hook:
        Option<Arc<dyn Fn(u64) -> Result<Option<TxnProtocolVersionRange>> + Send + Sync + 'static>>,
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

#[async_trait]
impl KvConnect for MockKvConnect {
    type KvClient = MockKvClient;

    async fn connect(&self, address: &str) -> Result<Self::KvClient> {
        Ok(MockKvClient {
            addr: address.to_owned(),
            dispatch: None,
        })
    }
}

impl MockPdClient {
    pub fn default() -> MockPdClient {
        MockPdClient::new(MockKvClient::default())
    }

    pub fn with_map_region_to_store_hook<F>(mut self, hook: F) -> MockPdClient
    where
        F: Fn(RegionWithLeader) -> Result<RegionStore> + Send + Sync + 'static,
    {
        self.map_region_to_store_hook = Some(Arc::new(hook));
        self
    }

    /// Override `region_for_key` with a custom hook, leaving the rest of the
    /// mock's behavior untouched.
    pub fn with_region_for_key_hook<F>(mut self, hook: F) -> MockPdClient
    where
        F: Fn(&Key) -> Result<RegionWithLeader> + Send + Sync + 'static,
    {
        self.region_for_key_hook = Some(Arc::new(hook));
        self
    }

    pub fn with_update_leader_hook<F>(mut self, hook: F) -> MockPdClient
    where
        F: Fn(RegionVerId, metapb::Peer) -> Result<()> + Send + Sync + 'static,
    {
        self.update_leader_hook = Some(Arc::new(hook));
        self
    }

    pub fn with_invalidate_region_hook<F>(mut self, hook: F) -> MockPdClient
    where
        F: Fn(RegionVerId) + Send + Sync + 'static,
    {
        self.invalidate_region_hook = Some(Arc::new(hook));
        self
    }

    pub fn with_reload_store_hook<F>(mut self, hook: F) -> MockPdClient
    where
        F: Fn(u64) -> Result<Option<TxnProtocolVersionRange>> + Send + Sync + 'static,
    {
        self.reload_store_hook = Some(Arc::new(hook));
        self
    }

    pub fn region1() -> RegionWithLeader {
        let mut region = RegionWithLeader::default();
        region.region.id = 1;
        region.region.start_key = vec![];
        region.region.end_key = vec![10];
        region.region.region_epoch = Some(RegionEpoch {
            conf_ver: 0,
            version: 0,
        });

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
        region.region.start_key = vec![10];
        region.region.end_key = vec![250, 250];
        region.region.region_epoch = Some(RegionEpoch {
            conf_ver: 0,
            version: 0,
        });

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
        region.region.start_key = vec![250, 250];
        region.region.end_key = vec![];
        region.region.region_epoch = Some(RegionEpoch {
            conf_ver: 0,
            version: 0,
        });

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
        if let Some(hook) = &self.map_region_to_store_hook {
            return hook(region);
        }
        Ok(RegionStore::new(region, Arc::new(self.client.clone())))
    }

    async fn region_for_key(&self, key: &Key) -> Result<RegionWithLeader> {
        if let Some(hook) = &self.region_for_key_hook {
            return hook(key);
        }
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

    async fn all_stores(&self) -> Result<Vec<Store>> {
        Ok(vec![Store::new(Arc::new(self.client.clone()))])
    }

    async fn reload_store(
        self: Arc<Self>,
        store_id: u64,
    ) -> Result<Option<TxnProtocolVersionRange>> {
        match &self.reload_store_hook {
            Some(hook) => hook(store_id),
            None => Ok(None),
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
        ver_id: crate::region::RegionVerId,
        leader: metapb::Peer,
    ) -> Result<()> {
        match &self.update_leader_hook {
            Some(hook) => hook(ver_id, leader),
            None => todo!(),
        }
    }

    async fn invalidate_region_cache(&self, ver_id: crate::region::RegionVerId) {
        if let Some(hook) = &self.invalidate_region_hook {
            hook(ver_id);
        }
    }

    async fn invalidate_store_cache(&self, _store_id: crate::region::StoreId) {}

    async fn load_keyspace(&self, _keyspace: &str) -> Result<keyspacepb::KeyspaceMeta> {
        unimplemented!()
    }
}
