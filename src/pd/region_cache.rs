// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use tikv_client_store::{Region, RegionId};
use crate::Key;

use derive_new::new;
use std::collections::{BTreeMap, HashMap};

#[derive(new)]
pub struct RegionCache {
    /// Maps the start_key to the end key and region.
    #[new(default)]
    regions: BTreeMap<Key, Region>,
    #[new(default)]
    id_to_start_key: HashMap<RegionId, Key>,
}

impl RegionCache {
    /// Returns the region which contains the given key in the region cache.
    /// Returns `None` if no region in the cache contains the key.
    pub fn find_region_by_key(&self, key: &Key) -> Option<Region> {
        let (_, candidate_region) = self.regions.range(..=key).next_back()?;
        if candidate_region.contains(key) {
            Some(candidate_region.clone())
        } else {
            None
        }
    }

    pub fn find_region_by_id(&self, id: RegionId) -> Option<Region> {
        let start_key = self.id_to_start_key.get(&id)?;
        self.regions.get(start_key).cloned()
    }

    /// Add a region to the cache. It removes intersecting regions in the cache automatically.
    pub fn add_region(&mut self, region: Region) {
        let end_key: Key = region.end_key();
        let mut to_be_removed = Vec::new();
        let mut search_range = if end_key.is_empty() {
            self.regions.range(..)
        } else {
            self.regions.range(..end_key)
        };
        while let Some((_, region_in_cache)) = search_range.next_back() {
            if region_in_cache.region.end_key > region.region.start_key {
                to_be_removed.push(region_in_cache.id());
            } else {
                break;
            }
        }
        for id in to_be_removed {
            let start_key = match self.id_to_start_key.remove(&id) {
                Some(id) => id,
                None => {
                    // Cache must be corrupt, give up and start again.
                    self.clear();
                    break;
                }
            };
            self.regions.remove(&start_key);
        }
        self.id_to_start_key.insert(region.id(), region.start_key());
        self.regions.insert(region.start_key(), region);
    }

    /// Removes the region with the given start_key from the cache.
    pub fn remove_region_by_start_key(&mut self, start_key: &Key) {
        if let Some(region) = self.regions.remove(start_key) {
            self.id_to_start_key.remove(&region.id());
        }
    }

    /// Removes the region with the given region ID from the cache.
    pub fn remove_region_by_id(&mut self, id: RegionId) {
        if let Some(start_key) = self.id_to_start_key.remove(&id) {
            self.regions.remove(&start_key);
        }
    }

    fn clear(&mut self) {
        self.regions.clear();
        self.id_to_start_key.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_disjoint_regions() {
        let mut cache = RegionCache::new();
        let region1 = region(1, vec![], vec![10]);
        let region2 = region(2, vec![10], vec![20]);
        let region3 = region(3, vec![30], vec![]);
        cache.add_region(region1.clone());
        cache.add_region(region2.clone());
        cache.add_region(region3.clone());

        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![].into(), region1);
        expected_cache.insert(vec![10].into(), region2);
        expected_cache.insert(vec![30].into(), region3);
        assert_eq!(cache.regions, expected_cache);
    }

    #[test]
    fn test_add_joint_regions() {
        let mut cache = RegionCache::new();
        cache.add_region(region(1, vec![], vec![10]));
        cache.add_region(region(2, vec![10], vec![20]));
        cache.add_region(region(3, vec![30], vec![40]));
        cache.add_region(region(4, vec![50], vec![60]));
        cache.add_region(region(5, vec![20], vec![35]));

        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![].into(), region(1, vec![], vec![10]));
        expected_cache.insert(vec![10].into(), region(2, vec![10], vec![20]));
        expected_cache.insert(vec![20].into(), region(5, vec![20], vec![35]));
        expected_cache.insert(vec![50].into(), region(4, vec![50], vec![60]));

        assert_eq!(cache.regions, expected_cache);

        cache.add_region(region(6, vec![15], vec![25]));

        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![].into(), region(1, vec![], vec![10]));
        expected_cache.insert(vec![15].into(), region(6, vec![15], vec![25]));
        expected_cache.insert(vec![50].into(), region(4, vec![50], vec![60]));
        assert_eq!(cache.regions, expected_cache);

        cache.add_region(region(7, vec![20], vec![]));

        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![].into(), region(1, vec![], vec![10]));
        expected_cache.insert(vec![20].into(), region(7, vec![20], vec![]));

        assert_eq!(cache.regions, expected_cache);

        cache.add_region(region(8, vec![], vec![15]));

        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![].into(), region(8, vec![], vec![15]));
        expected_cache.insert(vec![20].into(), region(7, vec![20], vec![]));

        assert_eq!(cache.regions, expected_cache);
    }

    #[test]
    fn test_find_region_by_key() {
        let mut cache = RegionCache::new();
        cache.add_region(region(1, vec![], vec![10]));
        cache.add_region(region(2, vec![10], vec![20]));
        cache.add_region(region(3, vec![30], vec![40]));
        cache.add_region(region(4, vec![50], vec![]));

        assert_eq!(
            cache.find_region_by_key(&vec![].into()),
            Some(region(1, vec![], vec![10]))
        );
        assert_eq!(
            cache.find_region_by_key(&vec![5].into()),
            Some(region(1, vec![], vec![10]))
        );
        assert_eq!(
            cache.find_region_by_key(&vec![10].into()),
            Some(region(2, vec![10], vec![20]))
        );
        assert_eq!(cache.find_region_by_key(&vec![20].into()), None);
        assert_eq!(cache.find_region_by_key(&vec![25].into()), None);
        assert_eq!(
            cache.find_region_by_key(&vec![60].into()),
            Some(region(4, vec![50], vec![]))
        );
    }

    fn region(id: u64, start_key: Vec<u8>, end_key: Vec<u8>) -> Region {
        let mut region = Region::default();
        region.region.set_start_key(start_key);
        region.region.set_end_key(end_key);
        region.region.set_id(id);
        // We don't care about other fields here

        region
    }
}
