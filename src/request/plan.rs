// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    backoff::Backoff,
    pd::PdClient,
    request::{KvRequest, Shardable},
    stats::tikv_stats,
    transaction::{resolve_locks, HasLocks},
    Error, Result,
};
use async_trait::async_trait;
use futures::{prelude::*, stream::StreamExt};
use std::{marker::PhantomData, sync::Arc};
use tikv_client_store::{HasError, HasRegionError, KvClient};

/// A plan for how to execute a request. A user builds up a plan with various
/// options, then exectutes it.
#[async_trait]
pub trait Plan: Sized + Clone + Sync + Send + 'static {
    /// The ultimate result of executing the plan (should be a high-level type, not a GRPC response).
    type Result: Send;

    /// Execute the plan.
    async fn execute(&self) -> Result<Self::Result>;
}

/// The simplest plan which just dispatches a request to a specific kv server.
#[derive(Clone)]
pub struct Dispatch<Req: KvRequest> {
    pub request: Req,
    pub kv_client: Option<Arc<dyn KvClient + Send + Sync>>,
}

#[async_trait]
impl<Req: KvRequest> Plan for Dispatch<Req> {
    type Result = Req::Response;

    async fn execute(&self) -> Result<Self::Result> {
        let stats = tikv_stats(self.request.label());
        let result = self
            .kv_client
            .as_ref()
            .expect("Unreachable: kv_client has not been initialised in Dispatch")
            .dispatch(&self.request)
            .await;
        let result = stats.done(result);
        result.map(|r| *r.downcast().expect("Downcast failed"))
    }
}

pub struct MultiRegion<P: Plan, PdC: PdClient> {
    pub(super) inner: P,
    pub pd_client: Arc<PdC>,
}

