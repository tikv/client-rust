// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Various mock versions of the various clients and other objects.
//!
//! The goal is to be able to test functionality independently of the rest of
//! the system, in particular without requiring a TiKV or PD server, or RPC layer.

use crate::{
    kv_client::{KvClient, KvConnect, Store},
    pd::{PdClient, PdRpcClient, Region, RegionId, RetryClient},
    request::{DispatchHook, KvRequest},
    transaction::Timestamp,
    Config, Error, Key, Result,
};

use fail::fail_point;
use futures::future::{ready, BoxFuture, FutureExt};
use grpcio::CallOption;
use kvproto::{errorpb, kvrpcpb, metapb};
use std::{sync::Arc, time::Duration};

/// Create a `PdRpcClient` with it's internals replaced with mocks so that the
/// client can be tested without doing any RPC calls.
pub fn pd_rpc_client() -> PdRpcClient<MockKvConnect, MockCluster> {
    let config = Config::default();
    PdRpcClient::new(
        &config,
        |_, _| MockKvConnect,
        |e, sm| {
            Ok(RetryClient::new_with_cluster(
                e,
                sm,
                config.timeout,
                MockCluster,
            ))
        },
    )
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
    fn dispatch<T: KvRequest>(
        &self,
        _request: &T,
        _opt: CallOption,
    ) -> BoxFuture<'static, Result<T::RpcResponse>> {
        unreachable!()
    }
}

impl KvConnect for MockKvConnect {
    type KvClient = MockKvClient;

    //TODO 异步化
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
