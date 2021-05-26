// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::PdClient,
    request::{Dispatch, KvRequest, Plan, ResolveLock, RetryRegion},
    store::Store,
    Result,
};
use futures::stream::BoxStream;
use std::sync::Arc;

use super::plan::PreserveShard;

pub trait Shardable {
    type Shard: Send;

    fn shards(
        shard: Self::Shard,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, Store)>>;

    fn get_shard(&self) -> Self::Shard;

    fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()>;
}

impl<Req: KvRequest + Shardable> Shardable for Dispatch<Req> {
    type Shard = Req::Shard;

    fn shards(
        shard: Self::Shard,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, Store)>> {
        Req::shards(shard, pd_client)
    }

    fn get_shard(&self) -> Self::Shard {
        self.request.get_shard()
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()> {
        self.kv_client = Some(store.client.clone());
        self.request.apply_shard(shard, store)
    }
}

impl<P: Plan + Shardable, PdC: PdClient> Shardable for ResolveLock<P, PdC> {
    impl_inner_shardable!();
}

impl<P: Plan + Shardable> Shardable for PreserveShard<P> {
    impl_inner_shardable!();
}

impl<P: Plan + Shardable, PdC: PdClient> Shardable for RetryRegion<P, PdC> {
    type Shard = P::Shard;

    fn shards(
        shard: Self::Shard,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, Store)>> {
        P::shards(shard, pd_client)
    }

    fn get_shard(&self) -> Self::Shard {
        self.inner.get_shard()
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &Store) -> Result<()> {
        self.shard = Some(shard.clone());
        self.inner.apply_shard(shard, store)
    }
}

#[macro_export]
macro_rules! shardable_keys {
    ($type_: ty) => {
        impl Shardable for $type_ {
            type Shard = Vec<Vec<u8>>;

            fn shards(
                mut keys: Self::Shard,
                pd_client: &std::sync::Arc<impl crate::pd::PdClient>,
            ) -> futures::stream::BoxStream<
                'static,
                crate::Result<(Self::Shard, crate::store::Store)>,
            > {
                keys.sort();
                crate::store::store_stream_for_keys(keys.into_iter(), pd_client.clone())
            }

            fn get_shard(&self) -> Self::Shard {
                self.keys.clone()
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
                range: Self::Shard,
                pd_client: &Arc<impl crate::pd::PdClient>,
            ) -> BoxStream<'static, crate::Result<(Self::Shard, crate::store::Store)>> {
                crate::store::store_stream_for_range(range, pd_client.clone())
            }

            fn get_shard(&self) -> Self::Shard {
                (self.start_key.clone().into(), self.end_key.clone().into())
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
