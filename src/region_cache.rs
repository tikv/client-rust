// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use log::{debug, warn};
use tokio::sync::RwLock;
use tokio::sync::{watch, Mutex, Notify};

use crate::common::Error;
use crate::pd::Cluster;
use crate::pd::RetryClient;
use crate::pd::RetryClientTrait;
use crate::proto::metapb::Store;
use crate::proto::metapb::{self};
use crate::region::RegionId;
use crate::region::RegionVerId;
use crate::region::RegionWithLeader;
use crate::region::StoreId;
use crate::Key;
use crate::Result;

const MAX_RETRY_WAITING_CONCURRENT_REQUEST: usize = 4;

struct RegionCacheMap {
    /// RegionVerID -> Region. It stores the concrete region caches.
    /// RegionVerID is the unique identifer of a region *across time*.
    // TODO: does it need TTL?
    ver_id_to_region: HashMap<RegionVerId, RegionWithLeader>,
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

// A refresh is shared independently of any individual caller's cancellation.
struct StoreReload {
    started_at: Instant,
    result: watch::Receiver<Option<std::result::Result<Store, String>>>,
}

pub struct RegionCache<Client = RetryClient<Cluster>> {
    region_cache: RwLock<RegionCacheMap>,
    store_cache: Arc<RwLock<HashMap<StoreId, (Instant, Store)>>>,
    store_reloads: Mutex<HashMap<StoreId, StoreReload>>,
    inner_client: Arc<Client>,
}

impl<Client> RegionCache<Client> {
    pub fn new(inner_client: Arc<Client>) -> RegionCache<Client> {
        RegionCache {
            region_cache: RwLock::new(RegionCacheMap::new()),
            store_cache: Arc::new(RwLock::new(HashMap::new())),
            store_reloads: Mutex::new(HashMap::new()),
            inner_client,
        }
    }
}

impl<C: RetryClientTrait> RegionCache<C> {
    // Retrieve cache entry by key. If there's no entry, query PD and update cache.
    pub async fn get_region_by_key(&self, key: &Key) -> Result<RegionWithLeader> {
        let region_cache_guard = self.region_cache.read().await;
        let res = {
            region_cache_guard
                .key_to_ver_id
                .range::<Key, _>(..=key)
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
    pub async fn get_region_by_id(&self, id: RegionId) -> Result<RegionWithLeader> {
        for _ in 0..=MAX_RETRY_WAITING_CONCURRENT_REQUEST {
            let region_cache_guard = self.region_cache.read().await;

            // check cache
            let ver_id = region_cache_guard.id_to_ver_id.get(&id);
            if let Some(ver_id) = ver_id {
                let region = region_cache_guard.ver_id_to_region.get(ver_id).unwrap();
                return Ok(region.clone());
            }

            // check concurrent requests
            let notify = region_cache_guard.on_my_way_id.get(&id).cloned();
            let notified = notify.as_ref().map(|notify| notify.notified());
            drop(region_cache_guard);

            if let Some(n) = notified {
                n.await;
                continue;
            } else {
                return self.read_through_region_by_id(id).await;
            }
        }
        Err(Error::StringError(format!(
            "Concurrent PD requests failed for {MAX_RETRY_WAITING_CONCURRENT_REQUEST} times"
        )))
    }

    pub async fn get_store_by_id(&self, id: StoreId) -> Result<Store> {
        let store = self
            .store_cache
            .read()
            .await
            .get(&id)
            .map(|(_, store)| store.clone());
        match store {
            Some(store) => Ok(store),
            None => self.read_through_store_by_id(id).await,
        }
    }

    /// Force read through (query from PD) and update cache
    pub async fn read_through_region_by_key(&self, key: Key) -> Result<RegionWithLeader> {
        let region = self.inner_client.clone().get_region(key.into()).await?;
        self.add_region(region.clone()).await;
        Ok(region)
    }

    /// Force read through (query from PD) and update cache
    async fn read_through_region_by_id(&self, id: RegionId) -> Result<RegionWithLeader> {
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
        let started_at = Instant::now();
        let store = self.inner_client.clone().get_store(id).await?;
        let mut cache = self.store_cache.write().await;
        Ok(publish_store(&mut cache, started_at, store))
    }

    /// Reload from PD at most once per second per store. Running refreshes are
    /// joined even after cooldown; canceling a waiter does not cancel the fetch.
    pub async fn reload_store(&self, id: StoreId) -> Result<Store>
    where
        C: Send + Sync + 'static,
    {
        let mut reloads = self.store_reloads.lock().await;
        let reuse = reloads.get(&id).is_some_and(|reload| {
            // A stopped task may leave an empty result behind. Only an open
            // channel represents an in-flight fetch; stopped tasks still enter
            // cooldown but must not prevent subsequent reloads indefinitely.
            let running = reload.result.borrow().is_none() && reload.result.has_changed().is_ok();
            if running {
                crate::stats::txn_protocol_store_reload("joined");
                true
            } else if reload.started_at.elapsed() < Duration::from_secs(1) {
                crate::stats::txn_protocol_store_reload("suppressed");
                true
            } else {
                false
            }
        });
        if !reuse {
            let started_at = Instant::now();
            let (sender, receiver) = watch::channel(None);
            reloads.insert(
                id,
                StoreReload {
                    started_at,
                    result: receiver,
                },
            );
            let client = self.inner_client.clone();
            let cache = self.store_cache.clone();
            tokio::spawn(async move {
                crate::stats::txn_protocol_store_reload("started");
                let fetched = match tokio::time::timeout(
                    Duration::from_secs(5),
                    client.get_store(id),
                )
                .await
                {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                let result = match fetched {
                    Ok(store) => {
                        let mut cache = cache.write().await;
                        let published = publish_store(&mut cache, started_at, store);
                        crate::stats::txn_protocol_store_reload("success");
                        Ok(published)
                    }
                    Err(error) => {
                        crate::stats::txn_protocol_store_reload("failure");
                        debug!("store metadata refresh failed for {}: {}", id, error);
                        Err(error)
                    }
                };
                sender.send_replace(Some(result));
            });
        }
        let mut receiver = reloads.get(&id).unwrap().result.clone();
        drop(reloads);
        loop {
            let result = receiver.borrow().clone();
            if let Some(result) = result {
                // Other metadata fetches may have published a newer snapshot while
                // this refresh was running, including after a failed refresh.
                if let Some((_, store)) = self.store_cache.read().await.get(&id) {
                    return Ok(store.clone());
                }
                return result.map_err(Error::StringError);
            }
            receiver.changed().await.map_err(|_| {
                Error::StringError("store metadata refresh task stopped".to_owned())
            })?;
        }
    }

    pub async fn add_region(&self, region: RegionWithLeader) {
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
                cache.key_to_ver_id.range::<Key, _>(..)
            } else {
                cache.key_to_ver_id.range::<Key, _>(..end_key)
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
        let region_entry = cache.ver_id_to_region.get_mut(&ver_id);
        if let Some(region) = region_entry {
            region.leader = Some(leader);
            debug!("updated cached region leader, region: {:?}", ver_id);
        }

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
            debug!("invalidated region cache entry, region: {:?}", ver_id);
        }
    }

    pub async fn invalidate_store_cache(&self, store_id: StoreId) -> Option<Store> {
        let mut cache = self.store_cache.write().await;
        let removed = cache.remove(&store_id).map(|(_, store)| store);
        if removed.is_some() {
            debug!("invalidated store cache entry, store: {:?}", store_id);
        }
        removed
    }

    pub async fn read_through_all_stores(&self) -> Result<Vec<Store>> {
        let started_at = Instant::now();
        let mut stores = self
            .inner_client
            .clone()
            .get_all_stores()
            .await?
            .into_iter()
            .filter(is_valid_tikv_store)
            .collect::<Vec<_>>();
        let mut cache = self.store_cache.write().await;
        for store in &mut stores {
            *store = publish_store(&mut cache, started_at, store.clone());
        }
        Ok(stores)
    }
}

/// Publish metadata consistently for cache misses, conditional reloads, and
/// full-store fetches. Invalid protocol ranges retain the last valid range (or
/// unknown); an absent range is a legitimate downgrade and replaces it.
fn publish_store(
    cache: &mut HashMap<StoreId, (Instant, Store)>,
    fetched_at: Instant,
    mut store: Store,
) -> Store {
    let previous = cache.get(&store.id);
    if let Some((published_at, cached)) = previous {
        if fetched_at < *published_at {
            return cached.clone();
        }
    }
    if let Some(range) = store.txn_protocol_version_range.as_ref() {
        if range.min > range.max {
            warn!(
                "ignoring invalid transaction protocol range [{}, {}] for store {}",
                range.min, range.max, store.id,
            );
            store.txn_protocol_version_range =
                previous.and_then(|(_, cached)| cached.txn_protocol_version_range.clone());
        }
    }
    // Advance the fetch timestamp even when retaining the previous range, so
    // an older in-flight lookup cannot replace it after this publication.
    cache.insert(store.id, (fetched_at, store.clone()));
    store
}

const ENGINE_LABEL_KEY: &str = "engine";
const ENGINE_LABEL_TIFLASH: &str = "tiflash";
const ENGINE_LABEL_TIFLASH_COMPUTE: &str = "tiflash_compute";

fn is_valid_tikv_store(store: &metapb::Store) -> bool {
    if metapb::StoreState::try_from(store.state).unwrap() == metapb::StoreState::Tombstone {
        return false;
    }
    let is_tiflash = store.labels.iter().any(|label| {
        label.key == ENGINE_LABEL_KEY
            && (label.value == ENGINE_LABEL_TIFLASH || label.value == ENGINE_LABEL_TIFLASH_COMPUTE)
    });
    !is_tiflash
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering::SeqCst;
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use super::RegionCache;
    use crate::common::Error;
    use crate::pd::RetryClientTrait;
    use crate::proto::keyspacepb;
    use crate::proto::metapb::RegionEpoch;
    use crate::proto::metapb::{self};
    use crate::region::RegionId;
    use crate::region::RegionWithLeader;
    use crate::region_cache::is_valid_tikv_store;
    use crate::Key;
    use crate::Result;

    #[derive(Default)]
    struct MockRetryClient {
        pub regions: Mutex<HashMap<RegionId, RegionWithLeader>>,
        pub get_region_count: AtomicU64,
        pub stores: Mutex<HashMap<u64, metapb::Store>>,
        pub get_store_count: AtomicU64,
        pub fail_get_store: std::sync::atomic::AtomicBool,
        pub get_store_gate: Option<Arc<tokio::sync::Semaphore>>,
    }

    #[async_trait]
    impl RetryClientTrait for MockRetryClient {
        async fn get_region(
            self: Arc<Self>,
            key: Vec<u8>,
        ) -> Result<crate::region::RegionWithLeader> {
            self.get_region_count.fetch_add(1, SeqCst);
            self.regions
                .lock()
                .await
                .iter()
                .filter(|(_, r)| r.contains(&key.clone().into()))
                .map(|(_, r)| r.clone())
                .next()
                .ok_or_else(|| Error::StringError("MockRetryClient: region not found".to_owned()))
        }

        async fn get_region_by_id(
            self: Arc<Self>,
            region_id: crate::region::RegionId,
        ) -> Result<crate::region::RegionWithLeader> {
            self.get_region_count.fetch_add(1, SeqCst);
            self.regions
                .lock()
                .await
                .iter()
                .filter(|(id, _)| id == &&region_id)
                .map(|(_, r)| r.clone())
                .next()
                .ok_or_else(|| Error::StringError("MockRetryClient: region not found".to_owned()))
        }

        async fn get_store(
            self: Arc<Self>,
            id: crate::region::StoreId,
        ) -> Result<crate::proto::metapb::Store> {
            self.get_store_count.fetch_add(1, SeqCst);
            if let Some(gate) = &self.get_store_gate {
                gate.acquire().await.unwrap().forget();
            }
            if self.fail_get_store.load(SeqCst) {
                return Err(Error::StringError("PD unavailable".into()));
            }
            self.stores
                .lock()
                .await
                .get(&id)
                .cloned()
                .ok_or_else(|| Error::StringError("store not found".into()))
        }

        async fn get_all_stores(self: Arc<Self>) -> Result<Vec<crate::proto::metapb::Store>> {
            Ok(self.stores.lock().await.values().cloned().collect())
        }

        async fn get_timestamp(self: Arc<Self>) -> Result<crate::proto::pdpb::Timestamp> {
            todo!()
        }

        async fn update_safepoint(self: Arc<Self>, _safepoint: u64) -> Result<bool> {
            todo!()
        }

        async fn load_keyspace(&self, _keyspace: &str) -> Result<keyspacepb::KeyspaceMeta> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn cache_is_used() -> Result<()> {
        let retry_client = Arc::new(MockRetryClient::default());
        let cache = RegionCache::new(retry_client.clone());
        retry_client.regions.lock().await.insert(
            1,
            RegionWithLeader {
                region: metapb::Region {
                    id: 1,
                    start_key: vec![],
                    end_key: vec![100],
                    region_epoch: Some(RegionEpoch {
                        conf_ver: 0,
                        version: 0,
                    }),
                    ..Default::default()
                },
                leader: Some(metapb::Peer {
                    store_id: 1,
                    ..Default::default()
                }),
            },
        );
        retry_client.regions.lock().await.insert(
            2,
            RegionWithLeader {
                region: metapb::Region {
                    id: 2,
                    start_key: vec![101],
                    end_key: vec![],
                    region_epoch: Some(RegionEpoch {
                        conf_ver: 0,
                        version: 0,
                    }),
                    ..Default::default()
                },
                leader: Some(metapb::Peer {
                    store_id: 2,
                    ..Default::default()
                }),
            },
        );

        assert_eq!(retry_client.get_region_count.load(SeqCst), 0);

        // first query, read through
        assert_eq!(cache.get_region_by_id(1).await?.end_key(), vec![100].into());
        assert_eq!(retry_client.get_region_count.load(SeqCst), 1);

        // should read from cache
        assert_eq!(cache.get_region_by_id(1).await?.end_key(), vec![100].into());
        assert_eq!(retry_client.get_region_count.load(SeqCst), 1);

        // invalidate, should read through
        cache
            .invalidate_region_cache(cache.get_region_by_id(1).await?.ver_id())
            .await;
        assert_eq!(cache.get_region_by_id(1).await?.end_key(), vec![100].into());
        assert_eq!(retry_client.get_region_count.load(SeqCst), 2);

        // update leader should work
        cache
            .update_leader(
                cache.get_region_by_id(2).await?.ver_id(),
                metapb::Peer {
                    store_id: 102,
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            cache.get_region_by_id(2).await?.leader.unwrap().store_id,
            102
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_add_disjoint_regions() {
        let retry_client = Arc::new(MockRetryClient::default());
        let cache = RegionCache::new(retry_client.clone());
        let region1 = region(1, vec![], vec![10]);
        let region2 = region(2, vec![10], vec![20]);
        let region3 = region(3, vec![30], vec![]);
        cache.add_region(region1.clone()).await;
        cache.add_region(region2.clone()).await;
        cache.add_region(region3.clone()).await;

        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![].into(), region1);
        expected_cache.insert(vec![10].into(), region2);
        expected_cache.insert(vec![30].into(), region3);

        assert(&cache, &expected_cache).await
    }

    #[tokio::test]
    async fn test_add_intersecting_regions() {
        let retry_client = Arc::new(MockRetryClient::default());
        let cache = RegionCache::new(retry_client.clone());

        cache.add_region(region(1, vec![], vec![10])).await;
        cache.add_region(region(2, vec![10], vec![20])).await;
        cache.add_region(region(3, vec![30], vec![40])).await;
        cache.add_region(region(4, vec![50], vec![60])).await;
        cache.add_region(region(5, vec![20], vec![35])).await;

        let mut expected_cache: BTreeMap<Key, _> = BTreeMap::new();
        expected_cache.insert(vec![].into(), region(1, vec![], vec![10]));
        expected_cache.insert(vec![10].into(), region(2, vec![10], vec![20]));
        expected_cache.insert(vec![20].into(), region(5, vec![20], vec![35]));
        expected_cache.insert(vec![50].into(), region(4, vec![50], vec![60]));
        assert(&cache, &expected_cache).await;

        cache.add_region(region(6, vec![15], vec![25])).await;
        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![].into(), region(1, vec![], vec![10]));
        expected_cache.insert(vec![15].into(), region(6, vec![15], vec![25]));
        expected_cache.insert(vec![50].into(), region(4, vec![50], vec![60]));
        assert(&cache, &expected_cache).await;

        cache.add_region(region(7, vec![20], vec![])).await;
        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![].into(), region(1, vec![], vec![10]));
        expected_cache.insert(vec![20].into(), region(7, vec![20], vec![]));
        assert(&cache, &expected_cache).await;

        cache.add_region(region(8, vec![], vec![15])).await;
        let mut expected_cache = BTreeMap::new();
        expected_cache.insert(vec![].into(), region(8, vec![], vec![15]));
        expected_cache.insert(vec![20].into(), region(7, vec![20], vec![]));
        assert(&cache, &expected_cache).await;
    }

    #[tokio::test]
    async fn test_get_region_by_key() -> Result<()> {
        let retry_client = Arc::new(MockRetryClient::default());
        let cache = RegionCache::new(retry_client.clone());

        let region1 = region(1, vec![], vec![10]);
        let region2 = region(2, vec![10], vec![20]);
        let region3 = region(3, vec![30], vec![40]);
        let region4 = region(4, vec![50], vec![]);
        cache.add_region(region1.clone()).await;
        cache.add_region(region2.clone()).await;
        cache.add_region(region3.clone()).await;
        cache.add_region(region4.clone()).await;

        assert_eq!(
            cache.get_region_by_key(&vec![].into()).await?,
            region1.clone()
        );
        assert_eq!(
            cache.get_region_by_key(&vec![5].into()).await?,
            region1.clone()
        );
        assert_eq!(
            cache.get_region_by_key(&vec![10].into()).await?,
            region2.clone()
        );
        assert!(cache.get_region_by_key(&vec![20].into()).await.is_err());
        assert!(cache.get_region_by_key(&vec![25].into()).await.is_err());
        assert_eq!(cache.get_region_by_key(&vec![60].into()).await?, region4);
        Ok(())
    }

    // a helper function to assert the cache is in expected state
    async fn assert(
        cache: &RegionCache<MockRetryClient>,
        expected_cache: &BTreeMap<Key, RegionWithLeader>,
    ) {
        let guard = cache.region_cache.read().await;
        let mut actual_keys = guard.ver_id_to_region.values().collect::<Vec<_>>();
        let mut expected_keys = expected_cache.values().collect::<Vec<_>>();
        actual_keys.sort_by_cached_key(|r| r.id());
        expected_keys.sort_by_cached_key(|r| r.id());

        assert_eq!(actual_keys, expected_keys);
        assert_eq!(
            guard.key_to_ver_id.keys().collect::<HashSet<_>>(),
            expected_cache.keys().collect::<HashSet<_>>()
        )
    }

    fn region(id: RegionId, start_key: Vec<u8>, end_key: Vec<u8>) -> RegionWithLeader {
        let mut region = RegionWithLeader::default();
        region.region.id = id;
        region.region.start_key = start_key;
        region.region.end_key = end_key;
        region.region.region_epoch = Some(RegionEpoch {
            conf_ver: 0,
            version: 0,
        });
        // We don't care about other fields here

        region
    }

    #[test]
    fn test_is_valid_tikv_store() {
        let mut store = metapb::Store::default();
        assert!(is_valid_tikv_store(&store));

        store.state = metapb::StoreState::Tombstone.into();
        assert!(!is_valid_tikv_store(&store));

        store.state = metapb::StoreState::Up.into();
        assert!(is_valid_tikv_store(&store));

        store.labels.push(metapb::StoreLabel {
            key: "some_key".to_owned(),
            value: "some_value".to_owned(),
        });
        assert!(is_valid_tikv_store(&store));

        store.labels.push(metapb::StoreLabel {
            key: "engine".to_owned(),
            value: "tiflash".to_owned(),
        });
        assert!(!is_valid_tikv_store(&store));

        store.labels[1].value = "tiflash_compute".to_string();
        assert!(!is_valid_tikv_store(&store));

        store.labels[1].value = "other".to_string();
        assert!(is_valid_tikv_store(&store));
    }

    fn store_metadata(id: u64, range: Option<(u32, u32)>) -> metapb::Store {
        metapb::Store {
            id,
            txn_protocol_version_range: range
                .map(|(min, max)| metapb::TxnProtocolVersionRange { min, max }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn protocol_metadata_is_validated_on_every_fetch_path() {
        let pd = Arc::new(MockRetryClient::default());
        pd.stores
            .lock()
            .await
            .insert(7, store_metadata(7, Some((0, 1))));
        let cache = RegionCache::new(pd.clone());
        let initial = cache.get_store_by_id(7).await.unwrap();
        pd.stores
            .lock()
            .await
            .insert(7, store_metadata(7, Some((2, 1))));
        assert_eq!(
            cache
                .reload_store(7)
                .await
                .unwrap()
                .txn_protocol_version_range,
            initial.txn_protocol_version_range
        );
        assert_eq!(
            cache.read_through_all_stores().await.unwrap()[0].txn_protocol_version_range,
            initial.txn_protocol_version_range
        );
        // Absence legitimately downgrades a previously advertised range.
        pd.stores.lock().await.insert(7, store_metadata(7, None));
        assert!(cache.read_through_all_stores().await.unwrap()[0]
            .txn_protocol_version_range
            .is_none());
        assert!(cache
            .get_store_by_id(7)
            .await
            .unwrap()
            .txn_protocol_version_range
            .is_none());
        // An invalid initial fetch must publish unknown rather than invalid bounds.
        pd.stores
            .lock()
            .await
            .insert(8, store_metadata(8, Some((3, 1))));
        assert!(cache
            .get_store_by_id(8)
            .await
            .unwrap()
            .txn_protocol_version_range
            .is_none());
    }

    #[test]
    fn protocol_publication_keeps_last_valid_range_and_orders_fetches() {
        let mut cache = HashMap::new();
        let now = super::Instant::now();
        let valid = store_metadata(7, Some((0, 1)));
        super::publish_store(&mut cache, now, valid.clone());
        let newer = now + std::time::Duration::from_secs(2);
        let retained = super::publish_store(&mut cache, newer, store_metadata(7, Some((3, 1))));
        assert_eq!(
            retained.txn_protocol_version_range,
            valid.txn_protocol_version_range
        );
        // Even an invalid newer fetch advances publication ordering.
        let late = super::publish_store(
            &mut cache,
            now + std::time::Duration::from_secs(1),
            store_metadata(7, None),
        );
        assert_eq!(
            late.txn_protocol_version_range,
            valid.txn_protocol_version_range
        );
        let legacy = super::publish_store(
            &mut cache,
            newer + std::time::Duration::from_secs(1),
            store_metadata(7, None),
        );
        assert!(legacy.txn_protocol_version_range.is_none());
    }

    #[tokio::test]
    async fn protocol_reload_is_shared_survives_cancellation_and_respects_cooldown() {
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        let pd = Arc::new(MockRetryClient {
            get_store_gate: Some(gate.clone()),
            ..Default::default()
        });
        pd.stores
            .lock()
            .await
            .insert(7, store_metadata(7, Some((0, 1))));
        let cache = Arc::new(RegionCache::new(pd.clone()));
        let owner_cache = cache.clone();
        let owner = tokio::spawn(async move { owner_cache.reload_store(7).await });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while pd.get_store_count.load(SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut waiter = Box::pin(cache.reload_store(7));
        assert!(futures::poll!(waiter.as_mut()).is_pending());
        owner.abort();
        assert!(owner.await.unwrap_err().is_cancelled());
        // A long-running fetch remains shared even after cooldown expires.
        cache
            .store_reloads
            .lock()
            .await
            .get_mut(&7)
            .unwrap()
            .started_at = super::Instant::now() - std::time::Duration::from_secs(2);
        let mut late_waiter = Box::pin(cache.reload_store(7));
        assert!(futures::poll!(late_waiter.as_mut()).is_pending());
        assert_eq!(pd.get_store_count.load(SeqCst), 1);
        gate.add_permits(1);
        let first = waiter.await.unwrap();
        assert_eq!(late_waiter.await.unwrap(), first);
        assert_eq!(pd.get_store_count.load(SeqCst), 1);
        // Completed results are reused inside cooldown, then refreshed afterward.
        cache
            .store_reloads
            .lock()
            .await
            .get_mut(&7)
            .unwrap()
            .started_at = super::Instant::now();
        pd.stores
            .lock()
            .await
            .insert(7, store_metadata(7, Some((0, 0))));
        assert_eq!(cache.reload_store(7).await.unwrap(), first);
        assert_eq!(pd.get_store_count.load(SeqCst), 1);
        cache
            .store_reloads
            .lock()
            .await
            .get_mut(&7)
            .unwrap()
            .started_at = super::Instant::now() - std::time::Duration::from_secs(2);
        gate.add_permits(1);
        assert_eq!(
            cache.reload_store(7).await.unwrap(),
            store_metadata(7, Some((0, 0)))
        );
        assert_eq!(pd.get_store_count.load(SeqCst), 2);
    }

    #[tokio::test]
    async fn protocol_failed_reload_preserves_cache_and_enters_cooldown() {
        let pd = Arc::new(MockRetryClient::default());
        let original = store_metadata(7, Some((0, 1)));
        pd.stores.lock().await.insert(7, original.clone());
        let cache = RegionCache::new(pd.clone());
        assert_eq!(cache.get_store_by_id(7).await.unwrap(), original);
        pd.fail_get_store.store(true, SeqCst);
        assert_eq!(cache.reload_store(7).await.unwrap(), original);
        assert_eq!(cache.reload_store(7).await.unwrap(), original);
        assert_eq!(pd.get_store_count.load(SeqCst), 2); // Initial fetch + one failed reload.
    }

    #[tokio::test]
    async fn protocol_stopped_reload_can_restart_after_cooldown() {
        let pd = Arc::new(MockRetryClient::default());
        pd.stores.lock().await.insert(7, store_metadata(7, None));
        let cache = RegionCache::new(pd.clone());
        let (sender, receiver) = tokio::sync::watch::channel(None);
        drop(sender);
        cache.store_reloads.lock().await.insert(
            7,
            super::StoreReload {
                started_at: super::Instant::now(),
                result: receiver,
            },
        );
        assert!(cache.reload_store(7).await.is_err());
        assert_eq!(pd.get_store_count.load(SeqCst), 0);
        cache
            .store_reloads
            .lock()
            .await
            .get_mut(&7)
            .unwrap()
            .started_at = super::Instant::now() - std::time::Duration::from_secs(2);
        assert_eq!(
            cache.reload_store(7).await.unwrap(),
            store_metadata(7, None)
        );
        assert_eq!(pd.get_store_count.load(SeqCst), 1);
    }

    #[tokio::test]
    async fn protocol_reload_timeout_does_not_poison_later_attempts() {
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        let pd = Arc::new(MockRetryClient {
            get_store_gate: Some(gate.clone()),
            ..Default::default()
        });
        pd.stores.lock().await.insert(7, store_metadata(7, None));
        let cache = RegionCache::new(pd.clone());
        let failed = tokio::time::timeout(std::time::Duration::from_secs(6), cache.reload_store(7))
            .await
            .unwrap();
        assert!(failed.is_err());
        assert_eq!(pd.get_store_count.load(SeqCst), 1);
        // Cooldown starts with the attempt, so the timed-out fetch can be retried.
        gate.add_permits(1);
        assert_eq!(
            cache.reload_store(7).await.unwrap(),
            store_metadata(7, None)
        );
        assert_eq!(pd.get_store_count.load(SeqCst), 2);
    }
}
