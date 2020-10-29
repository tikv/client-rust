// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Various mock versions of the various clients and other objects.
//!
//! The goal is to be able to test functionality independently of the rest of
//! the system, in particular without requiring a TiKV or PD server, or RPC layer.

use crate::{
    pd::{PdClient, PdRpcClient, RetryClient},
    request::DispatchHook,
    Config, Error, Key, Result, Timestamp,
};
use async_trait::async_trait;
use fail::fail_point;
use futures::future::{ready, BoxFuture, FutureExt};
use kvproto::{errorpb, kvrpcpb, metapb};
use std::sync::Arc;
use tikv_client_store::{HasError, KvClient, KvConnect, Region, RegionId, RpcFnType, Store};

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

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct MockKvClient {
    addr: String,
}

pub struct MockKvConnect;

pub struct MockCluster;

pub struct MockPdClient;

#[async_trait]
impl KvClient for MockKvClient {
    async fn dispatch<Req, Resp>(&self, _fun: RpcFnType<Req, Resp>, _request: Req) -> Result<Resp>
    where
        Req: Send + Sync + 'static,
        Resp: HasError + Sized + Clone + Send + 'static,
    {
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

#[async_trait]
impl PdClient for MockPdClient {
    type KvClient = MockKvClient;

    async fn map_region_to_store(self: Arc<Self>, region: Region) -> Result<Store<Self::KvClient>> {
        Ok(Store::new(
            region,
            MockKvClient {
                addr: String::new(),
            },
        ))
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
            _ => Err(Error::region_not_found(id)),
        }
    }

    async fn get_timestamp(self: Arc<Self>) -> Result<Timestamp> {
        unimplemented!()
    }

    async fn update_safepoint(self: Arc<Self>, _safepoint: u64) -> Result<bool> {
        unimplemented!()
    }
}

impl DispatchHook for kvrpcpb::ResolveLockRequest {
    fn dispatch_hook(&self) -> Option<BoxFuture<'static, Result<kvrpcpb::ResolveLockResponse>>> {
        fail_point!("region-error", |_| {
            let mut resp = kvrpcpb::ResolveLockResponse::default();
            resp.region_error = Some(errorpb::Error::default());
            Some(ready(Ok(resp)).boxed())
        });
        Some(ready(Ok(kvrpcpb::ResolveLockResponse::default())).boxed())
    }
}
