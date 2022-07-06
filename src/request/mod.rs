// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use async_trait::async_trait;
use derive_new::new;

use tikv_client_store::{HasKeyErrors, HasRegionError, Request};

use crate::{
    backoff::{Backoff, DEFAULT_REGION_BACKOFF, OPTIMISTIC_BACKOFF, PESSIMISTIC_BACKOFF},
    transaction::HasLocks,
};

pub use self::{
    plan::{
        Collect, CollectError, CollectSingle, CollectWithShard, DefaultProcessor, Dispatch,
        ExtractError, Merge, MergeResponse, Plan, Process, ProcessResponse, ResolveLock,
        ResponseWithShard, RetryableMultiRegion,
    },
    plan_builder::{PlanBuilder, SingleKey},
    shard::Shardable,
};

pub mod plan;
mod plan_builder;
#[macro_use]
mod shard;
pub mod codec;

/// Abstracts any request sent to a TiKV server.
#[async_trait]
pub trait KvRequest<C>: Request + Sized + Clone + Sync + Send + 'static {
    /// The expected response to the request.
    type Response: HasKeyErrors + HasLocks + HasRegionError + Clone + Send + 'static;

    fn encode_request(self, _codec: &C) -> Self {
        self
    }

    fn decode_response(&self, _codec: &C, resp: Self::Response) -> crate::Result<Self::Response> {
        Ok(resp)
    }
}

macro_rules! impl_decode_response {
    ($($o:ident)*; $($e:ident)*) => {
        fn decode_response(&self, codec: &C, mut resp: Self::Response) -> Result<Self::Response> {
            $(
                paste::paste! {
                    codec.[<decode_ $o>](resp.[<mut_ $o>]())?;
                }
            )*

            // decode errors
            if resp.has_region_error() {
                codec.decode_region_error(resp.mut_region_error())?;
            }

            $(
                paste::paste! {
                    if resp.[<has_ $e>]() {
                        codec.[<decode_ $e>](resp.[<mut_ $e>]())?;
                    }
                }
            )*

            Ok(resp)
        }
    };
}

#[macro_export]
macro_rules! impl_kv_request {
    ($req:ty $(,$i:ident)+; $resp:ty $(,$o:ident)*; $($e:ident),*) => {
        impl<C> KvRequest<C> for $req
        where C: RequestCodec
        {
            type Response = $resp;

            fn encode_request(mut self, codec: &C) -> Self {
                $(
                    paste::paste! {
                        *self.[<mut_ $i>]() = codec.[<encode_ $i>](self.[<take_ $i>]());
                    }
                )*

                self
            }

            impl_decode_response!{$($o)*; $($e)*}
        }
    };

    ($req:ty; $resp:ty $(,$o:ident)*;$($e:ident)*) => {
        impl<C> KvRequest<C> for $req
        where C: RequestCodec
        {
            type Response = $resp;

            fn encode_request(mut self, codec: &C) -> Self {
                let (start, end) = codec.encode_range(self.take_start_key(), self.take_end_key());
                *self.mut_start_key() = start;
                *self.mut_end_key() = end;

                self
            }

            impl_decode_response!{$($o)*; $($e)*}
        }
    };
}

#[macro_export]
macro_rules! impl_kv_request_for_single_key_op {
    ($req: ty, $resp: ty) => {
        impl<C> KvRequest<C> for $req
        where
            C: RequestCodec,
        {
            type Response = $resp;

            fn encode_request(mut self, codec: &C) -> Self {
                *self.mut_key() = codec.encode_key(self.take_key());

                self
            }
        }
    };
}

