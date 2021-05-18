// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::RetryClient,
    region::{Region, RegionId},
    Key, Result,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tikv_client_pd::Cluster;

pub struct RegionCache<Cl = Cluster> {
    /// Start key -> Region
    ///
    /// Invariant: there are no intersecting regions in the map at any time.
    cache_by_key: BTreeMap<Key, Region>,
    /// Region ID -> Region. Note: regions with identical ID doesn't necessarily
    /// mean they are the same, they can be different regions across time. For
    /// example, when a region is splitted one of the new regions may have
    /// the same ID but a different RegionVerId.
    cache_by_id: HashMap<RegionId, Region>,
    inner_client: Arc<RetryClient<Cl>>,
}

impl<Cl> RegionCache<Cl> {
    pub fn new(inner_client: Arc<RetryClient<Cl>>) -> RegionCache<Cl> {
        RegionCache {
            cache_by_key: BTreeMap::new(),
            cache_by_id: HashMap::new(),
            inner_client,
        }
    }
}

impl RegionCache<Cluster> {
    // Retrieve cache entry by key. If there's no entry, query PD and update cache.
    pub async fn get_region_by_key(&mut self, key: &Key) -> Result<Region> {
        let res = self.cache_by_key.range(..=key).next_back();
        match res {
            Some((_, candidate_region)) if candidate_region.contains(key) => {
                Ok(candidate_region.clone())
            }
            _ => self.read_through_region_for_key(key.clone()).await,
        }
    }

    // Retrieve cache entry by RegionId. If there's no entry, query PD and update cache.
    pub async fn get_region_by_id(&mut self, id: RegionId) -> Result<Region> {
        match self.cache_by_id.get(&id) {
            Some(region) => Ok(region.clone()),
            None => self.read_through_region_for_id(id).await,
        }
    }

    /// Force read through (query from PD) and update cache
    pub async fn read_through_region_for_key(&mut self, key: Key) -> Result<Region> {
        let region = self.inner_client.clone().get_region(key.into()).await?;
        self.add_region(region.clone());
        Ok(region)
    }

    /// Force read through (query from PD) and update cache
    pub async fn read_through_region_for_id(&mut self, id: RegionId) -> Result<Region> {
        let region = self.inner_client.clone().get_region_by_id(id).await?;
        self.add_region(region.clone());
        Ok(region)
    }

    pub fn add_region(&mut self, region: Region) {
        let end_key = region.end_key();
        let mut to_be_removed: Vec<(RegionId, Key)> = Vec::new();
        let mut search_range = if end_key.is_empty() {
            self.cache_by_key.range(..)
        } else {
            self.cache_by_key.range(..end_key)
        };
        while let Some((_, region_in_cache)) = search_range.next_back() {
            if region_in_cache.region.end_key > region.region.start_key {
                to_be_removed.push((
                    region_in_cache.id(),
                    region_in_cache.region.start_key.clone().into(),
                ));
            } else {
                break;
            }
        }

        for (id, start_key) in to_be_removed {
            self.cache_by_id.remove(&id);
            self.cache_by_key.remove(&start_key);
        }

        self.cache_by_id.insert(region.id(), region.clone());
        self.cache_by_key.insert(region.start_key(), region);
    }
}
