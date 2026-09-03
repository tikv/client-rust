// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use async_trait::async_trait;
use derive_new::new;

pub use self::keyspace::EncodeKeyspace;
pub use self::keyspace::KeyMode;
pub use self::keyspace::Keyspace;
pub use self::keyspace::TruncateKeyspace;
pub use self::plan::Collect;
pub use self::plan::CollectError;
pub use self::plan::CollectSingle;
pub use self::plan::CollectWithShard;
pub use self::plan::DefaultProcessor;
pub use self::plan::Dispatch;
pub use self::plan::ExtractError;
pub use self::plan::Merge;
pub use self::plan::MergeResponse;
pub use self::plan::Plan;
pub use self::plan::Process;
pub use self::plan::ProcessResponse;
pub use self::plan::ResolveLock;
pub use self::plan::ResponseWithShard;
pub use self::plan::RetryableMultiRegion;
pub use self::plan_builder::PlanBuilder;
pub use self::plan_builder::SingleKey;
pub use self::shard::Batchable;
pub use self::shard::HasNextBatch;
pub use self::shard::NextBatch;
pub use self::shard::RangeRequest;
pub use self::shard::Shardable;
use crate::backoff::Backoff;
use crate::backoff::DEFAULT_REGION_BACKOFF;
use crate::backoff::OPTIMISTIC_BACKOFF;
use crate::backoff::PESSIMISTIC_BACKOFF;
use crate::store::Request;
use crate::store::{HasKeyErrors, Store};
use crate::transaction::HasLocks;

mod keyspace;
pub mod plan;
mod plan_builder;
mod shard;

/// Abstracts any request sent to a TiKV server.
#[async_trait]
pub trait KvRequest: Request + Sized + Clone + Sync + Send + 'static {
    /// The expected response to the request.
    type Response: HasKeyErrors + HasLocks + Clone + Send + 'static;
}

/// For requests or plans which are handled at TiKV store (other than region) level.
pub trait StoreRequest {
    /// Apply the request to specified TiKV store.
    fn apply_store(&mut self, store: &Store);
}

#[derive(Clone, Debug, new, Eq, PartialEq)]
pub struct RetryOptions {
    /// How to retry when there is a region error and we need to resolve regions with PD.
    pub region_backoff: Backoff,
    /// How to retry when a key is locked.
    pub lock_backoff: Backoff,
}

impl RetryOptions {
    pub const fn default_optimistic() -> RetryOptions {
        RetryOptions {
            region_backoff: DEFAULT_REGION_BACKOFF,
            lock_backoff: OPTIMISTIC_BACKOFF,
        }
    }

    pub const fn default_pessimistic() -> RetryOptions {
        RetryOptions {
            region_backoff: DEFAULT_REGION_BACKOFF,
            lock_backoff: PESSIMISTIC_BACKOFF,
        }
    }