#[macro_export]
macro_rules! impl_kv_request_for_batch_get {
    ($req: ty, $resp: ty) => {
        impl<C> KvRequest<C> for $req
        where
            C: RequestCodec,
        {
            type Response = $resp;

            fn encode_request(mut self, codec: &C) -> Self {
                *self.mut_keys() = codec.encode_keys(self.take_keys());

                self
            }

            fn decode_response(
                &self,
                codec: &C,
                mut resp: Self::Response,
            ) -> $crate::Result<Self::Response> {
                codec.decode_pairs(resp.mut_pairs())?;

                Ok(resp)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_kv_request_for_scan_op {
    ($req: ty, $resp: ty, $pairs: ident) => {
        impl<C> KvRequest<C> for $req
        where
            C: RequestCodec,
        {
            type Response = $resp;

            fn encode_request(mut self, codec: &C) -> Self {
                let (start, end) =
                    codec.encode_range(self.take_start_key().into(), self.take_end_key().into());

                self.set_start_key(start);
                self.set_end_key(end);

                self
            }

            fn decode_response(
                &self,
                codec: &C,
                mut resp: Self::Response,
            ) -> $crate::Result<Self::Response> {
                paste::paste! {
                    let pairs = resp.[<mut_ $pairs>]();

                    codec.decode_pairs(pairs)?;

                    Ok(resp)
                }
            }
        }
    };
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
    use std::{
        any::Any,
        iter,
        sync::{atomic::AtomicUsize, Arc},
    };

    use grpcio::CallOption;

    use tikv_client_proto::{kvrpcpb, pdpb::Timestamp, tikvpb::TikvClient};
    use tikv_client_store::HasRegionError;

    use crate::{
        mock::{MockKvClient, MockPdClient},
        request::{codec::RequestCodec, KvRequest},
        store::store_stream_for_keys,
        transaction::lowering::new_commit_request,
        Error, Key, Result,
    };

    use super::*;

    #[tokio::test]
    async fn test_region_retry() {
        #[derive(Clone)]
        struct MockRpcResponse;

        impl HasKeyErrors for MockRpcResponse {
            fn key_errors(&mut self) -> Option<Vec<Error>> {
                None
            }
        }

        impl HasRegionError for MockRpcResponse {
            fn region_error(&mut self) -> Option<tikv_client_proto::errorpb::Error> {
                Some(tikv_client_proto::errorpb::Error::default())
            }
        }

        impl HasLocks for MockRpcResponse {}

        #[derive(Clone)]
        struct MockKvRequest {
            test_invoking_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl Request for MockKvRequest {
            async fn dispatch(&self, _: &TikvClient, _: CallOption) -> Result<Box<dyn Any>> {
                Ok(Box::new(MockRpcResponse {}))
            }

            fn label(&self) -> &'static str {
                "mock"
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn set_context(&mut self, _: kvrpcpb::Context) {
                unreachable!();
            }
        }

        #[async_trait]
        impl<C: RequestCodec> KvRequest<C> for MockKvRequest {
            type Response = MockRpcResponse;

            fn encode_request(self, _codec: &C) -> Self {
                self
            }

            fn decode_response(&self, _codec: &C, resp: Self::Response) -> Result<Self::Response> {
                Ok(resp)
            }
        }

        impl Shardable for MockKvRequest {
            type Shard = Vec<Vec<u8>>;

            fn shards(
                &self,
                pd_client: &std::sync::Arc<impl crate::pd::PdClient>,
            ) -> futures::stream::BoxStream<
                'static,
                crate::Result<(Self::Shard, crate::store::RegionStore)>,
            > {
                // Increases by 1 for each call.
                self.test_invoking_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                store_stream_for_keys(
                    Some(Key::from("mock_key".to_owned())).into_iter(),
                    pd_client.clone(),
                )
            }

            fn apply_shard(
                &mut self,
                _shard: Self::Shard,
                _store: &crate::store::RegionStore,
            ) -> crate::Result<()> {
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

        let plan = crate::request::PlanBuilder::new(pd_client.clone(), request)
            .resolve_lock(Backoff::no_jitter_backoff(1, 1, 3))
            .retry_multi_region(Backoff::no_jitter_backoff(1, 1, 3))
            .extract_error()
            .plan();
        let _ = plan.execute().await;

        // Original call plus the 3 retries
        assert_eq!(invoking_count.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_extract_error() {
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |_: &dyn Any| {
                Ok(Box::new(kvrpcpb::CommitResponse {
                    region_error: None,
                    error: Some(kvrpcpb::KeyError::default()),
                    commit_version: 0,
                }) as Box<dyn Any>)
            },
        )));

        let key: Key = "key".to_owned().into();
        let req = new_commit_request(iter::once(key), Timestamp::default(), Timestamp::default());

        // does not extract error
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), req.clone())
            .resolve_lock(OPTIMISTIC_BACKOFF)
            .retry_multi_region(OPTIMISTIC_BACKOFF)
            .plan();
        assert!(plan.execute().await.is_ok());

        // extract error
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), req)
            .resolve_lock(OPTIMISTIC_BACKOFF)
            .retry_multi_region(OPTIMISTIC_BACKOFF)
            .extract_error()
            .plan();
        assert!(plan.execute().await.is_err());
    }
}
