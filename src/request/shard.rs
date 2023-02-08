// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use super::plan::PreserveShard;
use crate::{
    pd::PdClient,
    request::{plan::CleanupLocks, Dispatch, KvRequest, Plan, ResolveLock},
    store::RegionStore,
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
        ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
            self.inner.shards(pd_client)
        }

        fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
            self.inner.apply_shard(shard, store)
        }
    };
}

pub trait Shardable {
    type Shard: Clone + Send + Sync;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>>;

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()>;
}

pub trait Batchable {
    type Item;

    fn batches(items: Vec<Self::Item>, batch_size: u64) -> Vec<Vec<Self::Item>> {
        let mut batches: Vec<Vec<Self::Item>> = Vec::new();
        let mut batch: Vec<Self::Item> = Vec::new();
        let mut size = 0;

        for item in items {
            let item_size = Self::item_size(&item);
            if size + item_size >= batch_size && !batch.is_empty() {
                batches.push(batch);
                batch = Vec::new();
                size = 0;
            }
            size += item_size;
            batch.push(item);
        }
        if !batch.is_empty() {
            batches.push(batch)
        }
        batches
    }

    fn item_size(item: &Self::Item) -> u64;
}

// Use to iterate in a region for scan requests that have batch size limit.
// HasNextBatch use to get the next batch according to previous response.
pub trait HasNextBatch {
    fn has_next_batch(&self) -> Option<(Vec<u8>, Vec<u8>)>;
}

// NextBatch use to change start key of request by result of `has_next_batch`.
pub trait NextBatch {
    fn next_batch(&mut self, _range: (Vec<u8>, Vec<u8>));
}

impl<Req: KvRequest + Shardable> Shardable for Dispatch<Req> {
    type Shard = Req::Shard;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        self.request.shards(pd_client)
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.kv_client = Some(store.client.clone());
        self.request.apply_shard(shard, store)
    }
}

impl<Req: KvRequest + NextBatch> NextBatch for Dispatch<Req> {
    fn next_batch(&mut self, range: (Vec<u8>, Vec<u8>)) {
        self.request.next_batch(range);
    }
}

impl<P: Plan + Shardable> Shardable for PreserveShard<P> {
    type Shard = P::Shard;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        self.inner.shards(pd_client)
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.shard = Some(shard.clone());
        self.inner.apply_shard(shard, store)
    }
}

impl<P: Plan + Shardable, PdC: PdClient> Shardable for ResolveLock<P, PdC> {
    impl_inner_shardable!();
}

impl<P: Plan + Shardable, PdC: PdClient> Shardable for CleanupLocks<P, PdC> {
    type Shard = P::Shard;

    fn shards(
        &self,
        pd_client: &Arc<impl PdClient>,
    ) -> BoxStream<'static, Result<(Self::Shard, RegionStore)>> {
        self.inner.shards(pd_client)
    }

    fn apply_shard(&mut self, shard: Self::Shard, store: &RegionStore) -> Result<()> {
        self.store = Some(store.clone());
        self.inner.apply_shard(shard, store)
    }
}

#[macro_export]
macro_rules! shardable_key {
    ($type_: ty) => {
        impl Shardable for $type_ {
            type Shard = Vec<Vec<u8>>;

            fn shards(
                &self,
                pd_client: &std::sync::Arc<impl $crate::pd::PdClient>,
            ) -> futures::stream::BoxStream<
                'static,
                $crate::Result<(Self::Shard, $crate::store::RegionStore)>,
            > {
                $crate::store::store_stream_for_keys(
                    std::iter::once(self.key.clone()),
                    pd_client.clone(),
                )
            }

            fn apply_shard(
                &mut self,
                mut shard: Self::Shard,
                store: &$crate::store::RegionStore,
            ) -> $crate::Result<()> {
                self.set_context(store.region_with_leader.context()?);
                assert!(shard.len() == 1);
                self.set_key(shard.pop().unwrap());
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! shardable_keys {
    ($type_: ty) => {
        impl Shardable for $type_ {
            type Shard = Vec<Vec<u8>>;

            fn shards(
                &self,
                pd_client: &std::sync::Arc<impl $crate::pd::PdClient>,
            ) -> futures::stream::BoxStream<
                'static,
                $crate::Result<(Self::Shard, $crate::store::RegionStore)>,
            > {
                let mut keys = self.keys.clone();
                keys.sort();
                $crate::store::store_stream_for_keys(keys.into_iter(), pd_client.clone())
            }

            fn apply_shard(
                &mut self,
                shard: Self::Shard,
                store: &$crate::store::RegionStore,
            ) -> $crate::Result<()> {
                self.set_context(store.region_with_leader.context()?);
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
                pd_client: &Arc<impl $crate::pd::PdClient>,
            ) -> BoxStream<'static, $crate::Result<(Self::Shard, $crate::store::RegionStore)>> {
                let start_key = self.start_key.clone().into();
                let end_key = self.end_key.clone().into();
                $crate::store::store_stream_for_range((start_key, end_key), pd_client.clone())
            }

            fn apply_shard(
                &mut self,
                shard: Self::Shard,
                store: &$crate::store::RegionStore,
            ) -> $crate::Result<()> {
                self.set_context(store.region_with_leader.context()?);

                self.set_start_key(shard.0.into());
                self.set_end_key(shard.1.into());
                Ok(())
            }
        }
    };
}

#[cfg(test)]
mod test {
    use rand::{thread_rng, Rng};

    use super::Batchable;

    #[test]
    fn test_batches() {
        let mut rng = thread_rng();

        let items: Vec<_> = (0..3)
            .map(|_| (0..2).map(|_| rng.gen::<u8>()).collect::<Vec<_>>())
            .collect();

        let batch_size = 5;

        let batches = BatchableTest::batches(items.clone(), batch_size);

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 2);
        assert_eq!(batches[1].len(), 1);
        assert_eq!(batches[0][0], items[0]);
        assert_eq!(batches[0][1], items[1]);
        assert_eq!(batches[1][0], items[2]);
    }

    #[test]
    fn test_batches_big_item() {
        let mut rng = thread_rng();

        let items: Vec<_> = (0..3)
            .map(|_| (0..3).map(|_| rng.gen::<u8>()).collect::<Vec<_>>())
            .collect();

        let batch_size = 2;

        let batches = BatchableTest::batches(items.clone(), batch_size);

        assert_eq!(batches.len(), 3);
        for i in 0..items.len() {
            let batch = &batches[i];
            assert_eq!(batch.len(), 1);
            assert_eq!(batch[0], items[i]);
        }
    }

    struct BatchableTest;

    impl Batchable for BatchableTest {
        type Item = Vec<u8>;

        fn item_size(item: &Self::Item) -> u64 {
            item.len() as u64
        }
    }
}
