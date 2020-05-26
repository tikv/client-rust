use super::requests;
use crate::pd::{PdClient, Region, RegionVerId};
use crate::request::KvRequest;
use crate::{ErrorKind, Key, Result, Timestamp};

use kvproto::kvrpcpb;
use std::sync::Arc;

const RESOLVE_LOCK_RETRY_LIMIT: usize = 10;
use itertools::Itertools;

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
    let locks_len = locks.len();

    struct RichLock {
        primary_key: Key,
        region: Region,
        lock_version: u64,
        txn_size: u64,
    }

    let mut expired_locks = vec![];
    for lock in locks {
        let expired = ts.physical - Timestamp::from_version(lock.lock_version).physical
            >= lock.lock_ttl as i64;
        if expired {
            let primary_key: Key = lock.primary_lock.into();
            let region = pd_client.region_for_key(&primary_key).await?;
            expired_locks.push(RichLock {
                primary_key,
                region,
                lock_version: lock.lock_version,
                txn_size: lock.txn_size,
            })
        }
    }

    let has_live_locks = locks_len > expired_locks.len();

    let grouped: Vec<((RegionVerId, u64), Vec<RichLock>)> = expired_locks
        .into_iter()
        .group_by(|rich_lock| (rich_lock.region.ver_id().clone(), rich_lock.lock_version))
        .into_iter()
        .map(|(k, v)| (k, v.collect()))
        .collect();

    for ((_region_ver_id, lock_version), locks) in grouped {
        //Every lock with the same (region, lock_version), the txn_size is the same.
        let is_large_txn = locks.get(0).unwrap().txn_size >= 16;

        let commit_version =
            requests::new_cleanup_request(locks.get(0).unwrap().primary_key.clone(), lock_version)
                .execute(pd_client.clone())
                .await?;

        let request_keys = if is_large_txn {
            vec![]
        } else {
            locks.iter().map(|lock| &lock.primary_key).collect()
        };
        let _cleaned_region = resolve_lock_with_retry(
            &locks.get(0).unwrap().region,
            &request_keys,
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
    keys: &Vec<&Key>,
    start_version: u64,
    commit_version: u64,
    pd_client: Arc<impl PdClient>,
) -> Result<RegionVerId> {
    // TODO: Add backoff
    let mut error = None;
    for _ in 0..RESOLVE_LOCK_RETRY_LIMIT {
        let region = pd_client.region_for_id(region.id()).await?;
        let context = match region.context() {
            Ok(context) => context,
            Err(e) => {
                // Retry if the region has no leader
                error = Some(e);
                continue;
            }
        };
        match requests::new_resolve_lock_request(keys, context, start_version, commit_version)
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
        let resolved_region = executor::block_on(resolve_lock_with_retry(
            &region1,
            &vec![key],
            1,
            2,
            client.clone(),
        ))
        .unwrap();
        assert_eq!(region1.ver_id(), resolved_region);

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
