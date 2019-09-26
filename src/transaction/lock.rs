use super::requests;
use crate::pd::{PdClient, RegionVerId};
use crate::request::KvRequest;
use crate::{ErrorKind, Key, Result, Timestamp};

use kvproto::kvrpcpb;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const RESOLVE_LOCK_RETRY_LIMIT: usize = 10;

/// _Resolves_ the given locks. Returns whether all the given locks are resolved.
///
/// If a key has a lock, the latest status of the key is unknown. We need to "resolve" the lock,
/// which means the key is finally either committed or rolled back, before we read the value of
/// the key. We first use `CleanupRequest` to let the status of the primary lock converge and get
/// its status (committed or rolled back). Then, we use the status of its primary lock to determine
/// the status of the other keys in the same transaction.
pub async fn resolve_locks(
    locks: Vec<kvrpcpb::LockInfo>,
    pd_client: Arc<impl PdClient>,
) -> Result<bool> {
    let ts = pd_client.clone().get_timestamp().await?;
    let mut has_live_locks = false;
    let expired_locks = locks.into_iter().filter(|lock| {
        let expired = ts.physical - Timestamp::from_version(lock.lock_version).physical
            >= lock.lock_ttl as i64;
        if !expired {
            has_live_locks = true;
        }
        expired
    });

    // records the commit version of each primary lock (representing the status of the transaction)
    let mut commit_versions: HashMap<u64, u64> = HashMap::new();
    let mut clean_regions: HashMap<u64, HashSet<RegionVerId>> = HashMap::new();
    for lock in expired_locks {
        let primary_key: Key = lock.primary_lock.into();
        let region_ver_id = pd_client.region_for_key(&primary_key).await?.ver_id();
        // skip if the region is cleaned
        if clean_regions
            .get(&lock.lock_version)
            .map(|regions| regions.contains(&region_ver_id))
            .unwrap_or(false)
        {
            continue;
        }

        let commit_version = match commit_versions.get(&lock.lock_version) {
            Some(&commit_version) => commit_version,
            None => {
                let commit_version = requests::new_cleanup_request(primary_key, lock.lock_version)
                    .execute(pd_client.clone())
                    .await?;
                commit_versions.insert(lock.lock_version, commit_version);
                commit_version
            }
        };

        let cleaned_region = resolve_lock_with_retry(
            lock.key.into(),
            lock.lock_version,
            commit_version,
            pd_client.clone(),
        )
        .await?;
        clean_regions
            .entry(lock.lock_version)
            .or_insert_with(HashSet::new)
            .insert(cleaned_region);
    }
    Ok(!has_live_locks)
}

async fn resolve_lock_with_retry(
    key: Key,
    start_version: u64,
    commit_version: u64,
    pd_client: Arc<impl PdClient>,
) -> Result<RegionVerId> {
    // TODO: Add backoff
    let mut error = None;
    for _ in 0..RESOLVE_LOCK_RETRY_LIMIT {
        let region = pd_client.region_for_key(&key).await?;
        let context = match region.context() {
            Ok(context) => context,
            Err(e) => {
                // Retry if the region has no leader
                error = Some(e);
                continue;
            }
        };
        match requests::new_resolve_lock_request(context, start_version, commit_version)
            .execute(pd_client.clone())
            .await
        {
            Ok(_) => {
                return Ok(region.ver_id());
            }
            Err(e) => match e.kind() {
                ErrorKind::RegionError(_) => {
                    // Retry on region error
                    error = Some(e);
                    continue;
                }
                _ => return Err(e),
            },
        }
    }
    Err(error.expect("no error is impossible"))
}

pub trait HasLocks {
    fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockPdClient;

    use futures::executor;

    #[test]
    fn test_resolve_lock_with_retry() {
        // Test resolve lock within retry limit
        fail::cfg("region-error", "9*return").unwrap();
        let client = Arc::new(MockPdClient);
        let key: Key = vec![1].into();
        let region1 = MockPdClient::region1();
        let resolved_region =
            executor::block_on(resolve_lock_with_retry(key, 1, 2, client.clone())).unwrap();
        assert_eq!(region1.ver_id(), resolved_region);

        // Test resolve lock over retry limit
        fail::cfg("region-error", "10*return").unwrap();
        let client = Arc::new(MockPdClient);
        let key: Key = vec![100].into();
        executor::block_on(resolve_lock_with_retry(key, 3, 4, client.clone()))
            .expect_err("should return error");
    }
}
