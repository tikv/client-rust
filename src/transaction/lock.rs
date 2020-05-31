use super::requests;
use crate::pd::{PdClient, Region, RegionVerId};
use crate::request::KvRequest;
use crate::{ErrorKind, Key, Result, Timestamp};

use crate::backoff::{Backoff, NoJitterBackoff};
use kvproto::kvrpcpb;
use std::{collections::HashMap, sync::Arc};

const RESOLVE_LOCK_RETRY_LIMIT: usize = 10;
const LARGE_TXN_THRESHOLD: u64 = 16;
const RESOLVE_LOCK_BACKOFF: NoJitterBackoff =
    NoJitterBackoff::new(10, 1000, RESOLVE_LOCK_RETRY_LIMIT as u32);

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
    type LockVersion = u64;
    let mut has_live_locks = false;

    struct ResolveInfo {
        region: Region,
        primary_key: Key,
        keys: Vec<Key>,
    }

    let mut grouped: HashMap<(RegionVerId, LockVersion), ResolveInfo> = HashMap::new();

    for lock in locks {
        let expired = ts.physical - Timestamp::from_version(lock.lock_version).physical
            >= lock.lock_ttl as i64;
        if expired {
            let primary_key: Key = lock.primary_lock.into();
            let region: Region = pd_client.region_for_key(&primary_key).await?;
            let is_small_txn = lock.txn_size < LARGE_TXN_THRESHOLD;

            let resolve_info = grouped
                .entry((region.ver_id(), lock.lock_version))
                .or_insert(ResolveInfo {
                    region,
                    primary_key: primary_key.clone(),
                    keys: vec![],
                });

            if is_small_txn {
                resolve_info.keys.push(primary_key);
            } else {
                //if txn is large,keep resolve_info.keys empty to send a full resolve lock request
            }
        } else {
            has_live_locks = true;
        }
    }

    for ((_region_ver_id, lock_version), resolve_info) in grouped {
        let commit_version = requests::new_cleanup_request(resolve_info.primary_key, lock_version)
            .execute(pd_client.clone())
            .await?;

        resolve_lock_with_retry(
            &resolve_info.region,
            &resolve_info.keys,
            lock_version,
            commit_version,
            pd_client.clone(),
        )
        .await?;
    }

    Ok(!has_live_locks)
}

async fn resolve_lock_with_retry(
    region: &Region,
    keys: &Vec<Key>,
    start_version: u64,
    commit_version: u64,
    pd_client: Arc<impl PdClient>,
) -> Result<()> {
    let mut backoff = RESOLVE_LOCK_BACKOFF;

    loop {
        let region = if keys.is_empty() {
            pd_client.region_for_id(region.id()).await?
        } else {
            //Region id for key may change caused by region split
            pd_client.region_for_key(keys.get(0).unwrap()).await?
        };

        let context = match region.context() {
            Ok(context) => context,
            Err(e) => {
                // Retry if the region has no leader

                match backoff.next_delay_duration() {
                    Some(duration) => {
                        futures_timer::Delay::new(duration).await;
                        continue;
                    }
                    None => return Err(e),
                }
            }
        };

        match requests::new_resolve_lock_request(keys, context, start_version, commit_version)
            .execute(pd_client.clone())
            .await
        {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => match e.kind() {
                ErrorKind::RegionError(_) => {
                    // Retry on region error
                    match backoff.next_delay_duration() {
                        Some(duration) => {
                            futures_timer::Delay::new(duration).await;
                            continue;
                        }
                        None => return Err(e),
                    }
                }
                _ => return Err(e),
            },
        }
    }
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
        executor::block_on(resolve_lock_with_retry(
            &region1,
            &vec![key],
            1,
            2,
            client.clone(),
        ))
        .unwrap();

        // Test resolve lock over retry limit
        fail::cfg("region-error", "10*return").unwrap();
        let client = Arc::new(MockPdClient);
        let key: Key = vec![100].into();
        executor::block_on(resolve_lock_with_retry(
            &region1,
            &vec![key],
            3,
            4,
            client.clone(),
        ))
        .expect_err("should return error");
    }
}
