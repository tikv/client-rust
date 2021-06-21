// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::RetryClient,
    region::{Region, RegionId, RegionVerId, StoreId},
    Key, Result,
};
use async_recursion::async_recursion;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use tikv_client_common::Error;
use tikv_client_pd::Cluster;
use tikv_client_proto::metapb::{self, Store};
use tokio::sync::{Notify, RwLock};

const MAX_RETRY_WAITING_CONCURRENT_REQUEST: usize = 4;

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
    /// We don't want to spawn multiple queries querying a same region id. If a
    /// request is on its way, others will wait for its completion.
    on_my_way_id: HashMap<RegionId, Arc<Notify>>,
}

impl RegionCacheMap {
    fn new() -> RegionCacheMap {
        RegionCacheMap {
            ver_id_to_region: HashMap::new(),
            key_to_ver_id: BTreeMap::new(),
            id_to_ver_id: HashMap::new(),
            on_my_way_id: HashMap::new(),
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
        drop(region_cache_guard);
        self.read_through_region_by_key(key.clone()).await
    }

    // Retrieve cache entry by RegionId. If there's no entry, query PD and update cache.
    pub async fn get_region_by_id(&self, id: RegionId) -> Result<Region> {
        for _ in 0..=MAX_RETRY_WAITING_CONCURRENT_REQUEST {
            let region_cache_guard = self.region_cache.read().await;

            // check cache
            let ver_id = region_cache_guard.id_to_ver_id.get(&id);
            if let Some(ver_id) = ver_id {
                let region = region_cache_guard.ver_id_to_region.get(ver_id).unwrap();
                return Ok(region.clone());
            }

            // check concurrent requests
            let mut notify = None;
            if let Some(n) = region_cache_guard.on_my_way_id.get(&id) {
                notify = Some(n.clone());
            }
            drop(region_cache_guard);

            if let Some(n) = notify {
                n.notified().await;
                continue;
            } else {
                return self.read_through_region_by_id(id).await;
            }
        }
        Err(Error::StringError(format!(
            "Concurrent PD requests failed for {} times",
            MAX_RETRY_WAITING_CONCURRENT_REQUEST
        )))
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
    async fn read_through_region_by_id(&self, id: RegionId) -> Result<Region> {
        // put a notify to let others know the region id is being queried
        let notify = Arc::new(Notify::new());
        {
            let mut region_cache_guard = self.region_cache.write().await;
            region_cache_guard.on_my_way_id.insert(id, notify.clone());
        }

        let region = self.inner_client.clone().get_region_by_id(id).await?;
        self.add_region(region.clone()).await;

        // notify others
        {
            let mut region_cache_guard = self.region_cache.write().await;
            notify.notify_waiters();
            region_cache_guard.on_my_way_id.remove(&id);
        }

        Ok(region)
    }

    async fn read_through_store_by_id(&self, id: StoreId) -> Result<Store> {
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

    pub async fn update_leader(
        &self,
        ver_id: crate::region::RegionVerId,
        leader: metapb::Peer,
    ) -> Result<()> {
        let mut cache = self.region_cache.write().await;
        let region_entry = cache.ver_id_to_region.get_mut(&ver_id).ok_or_else(|| {
            Error::StringError(
                "update leader failed: no corresponding entry in the region cache".to_owned(),
            )
        })?;
        region_entry.leader = Some(leader);
        Ok(())
    }

    pub async fn invalidate_region_cache(&self, ver_id: crate::region::RegionVerId) {
        let mut cache = self.region_cache.write().await;
        let region_entry = cache.ver_id_to_region.get(&ver_id);
        if let Some(region) = region_entry {
            let id = region.id();
            let start_key = region.start_key();
            cache.ver_id_to_region.remove(&ver_id);
            cache.id_to_ver_id.remove(&id);
            cache.key_to_ver_id.remove(&start_key);
        }
    }
}