// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::RetryClient,
    region::{Region, RegionId, RegionVerId, StoreId},
    Key, Result,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tikv_client_pd::Cluster;
use tikv_client_proto::metapb::Store;
use tokio::sync::RwLock;

pub struct RegionCache<Cl = Cluster> {
    /// Start key -> Region
    ///
    /// Invariant: there are no intersecting regions in the map at any time.
    cache_by_key: RwLock<BTreeMap<Key, Region>>,
    /// Region ID -> Region. Note: regions with identical ID doesn't necessarily
    /// mean they are the same, they can be different regions across time. For
    /// example, when a region is splitted one of the new regions may have
    /// the same ID but a different RegionVerId.
    cache_by_id: RwLock<HashMap<RegionId, Region>>,
    store_cache: RwLock<HashMap<StoreId, Store>>,
    inner_client: Arc<RetryClient<Cl>>,
}

impl<Cl> RegionCache<Cl> {
    pub fn new(inner_client: Arc<RetryClient<Cl>>) -> RegionCache<Cl> {
        RegionCache {
            cache_by_key: RwLock::new(BTreeMap::new()),
            cache_by_id: RwLock::new(HashMap::new()),
            store_cache: RwLock::new(HashMap::new()),
            inner_client,
        }
    }
}

impl RegionCache<Cluster> {
    // Retrieve cache entry by key. If there's no entry, query PD and update cache.
    pub async fn get_region_by_key(&self, key: &Key) -> Result<Region> {
        let res = {
            let read_guard = self.cache_by_key.read().await;
            read_guard
                .range(..=key)
                .next_back()
                .map(|(x, y)| (x.clone(), y.clone()))
        };

        match res {
            Some((_, candidate_region)) if candidate_region.contains(key) => Ok(candidate_region),
            _ => self.read_through_region_by_key(key.clone()).await,
        }
    }

    // Retrieve cache entry by RegionId. If there's no entry, query PD and update cache.
    pub async fn get_region_by_id(&self, id: RegionId) -> Result<Region> {
        match self.cache_by_id.read().await.get(&id) {
            Some(region) => Ok(region.clone()),
            None => self.read_through_region_by_id(id).await,
        }
    }

    pub async fn get_store_by_id(&self, id: StoreId) -> Result<Store> {
        let store = self.store_cache.read().await.get(&id).cloned();
        match store {
            Some(store) => Ok(store),
            None => self.read_through_store_by_id(id).await,
        }
    }

    /// Force read through (query from PD) and update cache
    pub async fn read_through_region_by_key(&self, key: Key) -> Result<Region> {
        let region = self.inner_client.clone().get_region(key.into()).await?;
        self.add_region(region.clone()).await;
        Ok(region)
    }

    /// Force read through (query from PD) and update cache
    pub async fn read_through_region_by_id(&self, id: RegionId) -> Result<Region> {
        let region = self.inner_client.clone().get_region_by_id(id).await?;
        self.add_region(region.clone()).await;
        Ok(region)
    }

    pub async fn read_through_store_by_id(&self, id: StoreId) -> Result<Store> {
        let store = self.inner_client.clone().get_store(id).await?;
        self.store_cache.write().await.insert(id, store.clone());
        Ok(store)
    }

    pub async fn add_region(&self, region: Region) {
        // FIXME: will it be the performance bottleneck?
        let mut cache_by_id = self.cache_by_id.write().await;
        let mut cache_by_key = self.cache_by_key.write().await;

        let end_key = region.end_key();
        let mut to_be_removed: Vec<(RegionVerId, Key)> = Vec::new();

        let mut search_range = {
            if end_key.is_empty() {
                cache_by_key.range(..)
            } else {
                cache_by_key.range(..end_key)
            }
        };
        while let Some((_, region_in_cache)) = search_range.next_back() {
            if region_in_cache.region.end_key > region.region.start_key {
                to_be_removed.push((
                    region_in_cache.ver_id(),
                    region_in_cache.region.start_key.clone().into(),
                ));
            } else {
                break;
            }
        }

        for (ver_id, _) in &to_be_removed {
            let id = ver_id.id;
            match cache_by_id.get(&id) {
                Some(r) if &r.ver_id() == ver_id => {
                    cache_by_id.remove(&id);
                }
                _ => {}
            }
        }
        cache_by_id.insert(region.id(), region.clone());

        for (ver_id, start_key) in &to_be_removed {
            match cache_by_key.get(&start_key) {
                Some(r) if &r.ver_id() == ver_id => {
                    cache_by_key.remove(&start_key);
                }
                _ => {}
            }
        }
        cache_by_key.insert(region.start_key(), region);
    }
}
