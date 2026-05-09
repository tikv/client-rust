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
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    async fn test_region_store_mapping_retry() {
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
            invalidated: AtomicBool,
            invalidation_count: AtomicUsize,
        }

        impl FlakyStoreMappingPdClient {
            fn region(store_id: StoreId) -> RegionWithLeader {
                let mut region = RegionWithLeader::default();
                region.region.id = 1;
                region.region.start_key = vec![];
                region.region.end_key = vec![];
                region.region.region_epoch = Some(RegionEpoch {
                    conf_ver: 0,
                    version: 0,
                });
                region.leader = Some(metapb::Peer {
                    store_id,
                    ..Default::default()
                });
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
                match region.get_store_id()? {
                    41 => Err(Error::InternalError {
                        message: "invalid store ID 41, not found".to_owned(),
                    }),
                    _ => Ok(RegionStore::new(region, Arc::new(self.client.clone()))),
                }
            }

            async fn region_for_key(&self, _: &Key) -> Result<RegionWithLeader> {
                let store_id = if self.invalidated.load(Ordering::SeqCst) {
                    42
                } else {
                    41
                };
                Ok(Self::region(store_id))
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
                self.invalidated.store(true, Ordering::SeqCst);
                self.invalidation_count.fetch_add(1, Ordering::SeqCst);
            }

            async fn invalidate_store_cache(&self, _store_id: StoreId) {}
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
                dispatch_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(MockOkResponse) as Box<dyn Any>)
            }),
            invalidated: AtomicBool::new(false),
            invalidation_count: AtomicUsize::new(0),
        });

        let request = MockKvRequest {
            shard_invoking_count: shard_invoking_count.clone(),
        };

        let plan = crate::request::PlanBuilder::new(pd_client.clone(), Keyspace::Disable, request)
            .retry_multi_region(Backoff::no_jitter_backoff(1, 1, 3))
            .plan();

        let response = plan.execute().await;
        assert!(response.is_ok());
        assert_eq!(dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(shard_invoking_count.load(Ordering::SeqCst), 2);
        assert_eq!(pd_client.invalidation_count.load(Ordering::SeqCst), 1);
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
}