    pub const fn none() -> RetryOptions {
        RetryOptions {
            region_backoff: Backoff::no_backoff(),
            lock_backoff: Backoff::no_backoff(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::any::Any;
    use std::iter;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use tonic::transport::Channel;

    use super::*;
    use crate::mock::MockKvClient;
    use crate::mock::MockPdClient;
    use crate::proto::keyspacepb;
    use crate::proto::kvrpcpb;
    use crate::proto::metapb::{self, RegionEpoch};
    use crate::proto::pdpb::Timestamp;
    use crate::proto::tikvpb::tikv_client::TikvClient;
    use crate::region::{RegionId, RegionVerId, RegionWithLeader, StoreId};
    use crate::store::region_stream_for_keys;
    use crate::store::HasRegionError;
    use crate::store::{RegionStore, Store};
    use crate::transaction::lowering::new_commit_request;
    use crate::Error;
    use crate::Key;
    use crate::Result;

    #[tokio::test]
    async fn test_region_retry() {
        #[derive(Debug, Clone)]
        struct MockRpcResponse;

        impl HasKeyErrors for MockRpcResponse {
            fn key_errors(&mut self) -> Option<Vec<Error>> {
                None
            }
        }

        impl HasRegionError for MockRpcResponse {
            fn region_error(&mut self) -> Option<crate::proto::errorpb::Error> {
                Some(crate::proto::errorpb::Error::default())
            }
        }

        impl HasLocks for MockRpcResponse {}

        #[derive(Clone)]
        struct MockKvRequest {
            test_invoking_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl Request for MockKvRequest {
            async fn dispatch(&self, _: &TikvClient<Channel>, _: Duration) -> Result<Box<dyn Any>> {
                Ok(Box::new(MockRpcResponse {}))
            }

            fn label(&self) -> &'static str {
                "mock"
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn set_leader(&mut self, _: &RegionWithLeader) -> Result<()> {
                Ok(())
            }

            fn set_api_version(&mut self, _: kvrpcpb::ApiVersion) {}
        }

        #[async_trait]
        impl KvRequest for MockKvRequest {
            type Response = MockRpcResponse;
        }

        impl Shardable for MockKvRequest {
            type Shard = Vec<Vec<u8>>;

            fn shards(
                &self,
                pd_client: &std::sync::Arc<impl crate::pd::PdClient>,
            ) -> futures::stream::BoxStream<
                'static,
                crate::Result<(Self::Shard, crate::region::RegionWithLeader)>,
            > {
                // Increases by 1 for each call.
                self.test_invoking_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                region_stream_for_keys(
                    Some(Key::from("mock_key".to_owned())).into_iter(),
                    pd_client.clone(),
                )
            }

            fn apply_shard(&mut self, _shard: Self::Shard) {}

            fn apply_store(&mut self, _store: &crate::store::RegionStore) -> crate::Result<()> {
                Ok(())
            }
        }

        let invoking_count = Arc::new(AtomicUsize::new(0));

        let request = MockKvRequest {
            test_invoking_count: invoking_count.clone(),
        };

        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |_: &dyn Any| Ok(Box::new(MockRpcResponse) as Box<dyn Any>),
        )));

        let plan = crate::request::PlanBuilder::new(pd_client.clone(), Keyspace::Disable, request)
            .retry_multi_region(Backoff::no_jitter_backoff(1, 1, 3))
            .extract_error()
            .plan();
        let _ = plan.execute().await;

        // Original call plus the 3 retries
        assert_eq!(invoking_count.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_fallback_mapping_error_tries_remaining_voter() {
        #[derive(Debug, Clone)]
        struct MockOkResponse;

        impl HasKeyErrors for MockOkResponse {
            fn key_errors(&mut self) -> Option<Vec<Error>> {
                None
            }
        }

        impl HasRegionError for MockOkResponse {
            fn region_error(&mut self) -> Option<crate::proto::errorpb::Error> {
                None
            }
        }

        impl HasLocks for MockOkResponse {}

        struct FlakyStoreMappingPdClient {
            client: MockKvClient,
            mapped_store_ids: std::sync::Mutex<Vec<StoreId>>,
            invalidate_region_count: AtomicUsize,
            invalidated_store_ids: std::sync::Mutex<Vec<StoreId>>,
        }

        impl FlakyStoreMappingPdClient {
            fn region() -> RegionWithLeader {
                let mut region = RegionWithLeader::default();
                region.region.id = 1;
                region.region.start_key = vec![];
                region.region.end_key = vec![];
                region.region.region_epoch = Some(RegionEpoch {
                    conf_ver: 0,
                    version: 0,
                });
                let leader = metapb::Peer {
                    id: 1,
                    store_id: 41,
                    ..Default::default()
                };
                let follower = metapb::Peer {
                    id: 2,
                    store_id: 42,
                    ..Default::default()
                };
                let second_follower = metapb::Peer {
                    id: 3,
                    store_id: 43,
                    ..Default::default()
                };
                region.region.peers = vec![leader.clone(), follower, second_follower];
                region.leader = Some(leader);
                region
            }
        }

        #[async_trait]
        impl crate::pd::PdClient for FlakyStoreMappingPdClient {
            type KvClient = MockKvClient;

            async fn map_region_to_store(
                self: Arc<Self>,
                region: RegionWithLeader,
            ) -> Result<RegionStore> {
                let store_id = region.get_store_id()?;
                {
                    let mut mapped_store_ids = self.mapped_store_ids.lock().unwrap();
                    mapped_store_ids.push(store_id);
                }
                if store_id == 42 {
                    Err(Error::GrpcAPI(tonic::Status::unavailable(
                        "follower mapping failure",
                    )))
                } else {
                    Ok(RegionStore::new(region, Arc::new(self.client.clone())))
                }
            }

            async fn region_for_key(&self, _: &Key) -> Result<RegionWithLeader> {
                Ok(Self::region())
            }

            async fn region_for_id(&self, id: RegionId) -> Result<RegionWithLeader> {
                match id {
                    1 => self.region_for_key(&Key::EMPTY).await,
                    _ => Err(Error::RegionNotFoundInResponse { region_id: id }),
                }
            }

            async fn all_stores(&self) -> Result<Vec<Store>> {
                Ok(vec![Store::new(Arc::new(self.client.clone()))])
            }

            async fn get_timestamp(self: Arc<Self>) -> Result<Timestamp> {
                Ok(Timestamp::default())
            }

            async fn update_safepoint(self: Arc<Self>, _safepoint: u64) -> Result<bool> {
                unimplemented!()
            }

            async fn load_keyspace(&self, _keyspace: &str) -> Result<keyspacepb::KeyspaceMeta> {
                unimplemented!()
            }

            async fn update_leader(
                &self,
                _ver_id: RegionVerId,
                _leader: metapb::Peer,
            ) -> Result<()> {
                Ok(())
            }

            async fn invalidate_region_cache(&self, _ver_id: RegionVerId) {
                self.invalidate_region_count.fetch_add(1, Ordering::SeqCst);
            }

            async fn invalidate_store_cache(&self, store_id: StoreId) {
                self.invalidated_store_ids.lock().unwrap().push(store_id);
            }
        }

        #[derive(Clone)]
        struct MockKvRequest {
            shard_invoking_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl Request for MockKvRequest {
            async fn dispatch(&self, _: &TikvClient<Channel>, _: Duration) -> Result<Box<dyn Any>> {
                Ok(Box::new(MockOkResponse))
            }

            fn label(&self) -> &'static str {
                "mock"
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn set_leader(&mut self, _: &RegionWithLeader) -> Result<()> {
                Ok(())
            }

            fn set_api_version(&mut self, _: kvrpcpb::ApiVersion) {}
        }

        #[async_trait]
        impl KvRequest for MockKvRequest {
            type Response = MockOkResponse;
        }

        impl Shardable for MockKvRequest {
            type Shard = Vec<Vec<u8>>;

            fn shards(
                &self,
                pd_client: &Arc<impl crate::pd::PdClient>,
            ) -> futures::stream::BoxStream<
                'static,
                crate::Result<(Self::Shard, crate::region::RegionWithLeader)>,
            > {
                self.shard_invoking_count.fetch_add(1, Ordering::SeqCst);
                region_stream_for_keys(
                    Some(Key::from("mock_key".to_owned())).into_iter(),
                    pd_client.clone(),
                )
            }

            fn apply_shard(&mut self, _shard: Self::Shard) {}

            fn apply_store(&mut self, _store: &crate::store::RegionStore) -> crate::Result<()> {
                Ok(())
            }
        }

        let dispatch_count = Arc::new(AtomicUsize::new(0));
        let shard_invoking_count = Arc::new(AtomicUsize::new(0));
        let dispatch_count_clone = dispatch_count.clone();

        let pd_client = Arc::new(FlakyStoreMappingPdClient {
            client: MockKvClient::with_dispatch_hook(move |_: &dyn Any| {
                if dispatch_count_clone.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(Error::GrpcAPI(tonic::Status::unavailable(
                        "leader unavailable",
                    )))
                } else {
                    Ok(Box::new(MockOkResponse) as Box<dyn Any>)
                }
            }),
            mapped_store_ids: std::sync::Mutex::new(Vec::new()),
            invalidate_region_count: AtomicUsize::new(0),
            invalidated_store_ids: std::sync::Mutex::new(Vec::new()),
        });

        let request = MockKvRequest {
            shard_invoking_count: shard_invoking_count.clone(),
        };

        let plan = crate::request::PlanBuilder::new(pd_client.clone(), Keyspace::Disable, request)
            .retry_multi_region(Backoff::no_jitter_backoff(1, 1, 3))
            .plan();

        let response = plan.execute().await;
        assert!(response.is_ok());
        assert_eq!(dispatch_count.load(Ordering::SeqCst), 2);
        assert_eq!(shard_invoking_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            pd_client.mapped_store_ids.lock().unwrap().as_slice(),
            &[41, 42, 43],
            "a failed follower mapping must not hide the remaining healthy voter"
        );
        assert_eq!(pd_client.invalidate_region_count.load(Ordering::SeqCst), 0);
        assert_eq!(
            pd_client.invalidated_store_ids.lock().unwrap().as_slice(),
            &[41, 42],
            "both the failed leader RPC and failed follower mapping invalidate their Store entries"
        );
    }

    #[tokio::test]
    async fn test_all_mapping_errors_reload_stale_region() {
        let old_region = fallback_region(&[44]);
        let mut new_region = fallback_region(&[]);
        let new_leader = metapb::Peer {
            id: 3,
            store_id: 45,
            ..Default::default()
        };
        new_region.region.peers = vec![new_leader.clone()];
        new_region.leader = Some(new_leader);

        let region_invalidated = Arc::new(AtomicBool::new(false));
        let locate_invalidated = region_invalidated.clone();
        let invalidate_observer = region_invalidated.clone();
        let mapping_attempts = Arc::new(std::sync::Mutex::new(Vec::new()));
        let tracked_mapping_attempts = mapping_attempts.clone();
        let client = MockKvClient::with_dispatch_hook(|_: &dyn Any| {
            Ok(Box::new(kvrpcpb::GetResponse::default()) as Box<dyn Any>)
        });
        let mapping_client = client.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client)
                .with_region_for_key_hook(move |_| {
                    if locate_invalidated.load(Ordering::SeqCst) {
                        Ok(new_region.clone())
                    } else {
                        Ok(old_region.clone())
                    }
                })
                .with_map_region_to_store_hook(move |region| {
                    let store_id = region.get_store_id()?;
                    tracked_mapping_attempts.lock().unwrap().push(store_id);
                    if store_id == 41 || store_id == 44 {
                        Err(Error::GrpcAPI(tonic::Status::unavailable(
                            "stale peer mapping failed",
                        )))
                    } else {
                        Ok(RegionStore::new(region, Arc::new(mapping_client.clone())))
                    }
                })
                .with_invalidate_region_hook(move |_| {
                    invalidate_observer.store(true, Ordering::SeqCst);
                }),
        );

        let response = execute_fallback_get(pd_client, Backoff::no_jitter_backoff(1, 1, 1)).await;

        assert!(response.is_ok());
        assert!(region_invalidated.load(Ordering::SeqCst));
        assert_eq!(
            mapping_attempts.lock().unwrap().as_slice(),
            &[41, 44, 45],
            "after every stale peer fails to map, retry must load the replacement Region"
        );
    }

    #[tokio::test]
    async fn test_extract_error() {
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |_: &dyn Any| {
                Ok(Box::new(kvrpcpb::CommitResponse {
                    error: Some(kvrpcpb::KeyError::default()),
                    ..Default::default()
                }) as Box<dyn Any>)
            },
        )));

        let key: Key = "key".to_owned().into();
        let req = new_commit_request(iter::once(key), Timestamp::default(), Timestamp::default());

        // does not extract error
        let plan =
            crate::request::PlanBuilder::new(pd_client.clone(), Keyspace::Disable, req.clone())
                .retry_multi_region(OPTIMISTIC_BACKOFF)
                .plan();
        assert!(plan.execute().await.is_ok());

        // extract error
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), Keyspace::Disable, req)
            .retry_multi_region(OPTIMISTIC_BACKOFF)
            .extract_error()
            .plan();
        assert!(plan.execute().await.is_err());
    }

    #[tokio::test]
    async fn test_grpc_error_invalidates_store_cache() {
        #[derive(Debug, Clone)]
        struct MockOkResponse;

        impl HasKeyErrors for MockOkResponse {
            fn key_errors(&mut self) -> Option<Vec<Error>> {
                None
            }
        }

        impl HasRegionError for MockOkResponse {
            fn region_error(&mut self) -> Option<crate::proto::errorpb::Error> {
                None
            }
        }

        impl HasLocks for MockOkResponse {}

        struct InvalidationTrackingPdClient {
            client: MockKvClient,
            invalidate_region_count: AtomicUsize,
            invalidate_store_count: AtomicUsize,
        }

        impl InvalidationTrackingPdClient {
            fn region() -> RegionWithLeader {
                let mut region = RegionWithLeader::default();
                region.region.id = 1;
                region.region.start_key = vec![];
                region.region.end_key = vec![];
                region.region.region_epoch = Some(RegionEpoch {
                    conf_ver: 0,
                    version: 0,
                });
                region.leader = Some(metapb::Peer {
                    store_id: 41,
                    ..Default::default()
                });
                region
            }
        }

        #[async_trait]
        impl crate::pd::PdClient for InvalidationTrackingPdClient {
            type KvClient = MockKvClient;

            async fn map_region_to_store(
                self: Arc<Self>,
                region: RegionWithLeader,
            ) -> Result<RegionStore> {
                Ok(RegionStore::new(region, Arc::new(self.client.clone())))
            }

            async fn region_for_key(&self, _: &Key) -> Result<RegionWithLeader> {
                Ok(Self::region())
            }

            async fn region_for_id(&self, id: RegionId) -> Result<RegionWithLeader> {
                match id {
                    1 => Ok(Self::region()),
                    _ => Err(Error::RegionNotFoundInResponse { region_id: id }),
                }
            }

            async fn all_stores(&self) -> Result<Vec<Store>> {
                Ok(vec![Store::new(Arc::new(self.client.clone()))])
            }

            async fn get_timestamp(self: Arc<Self>) -> Result<Timestamp> {
                Ok(Timestamp::default())
            }

            async fn update_safepoint(self: Arc<Self>, _safepoint: u64) -> Result<bool> {
                unimplemented!()
            }

            async fn load_keyspace(&self, _keyspace: &str) -> Result<keyspacepb::KeyspaceMeta> {
                unimplemented!()
            }

            async fn update_leader(
                &self,
                _ver_id: RegionVerId,
                _leader: metapb::Peer,
            ) -> Result<()> {
                Ok(())
            }

            async fn invalidate_region_cache(&self, _ver_id: RegionVerId) {
                self.invalidate_region_count.fetch_add(1, Ordering::SeqCst);
            }

            async fn invalidate_store_cache(&self, _store_id: StoreId) {
                self.invalidate_store_count.fetch_add(1, Ordering::SeqCst);
            }
        }

        #[derive(Clone)]
        struct MockKvRequest;

        #[async_trait]
        impl Request for MockKvRequest {
            async fn dispatch(&self, _: &TikvClient<Channel>, _: Duration) -> Result<Box<dyn Any>> {
                Ok(Box::new(MockOkResponse))
            }

            fn label(&self) -> &'static str {
                "mock"
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn set_leader(&mut self, _: &RegionWithLeader) -> Result<()> {
                Ok(())
            }

            fn set_api_version(&mut self, _: kvrpcpb::ApiVersion) {}
        }

        #[async_trait]
        impl KvRequest for MockKvRequest {
            type Response = MockOkResponse;
        }

        impl Shardable for MockKvRequest {
            type Shard = Vec<Vec<u8>>;

            fn shards(
                &self,
                pd_client: &Arc<impl crate::pd::PdClient>,
            ) -> futures::stream::BoxStream<
                'static,
                crate::Result<(Self::Shard, crate::region::RegionWithLeader)>,
            > {
                region_stream_for_keys(
                    Some(Key::from("mock_key".to_owned())).into_iter(),
                    pd_client.clone(),
                )
            }

            fn apply_shard(&mut self, _shard: Self::Shard) {}

            fn apply_store(&mut self, _store: &crate::store::RegionStore) -> crate::Result<()> {
                Ok(())
            }
        }

        let fail_first_dispatch = Arc::new(AtomicBool::new(true));
        let pd_client = Arc::new(InvalidationTrackingPdClient {
            client: MockKvClient::with_dispatch_hook(move |_: &dyn Any| {
                if fail_first_dispatch.swap(false, Ordering::SeqCst) {
                    Err(Error::GrpcAPI(tonic::Status::unavailable(
                        "transient failure",
                    )))
                } else {
                    Ok(Box::new(MockOkResponse) as Box<dyn Any>)
                }
            }),
            invalidate_region_count: AtomicUsize::new(0),
            invalidate_store_count: AtomicUsize::new(0),
        });

        let plan =
            crate::request::PlanBuilder::new(pd_client.clone(), Keyspace::Disable, MockKvRequest)
                .retry_multi_region(Backoff::no_jitter_backoff(1, 1, 1))
                .plan();
        let response = plan.execute().await;
        assert!(response.is_ok());
        assert_eq!(pd_client.invalidate_region_count.load(Ordering::SeqCst), 1);
        assert_eq!(pd_client.invalidate_store_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_unreachable_leader_falls_back_to_voter_without_replica_read() {
        let mut region = RegionWithLeader::default();
        region.region.id = 1;
        region.region.region_epoch = Some(RegionEpoch {
            conf_ver: 1,
            version: 1,
        });
        let leader = metapb::Peer {
            id: 1,
            store_id: 41,
            ..Default::default()
        };
        let learner = metapb::Peer {
            id: 2,
            store_id: 42,
            role: metapb::PeerRole::Learner as i32,
            ..Default::default()
        };
        let witness = metapb::Peer {
            id: 3,
            store_id: 43,
            is_witness: true,
            ..Default::default()
        };
        let follower = metapb::Peer {
            id: 4,
            store_id: 44,
            ..Default::default()
        };
        region.region.peers = vec![leader.clone(), learner, witness, follower.clone()];
        region.leader = Some(leader);

        let accesses = Arc::new(std::sync::Mutex::new(Vec::new()));
        let dispatch_accesses = accesses.clone();
        let client = MockKvClient::with_dispatch_hook(move |request: &dyn Any| {
            let context = fallback_get_context(request);
            let store_id = context.peer.as_ref().expect("target peer").store_id;
            dispatch_accesses
                .lock()
                .unwrap()
                .push((store_id, context.replica_read));
            if store_id == 41 {
                Err(Error::GrpcAPI(tonic::Status::unavailable(
                    "leader unavailable",
                )))
            } else {
                Ok(Box::new(kvrpcpb::GetResponse {
                    region_error: Some(crate::proto::errorpb::Error {
                        not_leader: Some(crate::proto::errorpb::NotLeader {
                            region_id: 1,
                            leader: None,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }) as Box<dyn Any>)
            }
        });
        let located_region = region.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client).with_region_for_key_hook(move |_| Ok(located_region.clone())),
        );
        let response = execute_fallback_get(pd_client, Backoff::no_backoff()).await;

        assert!(response.is_err());
        assert_eq!(
            accesses.lock().unwrap().as_slice(),
            &[(41, false), (44, false)],
            "leader is tried first; learner and witness are skipped; fallback stays a normal request"
        );
    }

    fn fallback_region(follower_store_ids: &[StoreId]) -> RegionWithLeader {
        let mut region = RegionWithLeader::default();
        region.region.id = 1;
        region.region.region_epoch = Some(RegionEpoch {
            conf_ver: 1,
            version: 1,
        });
        let leader = metapb::Peer {
            id: 1,
            store_id: 41,
            ..Default::default()
        };
        region.region.peers.push(leader.clone());
        region
            .region
            .peers
            .extend(
                follower_store_ids
                    .iter()
                    .enumerate()
                    .map(|(index, store_id)| metapb::Peer {
                        id: index as u64 + 2,
                        store_id: *store_id,
                        ..Default::default()
                    }),
            );
        region.leader = Some(leader);
        region
    }

    fn fallback_get_context(request: &dyn Any) -> &kvrpcpb::Context {
        request
            .downcast_ref::<kvrpcpb::GetRequest>()
            .expect("get request")
            .context
            .as_ref()
            .expect("request context")
    }

    async fn execute_fallback_get<PdC: crate::pd::PdClient>(
        pd_client: Arc<PdC>,
        backoff: Backoff,
    ) -> Result<Vec<Result<kvrpcpb::GetResponse>>> {
        PlanBuilder::new(
            pd_client,
            Keyspace::Disable,
            kvrpcpb::GetRequest {
                key: b"key".to_vec(),
                ..Default::default()
            },
        )
        .retry_multi_region(backoff)
        .plan()
        .execute()
        .await
    }

    #[tokio::test]
    async fn test_not_leader_follower_does_not_hide_remaining_voters() {
        let accesses = Arc::new(std::sync::Mutex::new(Vec::new()));
        let dispatch_accesses = accesses.clone();
        let client = MockKvClient::with_dispatch_hook(move |request: &dyn Any| {
            let context = fallback_get_context(request);
            let store_id = context.peer.as_ref().expect("target peer").store_id;
            dispatch_accesses
                .lock()
                .unwrap()
                .push((store_id, context.replica_read));
            match store_id {
                41 => Err(Error::GrpcAPI(tonic::Status::unavailable(
                    "leader unavailable",
                ))),
                44 => Ok(Box::new(kvrpcpb::GetResponse {
                    region_error: Some(crate::proto::errorpb::Error {
                        not_leader: Some(crate::proto::errorpb::NotLeader {
                            region_id: 1,
                            leader: None,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }) as Box<dyn Any>),
                45 => Ok(Box::new(kvrpcpb::GetResponse::default()) as Box<dyn Any>),
                _ => unreachable!(),
            }
        });
        let region = fallback_region(&[44, 45]);
        let located_region = region.clone();
        let updated_leader_store_id = Arc::new(AtomicU64::new(0));
        let tracked_leader_store_id = updated_leader_store_id.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client)
                .with_region_for_key_hook(move |_| Ok(located_region.clone()))
                .with_update_leader_hook(move |_, leader| {
                    tracked_leader_store_id.store(leader.store_id, Ordering::SeqCst);
                    Ok(())
                }),
        );

        let response = execute_fallback_get(pd_client, Backoff::no_backoff()).await;

        assert!(response.is_ok());
        assert_eq!(
            accesses.lock().unwrap().as_slice(),
            &[(41, false), (44, false), (45, false)]
        );
        assert_eq!(updated_leader_store_id.load(Ordering::SeqCst), 45);
    }

    #[tokio::test]
    async fn test_follower_leader_hint_is_ignored() {
        let accesses = Arc::new(std::sync::Mutex::new(Vec::new()));
        let dispatch_accesses = accesses.clone();
        let client = MockKvClient::with_dispatch_hook(move |request: &dyn Any| {
            let store_id = fallback_get_context(request)
                .peer
                .as_ref()
                .expect("target peer")
                .store_id;
            dispatch_accesses.lock().unwrap().push(store_id);
            if store_id == 41 {
                return Err(Error::GrpcAPI(tonic::Status::unavailable(
                    "leader unavailable",
                )));
            }

            let leader = (store_id == 44).then_some(metapb::Peer {
                id: 3,
                store_id: 45,
                ..Default::default()
            });
            Ok(Box::new(kvrpcpb::GetResponse {
                region_error: Some(crate::proto::errorpb::Error {
                    not_leader: Some(crate::proto::errorpb::NotLeader {
                        region_id: 1,
                        // Store 44 points at store 45, but fallback deliberately
                        // ignores follower hints. Store 45 is tried naturally
                        // and independently reports that it is not the leader.
                        leader,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }) as Box<dyn Any>)
        });
        let region = fallback_region(&[44, 45]);
        let located_region = region.clone();
        let update_leader_count = Arc::new(AtomicUsize::new(0));
        let tracked_update_leader_count = update_leader_count.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client)
                .with_region_for_key_hook(move |_| Ok(located_region.clone()))
                .with_update_leader_hook(move |_, _| {
                    tracked_update_leader_count.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }),
        );

        let response = execute_fallback_get(pd_client, Backoff::no_jitter_backoff(1, 1, 1)).await;

        assert!(matches!(
            response,
            Err(Error::GrpcAPI(status)) if status.code() == tonic::Code::Unavailable
        ));
        assert_eq!(
            accesses.lock().unwrap().as_slice(),
            &[41, 44, 45, 41, 44, 45]
        );
        assert_eq!(
            update_leader_count.load(Ordering::SeqCst),
            0,
            "a fallback follower's leader hint must not update the cache"
        );
    }

    #[tokio::test]
    async fn test_invalidating_routing_error_outweighs_fallback_server_busy() {
        let accesses = Arc::new(std::sync::Mutex::new(Vec::new()));
        let dispatch_accesses = accesses.clone();
        let client = MockKvClient::with_dispatch_hook(move |request: &dyn Any| {
            let store_id = fallback_get_context(request)
                .peer
                .as_ref()
                .expect("target peer")
                .store_id;
            dispatch_accesses.lock().unwrap().push(store_id);
            if store_id == 41 {
                Err(Error::GrpcAPI(tonic::Status::unavailable(
                    "cached leader unavailable",
                )))
            } else {
                Ok(Box::new(kvrpcpb::GetResponse {
                    region_error: Some(crate::proto::errorpb::Error {
                        server_is_busy: Some(crate::proto::errorpb::ServerIsBusy::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }) as Box<dyn Any>)
            }
        });
        let region = fallback_region(&[44]);
        let located_region = region.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client).with_region_for_key_hook(move |_| Ok(located_region.clone())),
        );

        let response = execute_fallback_get(pd_client, Backoff::no_backoff()).await;

        assert!(matches!(
            response,
            Err(Error::GrpcAPI(status)) if status.code() == tonic::Code::Unavailable
        ));
        assert_eq!(accesses.lock().unwrap().as_slice(), &[41, 44]);
    }

    #[tokio::test]
    async fn test_fallback_not_leader_invalidates_stale_region() {
        let accesses = Arc::new(std::sync::Mutex::new(Vec::new()));
        let dispatch_accesses = accesses.clone();
        let client = MockKvClient::with_dispatch_hook(move |request: &dyn Any| {
            let store_id = fallback_get_context(request)
                .peer
                .as_ref()
                .expect("target peer")
                .store_id;
            dispatch_accesses.lock().unwrap().push(store_id);
            if store_id == 41 {
                Err(Error::GrpcAPI(tonic::Status::deadline_exceeded(
                    "cached leader timed out",
                )))
            } else {
                Ok(Box::new(kvrpcpb::GetResponse {
                    region_error: Some(crate::proto::errorpb::Error {
                        not_leader: Some(crate::proto::errorpb::NotLeader {
                            region_id: 1,
                            leader: None,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }) as Box<dyn Any>)
            }
        });
        let region = fallback_region(&[44]);
        let located_region = region.clone();
        let invalidate_count = Arc::new(AtomicUsize::new(0));
        let tracked_invalidate_count = invalidate_count.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client)
                .with_region_for_key_hook(move |_| Ok(located_region.clone()))
                .with_invalidate_region_hook(move |_| {
                    tracked_invalidate_count.fetch_add(1, Ordering::SeqCst);
                }),
        );

        let response = execute_fallback_get(pd_client, Backoff::no_backoff()).await;

        assert!(matches!(response, Err(Error::LeaderNotFound { .. })));
        assert_eq!(accesses.lock().unwrap().as_slice(), &[41, 44]);
        assert_eq!(invalidate_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_nonzero_server_busy_reloads_cached_leader() {
        let accesses = Arc::new(std::sync::Mutex::new(Vec::new()));
        let dispatch_accesses = accesses.clone();
        let leader_busy = Arc::new(AtomicBool::new(false));
        let dispatch_leader_busy = leader_busy.clone();
        let client = MockKvClient::with_dispatch_hook(move |request: &dyn Any| {
            let context = fallback_get_context(request);
            let store_id = context.peer.as_ref().expect("target peer").store_id;
            dispatch_accesses
                .lock()
                .unwrap()
                .push((store_id, context.replica_read));
            if store_id == 41 {
                dispatch_leader_busy.store(true, Ordering::SeqCst);
                Ok(Box::new(kvrpcpb::GetResponse {
                    region_error: Some(crate::proto::errorpb::Error {
                        server_is_busy: Some(crate::proto::errorpb::ServerIsBusy {
                            estimated_wait_ms: 500,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }) as Box<dyn Any>)
            } else {
                Ok(Box::new(kvrpcpb::GetResponse::default()) as Box<dyn Any>)
            }
        });
        let initial_region = fallback_region(&[44]);
        let mut updated_region = initial_region.clone();
        updated_region.leader = updated_region
            .region
            .peers
            .iter()
            .find(|peer| peer.store_id == 44)
            .cloned();
        let pd_client = Arc::new(
            MockPdClient::new(client).with_region_for_key_hook(move |_| {
                if leader_busy.load(Ordering::SeqCst) {
                    Ok(updated_region.clone())
                } else {
                    Ok(initial_region.clone())
                }
            }),
        );

        let response = execute_fallback_get(pd_client, Backoff::no_jitter_backoff(1, 1, 1)).await;

        assert!(response.is_ok());
        assert_eq!(
            accesses.lock().unwrap().as_slice(),
            &[(41, false), (44, false)],
            "a nonzero busy response must re-read the shared leader cache"
        );
    }

    #[tokio::test]
    async fn test_second_server_busy_zero_probes_followers_without_replica_read() {
        let accesses = Arc::new(std::sync::Mutex::new(Vec::new()));
        let dispatch_accesses = accesses.clone();
        let client = MockKvClient::with_dispatch_hook(move |request: &dyn Any| {
            let context = fallback_get_context(request);
            let store_id = context.peer.as_ref().expect("target peer").store_id;
            dispatch_accesses
                .lock()
                .unwrap()
                .push((store_id, context.replica_read));
            match store_id {
                41 => Ok(Box::new(kvrpcpb::GetResponse {
                    region_error: Some(crate::proto::errorpb::Error {
                        server_is_busy: Some(crate::proto::errorpb::ServerIsBusy::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }) as Box<dyn Any>),
                44 => Ok(Box::new(kvrpcpb::GetResponse {
                    region_error: Some(crate::proto::errorpb::Error {
                        not_leader: Some(crate::proto::errorpb::NotLeader {
                            region_id: 1,
                            leader: None,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }) as Box<dyn Any>),
                45 => Ok(Box::new(kvrpcpb::GetResponse::default()) as Box<dyn Any>),
                _ => unreachable!(),
            }
        });
        let region = fallback_region(&[44, 45]);
        let located_region = region.clone();
        let updated_leader_store_id = Arc::new(AtomicU64::new(0));
        let tracked_leader_store_id = updated_leader_store_id.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client)
                .with_region_for_key_hook(move |_| Ok(located_region.clone()))
                .with_update_leader_hook(move |_, leader| {
                    tracked_leader_store_id.store(leader.store_id, Ordering::SeqCst);
                    Ok(())
                }),
        );

        let response = execute_fallback_get(pd_client, Backoff::no_jitter_backoff(1, 1, 2)).await;

        assert!(response.is_ok());
        assert_eq!(
            accesses.lock().unwrap().as_slice(),
            &[(41, false), (41, false), (44, false), (45, false)],
            "the same leader is retried once, then followers are probed with leader-read semantics"
        );
        assert_eq!(updated_leader_store_id.load(Ordering::SeqCst), 45);
    }

    #[tokio::test]
    async fn test_server_busy_follower_probe_is_one_shot() {
        let accesses = Arc::new(std::sync::Mutex::new(Vec::new()));
        let dispatch_accesses = accesses.clone();
        let leader_attempts = Arc::new(AtomicUsize::new(0));
        let dispatch_leader_attempts = leader_attempts.clone();
        let client = MockKvClient::with_dispatch_hook(move |request: &dyn Any| {
            let context = fallback_get_context(request);
            let store_id = context.peer.as_ref().expect("target peer").store_id;
            dispatch_accesses
                .lock()
                .unwrap()
                .push((store_id, context.replica_read));
            if store_id == 41 {
                if dispatch_leader_attempts.fetch_add(1, Ordering::SeqCst) < 3 {
                    return Ok(Box::new(kvrpcpb::GetResponse {
                        region_error: Some(crate::proto::errorpb::Error {
                            server_is_busy: Some(crate::proto::errorpb::ServerIsBusy::default()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }) as Box<dyn Any>);
                }
                return Ok(Box::new(kvrpcpb::GetResponse::default()) as Box<dyn Any>);
            }

            Ok(Box::new(kvrpcpb::GetResponse {
                region_error: Some(crate::proto::errorpb::Error {
                    not_leader: Some(crate::proto::errorpb::NotLeader {
                        region_id: 1,
                        leader: None,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }) as Box<dyn Any>)
        });
        let region = fallback_region(&[44, 45]);
        let located_region = region.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client).with_region_for_key_hook(move |_| Ok(located_region.clone())),
        );

        let response = execute_fallback_get(pd_client, Backoff::no_jitter_backoff(1, 1, 3)).await;

        assert!(response.is_ok());
        assert_eq!(
            accesses.lock().unwrap().as_slice(),
            &[
                (41, false),
                (41, false),
                (44, false),
                (45, false),
                (41, false),
                (41, false),
            ],
            "a fruitless probe restores the cached leader and is not fired again"
        );
    }

    #[tokio::test]
    async fn test_fallback_key_error_updates_leader_cache() {
        let client = MockKvClient::with_dispatch_hook(move |request: &dyn Any| {
            let store_id = fallback_get_context(request)
                .peer
                .as_ref()
                .expect("target peer")
                .store_id;
            if store_id == 41 {
                Err(Error::GrpcAPI(tonic::Status::unavailable(
                    "leader unavailable",
                )))
            } else {
                Ok(Box::new(kvrpcpb::GetResponse {
                    error: Some(kvrpcpb::KeyError::default()),
                    ..Default::default()
                }) as Box<dyn Any>)
            }
        });
        let region = fallback_region(&[44]);
        let located_region = region.clone();
        let updated_leader_store_id = Arc::new(AtomicU64::new(0));
        let tracked_leader_store_id = updated_leader_store_id.clone();
        let pd_client = Arc::new(
            MockPdClient::new(client)
                .with_region_for_key_hook(move |_| Ok(located_region.clone()))
                .with_update_leader_hook(move |_, leader| {
                    tracked_leader_store_id.store(leader.store_id, Ordering::SeqCst);
                    Ok(())
                }),
        );

        let response = execute_fallback_get(pd_client, Backoff::no_backoff())
            .await
            .expect("key errors stay in the per-region result");

        assert!(matches!(
            response.as_slice(),
            [Err(Error::MultipleKeyErrors(_))]
        ));
        assert_eq!(updated_leader_store_id.load(Ordering::SeqCst), 44);
    }
}
