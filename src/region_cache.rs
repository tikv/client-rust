// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::RetryClient,
    region::{Region, RegionId, RegionVerId, StoreId},
    Key, Result,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use tikv_client_pd::Cluster;
use tikv_client_proto::metapb::Store;
use tokio::sync::RwLock;

struct RegionCacheMap {
    /// RegionVerID -> Region. It stores the concrete region caches.
    /// RegionVerID is the unique identifer of a region *across time*.
    // TODO: does it need TTL?
    ver_id_to_region: HashMap<RegionVerId, Region>,
    /// Start_key -> RegionVerID
    ///
    /// Invariant: there are no intersecting regions in the map at any time.
    key_to_ver_id: BTreeMap<Key, RegionVerId>,
    /// RegionID -> RegionVerID. Note: regions with identical ID doesn't necessarily
    /// mean they are the same, they can be different regions across time.
    id_to_ver_id: HashMap<RegionId, RegionVerId>,
}

impl RegionCacheMap {
    fn new() -> RegionCacheMap {
        RegionCacheMap {
            ver_id_to_region: HashMap::new(),
            key_to_ver_id: BTreeMap::new(),
            id_to_ver_id: HashMap::new(),
        }
    }
}

pub struct RegionCache<Cl = Cluster> {
    region_cache: RwLock<RegionCacheMap>,
    store_cache: RwLock<HashMap<StoreId, Store>>,
    inner_client: Arc<RetryClient<Cl>>,
}

impl<Cl> RegionCache<Cl> {
    pub fn new(inner_client: Arc<RetryClient<Cl>>) -> RegionCache<Cl> {
        RegionCache {
            region_cache: RwLock::new(RegionCacheMap::new()),
            store_cache: RwLock::new(HashMap::new()),
            inner_client,
        }
    }
}

impl RegionCache<Cluster> {
    // Retrieve cache entry by key. If there's no entry, query PD and update cache.
    pub async fn get_region_by_key(&self, key: &Key) -> Result<Region> {
        let region_cache_guard = self.region_cache.read().await;
        let res = {
            region_cache_guard
                .key_to_ver_id
                .range(..=key)
                .next_back()
                .map(|(x, y)| (x.clone(), y.clone()))
        };

        if let Some((_, candidate_region_ver_id)) = res {
            let region = region_cache_guard
                .ver_id_to_region
                .get(&candidate_region_ver_id)
                .unwrap();

            if region.contains(key) {
                return Ok(region.clone());
            }
        }
        std::mem::drop(region_cache_guard);
        self.read_through_region_by_key(key.clone()).await
    }

    // Retrieve cache entry by RegionId. If there's no entry, query PD and update cache.
    pub async fn get_region_by_id(&self, id: RegionId) -> Result<Region> {
        let region_cache_guard = self.region_cache.read().await;
        let ver_id = region_cache_guard.id_to_ver_id.get(&id);
        if let Some(ver_id) = ver_id {
            let region = region_cache_guard.ver_id_to_region.get(&ver_id).unwrap();
            return Ok(region.clone());
        }

        std::mem::drop(region_cache_guard);
        self.read_through_region_by_id(id).await
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
        let mut cache = self.region_cache.write().await;

        let end_key = region.end_key();
        let mut to_be_removed: HashSet<RegionVerId> = HashSet::new();

        if let Some(ver_id) = cache.id_to_ver_id.get(&region.id()) {
            if ver_id != &region.ver_id() {
                to_be_removed.insert(ver_id.clone());
            }
        }

        let mut search_range = {
            if end_key.is_empty() {
                cache.key_to_ver_id.range(..)
            } else {
                cache.key_to_ver_id.range(..end_key)
            }
        };
        while let Some((_, ver_id_in_cache)) = search_range.next_back() {
            let region_in_cache = cache.ver_id_to_region.get(ver_id_in_cache).unwrap();

            if region_in_cache.region.end_key > region.region.start_key {
                to_be_removed.insert(ver_id_in_cache.clone());
            } else {
                break;
            }
        }

        for ver_id in to_be_removed {
            let region_to_remove = cache.ver_id_to_region.remove(&ver_id).unwrap();
            cache.key_to_ver_id.remove(&region_to_remove.start_key());
            cache.id_to_ver_id.remove(&region_to_remove.id());
        }
        cache
            .key_to_ver_id
            .insert(region.start_key(), region.ver_id());
        cache.id_to_ver_id.insert(region.id(), region.ver_id());
        cache.ver_id_to_region.insert(region.ver_id(), region);
    }
}
