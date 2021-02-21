// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{backoff::Backoff, pd::PdClient, request::{KvRequest, Shardable}, stats::tikv_stats, transaction::{resolve_locks, HasLocks, TransactionStatus}, Error, Result};
use async_trait::async_trait;
use futures::{prelude::*, stream::StreamExt};
use std::{marker::PhantomData, sync::Arc};
use tikv_client_store::{HasError, HasRegionError, KvClient};
use std::sync::RwLock;

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

pub struct MultiRegionPlan<P: Plan, PdC: PdClient> {
    pub(super) inner: P,
    pub pd_client: Arc<PdC>,
}

impl<P: Plan, PdC: PdClient> Clone for MultiRegionPlan<P, PdC> {
    fn clone(&self) -> Self {
        MultiRegionPlan {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan<Result: HasError> + Shardable, PdC: PdClient> Plan for MultiRegionPlan<P, PdC> {
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
        let result = self.inner.execute().await?;
        self.merge.merge(result)
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
pub trait Process: Sized + Clone + Send + Sync + 'static {
    type Out: Send;

    fn process(input: Result<Self>) -> Result<Self::Out>;
}

#[derive(Clone)]
pub struct ProcessResponse<P: Plan, Pr: Process> {
    pub inner: P,
    pub phantom: PhantomData<Pr>,
}

#[async_trait]
impl<P: Plan<Result = Pr>, Pr: Process> Plan for ProcessResponse<P, Pr> {
    type Result = Pr::Out;

    async fn execute(&self) -> Result<Self::Result> {
        Pr::process(self.inner.execute().await)
    }
}

pub struct RetryRegionPlan<P: Plan, PdC: PdClient> {
    pub inner: P,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,
}

impl<P: Plan, PdC: PdClient> Clone for RetryRegionPlan<P, PdC> {
    fn clone(&self) -> Self {
        RetryRegionPlan {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan<Result: HasError>, PdC: PdClient> Plan for RetryRegionPlan<P, PdC> {
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

pub struct ResolveLockPlan<P: Plan, PdC: PdClient> {
    pub inner: P,
    pub pd_client: Arc<PdC>,
    pub backoff: Backoff,
}

impl<P: Plan, PdC: PdClient> Clone for ResolveLockPlan<P, PdC> {
    fn clone(&self) -> Self {
        ResolveLockPlan {
            inner: self.inner.clone(),
            pd_client: self.pd_client.clone(),
            backoff: self.backoff.clone(),
        }
    }
}

#[async_trait]
impl<P: Plan<Result: HasLocks>, PdC: PdClient> Plan for ResolveLockPlan<P, PdC> {
    type Result = P::Result;

    async fn execute(&self) -> Result<Self::Result> {
        let mut result = self.inner.execute().await?;
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
                let mut clone = self.clone();
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

#[derive(Clone)]
pub struct HeartbeatPlan<P: Plan> {
    pub inner: P,
    pub status: Arc<RwLock<TransactionStatus>>,
}

#[async_trait]
impl<P: Plan> Plan for HeartbeatPlan<P> {
    type Result = P::Result;

    async fn execute(&self) -> Result<Self::Result> {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(5000));
            let status = self.status.read().unwrap();
            let result = self.inner.execute().await?;
            if *status != TransactionStatus::Active {
                result;
            }
        }
    }
}