impl<P: Plan, PdC: PdClient> Clone for MultiRegion<P, PdC> {
    fn clone(&self) -> Self {
        MultiRegion {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan + Shardable, PdC: PdClient> Plan for MultiRegion<P, PdC>
where
    P::Result: HasError,
{
    type Result = Vec<Result<P::Result>>;

    async fn execute(&self) -> Result<Self::Result> {
        Ok(self
            .inner
            .shards(&self.pd_client)
            .and_then(move |(shard, store)| async move {
                let mut clone = self.inner.clone();
                clone.apply_shard(shard, &store)?;
                let mut response = clone.execute().await?;
                match response.error() {
                    Some(e) => Err(e),
                    None => Ok(response),
                }
            })
            .collect()
            .await)
    }
}

/// A technique for merging responses into a single result (with type `Out`).
pub trait Merge<In>: Sized + Clone + Send + Sync + 'static {
    type Out: Send;

    fn merge(&self, input: Vec<Result<In>>) -> Result<Self::Out>;
}

#[derive(Clone)]
pub struct MergeResponse<P: Plan, In, M: Merge<In>> {
    pub inner: P,
    pub merge: M,
    pub phantom: PhantomData<In>,
}

#[async_trait]
impl<In: Clone + Send + Sync + 'static, P: Plan<Result = Vec<Result<In>>>, M: Merge<In>> Plan
    for MergeResponse<P, In, M>
{
    type Result = M::Out;

    async fn execute(&self) -> Result<Self::Result> {
        self.merge.merge(self.inner.execute().await?)
    }
}

/// A merge strategy which collects data from a response into a single type.
#[derive(Clone, Copy)]
pub struct Collect;

/// A merge strategy which returns an error if any response is an error and
/// otherwise returns a Vec of the results.
#[derive(Clone, Copy)]
pub struct CollectError;

impl<T: Send> Merge<T> for CollectError {
    type Out = Vec<T>;

    fn merge(&self, input: Vec<Result<T>>) -> Result<Self::Out> {
        input.into_iter().collect()
    }
}

/// Process data into another kind of data.
pub trait Process<In>: Sized + Clone + Send + Sync + 'static {
    type Out: Send;

    fn process(&self, input: Result<In>) -> Result<Self::Out>;
}

#[derive(Clone)]
pub struct ProcessResponse<P: Plan, In, Pr: Process<In>> {
    pub inner: P,
    pub processor: Pr,
    pub phantom: PhantomData<In>,
}

#[async_trait]
impl<In: Clone + Sync + Send + 'static, P: Plan<Result = In>, Pr: Process<In>> Plan
    for ProcessResponse<P, In, Pr>
{
    type Result = Pr::Out;

    async fn execute(&self) -> Result<Self::Result> {
        self.processor.process(self.inner.execute().await)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DefaultProcessor;

pub struct RetryRegion<P: Plan, PdC: PdClient> {
    pub inner: P,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,
}

impl<P: Plan, PdC: PdClient> Clone for RetryRegion<P, PdC> {
    fn clone(&self) -> Self {
        RetryRegion {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan, PdC: PdClient> Plan for RetryRegion<P, PdC>
where
    P::Result: HasError,
{
    type Result = P::Result;

    async fn execute(&self) -> Result<Self::Result> {
        let mut result = self.inner.execute().await?;
        let mut clone = self.clone();
        while let Some(region_error) = result.region_error() {
            match clone.backoff.next_delay_duration() {
                None => return Err(region_error),
                Some(delay_duration) => {
                    futures_timer::Delay::new(delay_duration).await;
                    result = clone.inner.execute().await?;
                }
            }
        }

        Ok(result)
    }
}

pub struct ResolveLock<P: Plan, PdC: PdClient> {
    pub inner: P,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,
}

impl<P: Plan, PdC: PdClient> Clone for ResolveLock<P, PdC> {
    fn clone(&self) -> Self {
        ResolveLock {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan, PdC: PdClient> Plan for ResolveLock<P, PdC>
where
    P::Result: HasLocks,
{
    type Result = P::Result;

    async fn execute(&self) -> Result<Self::Result> {
        let mut result = self.inner.execute().await?;
        let mut clone = self.clone();
        loop {
            let locks = result.take_locks();
            if locks.is_empty() {
                return Ok(result);
            }

            if self.backoff.is_none() {
                return Err(Error::ResolveLockError);
            }

            let pd_client = self.pd_client.clone();
            if resolve_locks(locks, pd_client.clone()).await? {
                result = self.inner.execute().await?;
            } else {
                match clone.backoff.next_delay_duration() {
                    None => return Err(Error::ResolveLockError),
                    Some(delay_duration) => {
                        futures_timer::Delay::new(delay_duration).await;
                        result = clone.inner.execute().await?;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::mock::{mock_store, MockPdClient};
    use futures::stream::BoxStream;
    use tikv_client_proto::kvrpcpb::BatchGetResponse;

    #[derive(Clone)]
    struct ErrPlan;

    #[async_trait]
    impl Plan for ErrPlan {
        type Result = BatchGetResponse;

        async fn execute(&self) -> Result<Self::Result> {
            Err(Error::Unimplemented)
        }
    }

    impl Shardable for ErrPlan {
        type Shard = u8;

        fn shards(
            &self,
            _: &Arc<impl crate::pd::PdClient>,
        ) -> BoxStream<'static, crate::Result<(Self::Shard, crate::store::Store)>> {
            Box::pin(stream::iter(1..=3).map(|_| Err(Error::Unimplemented)))
                .map_ok(|_: u8| (42, mock_store()))
                .boxed()
        }

        fn apply_shard(&mut self, _: Self::Shard, _: &crate::store::Store) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_err() {
        let plan = RetryRegion {
            inner: MultiRegion {
                inner: ResolveLock {
                    inner: ErrPlan,
                    backoff: Backoff::no_backoff(),
                    pd_client: Arc::new(MockPdClient::default()),
                },
                pd_client: Arc::new(MockPdClient::default()),
            },
            backoff: Backoff::no_backoff(),
            pd_client: Arc::new(MockPdClient::default()),
        };
        plan.execute()
            .await
            .unwrap()
            .iter()
            .for_each(|r| assert!(r.is_err()));
    }
}
