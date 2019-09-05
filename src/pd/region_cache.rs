use super::Region;
use crate::Key;

use derive_new::new;
use std::collections::BTreeMap;

#[derive(new)]
pub struct RegionCache {
    /// Maps the start_key to the region
    #[new(default)]
    regions: BTreeMap<Key, Region>,
}

impl RegionCache {
    /// Returns the region which contains the given key in the region cache.
    /// Returns `None` if no region in the cache contains the key.
    pub fn find_region_by_key(&self, key: &Key) -> Option<Region> {
        let (_, candidate_region) = self.regions.range(..key).next_back()?;
        if candidate_region.contains(key) {
            Some(candidate_region.clone())
        } else {
            None
        }
    }

    /// Add a region to the cache. It removes intersecting regions in the cache automatically.
    pub fn add_region(&mut self, region: Region) {
        let end_key: Key = region.end_key().clone().into();
        let mut to_be_removed = Vec::new();
        let search_range = if end_key.is_empty() {
            self.regions.range(..)
        } else {
            self.regions.range(..end_key)
        };
        for (start_key, region_in_cache) in search_range {
            if region_in_cache.region.end_key > region.region.start_key {
                to_be_removed.push(start_key.clone());
            } else {
                break;
            }
        }
        for start_key in to_be_removed {
            self.regions.remove(&start_key);
        }
        self.regions.insert(region.start_key(), region);
    }

    /// Removes the region with the given start_key from the cache.
    pub fn remove_region_by_start_key(&mut self, start_key: &Key) {
        self.regions.remove(start_key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_disjoint_regions() {
        let mut cache = RegionCache::new();
        let region1 = region(vec![], vec![10]);
        let region2 = region(vec![10], vec![20]);
        let region3 = region(vec![30], vec![]);
        cache.add_region(region1.clone());
        cache.add_region(region2.clone());
        cache.add_region(region3.clone());

        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![], region1);
        expected_cache.insert(vec![10], region2);
        expected_cache.insert(vec![30], region3);
    }

    fn region(start_key: Vec<u8>, end_key: Vec<u8>) -> Region {
        let mut region = Region::default();
        region.region.set_start_key(vec![0]);
        region.region.set_end_key(vec![10]);
        // We don't care about other fields here

        region
    }
}
