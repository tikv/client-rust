// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use super::plan::PreserveShard;
use crate::{
    pd::PdClient,
    request::{Dispatch, KvRequest, Plan, ResolveLock, RetryRegion},
    store::Store,
    Result,
};
use futures::stream::BoxStream;
use std::sync::Arc;

macro_rules! impl_inner_shardable {
    () => {
        type Shard = P::Shard;

        fn shards(
            &self,
            pd_client: &Arc<impl PdClient>,
        ) -> BoxStream<'static, Result<(Self::Shard, Store)>> {
            self.inner.shards(pd_client)
        }

        fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()> {
            self.inner.apply_shard(shard, store)
        }
    };
}

pub trait Shardable {
    type Shard: Clone + Send + Sync;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, Store)>>;

    fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()>;
}

impl<Req: KvRequest + Shardable> Shardable for Dispatch<Req> {
    type Shard = Req::Shard;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, Store)>> {
        self.request.shards(pd_client)
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()> {
        self.kv_client = Some(store.client.clone());
        self.request.apply_shard(shard, store)
    }
}

impl<P: Plan + Shardable> Shardable for PreserveShard<P> {
    type Shard = P::Shard;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, Store)>> {
        self.inner.shards(pd_client)
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()> {
        self.shard = Some(shard.clone());
        self.inner.apply_shard(shard, store)
    }
}

impl<P: Plan + Shardable, PdC: PdClient> Shardable for ResolveLock<P, PdC> {
    impl_inner_shardable!();
}

impl<P: Plan + Shardable, PdC: PdClient> Shardable for RetryRegion<P, PdC> {
    impl_inner_shardable!();
}

#[macro_export]
macro_rules! shardable_keys {
    ($type_: ty) => {
        impl Shardable for $type_ {
            type Shard = Vec<Vec<u8>>;

            fn shards(
                &self,
                pd_client: &std::sync::Arc<impl crate::pd::PdClient>,
            ) -> futures::stream::BoxStream<
                'static,
                crate::Result<(Self::Shard, crate::store::Store)>,
            > {
                let mut keys = self.keys.clone();
                keys.sort();
                crate::store::store_stream_for_keys(keys.into_iter(), pd_client.clone())
            }

            fn apply_shard(
                &mut self,
                shard: Self::Shard,
                store: &crate::store::Store,
            ) -> crate::Result<()> {
                self.set_context(store.region.context()?);
                self.set_keys(shard.into_iter().map(Into::into).collect());
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! shardable_range {
    ($type_: ty) => {
        impl Shardable for $type_ {
            type Shard = (Vec<u8>, Vec<u8>);

            fn shards(
                &self,
                pd_client: &Arc<impl crate::pd::PdClient>,
            ) -> BoxStream<'static, crate::Result<(Self::Shard, crate::store::Store)>> {
                let start_key = self.start_key.clone().into();
                let end_key = self.end_key.clone().into();
                crate::store::store_stream_for_range((start_key, end_key), pd_client.clone())
            }

            fn apply_shard(
                &mut self,
                shard: Self::Shard,
                store: &crate::store::Store,
            ) -> crate::Result<()> {
                self.set_context(store.region.context()?);

                self.set_start_key(shard.0.into());
                self.set_end_key(shard.1.into());
                Ok(())
            }
        }
    };
}
