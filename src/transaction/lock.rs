// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    backoff::{Backoff, DEFAULT_REGION_BACKOFF, OPTIMISTIC_BACKOFF},
    pd::PdClient,
    region::RegionVerId,
    request::{CollectSingle, Plan},
    timestamp::TimestampExt,
    transaction::{
        requests,
        requests::{new_check_txn_status_request, TransactionStatus, TransactionStatusKind},
    },
    Error, Result,
};
use log::debug;
use slog::Logger;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tikv_client_proto::{kvrpcpb, kvrpcpb::LockInfo, pdpb::Timestamp};
use tokio::sync::RwLock;

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
    debug!("resolving locks");
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
        let region_ver_id = pd_client
            .region_for_key(&lock.primary_lock.clone().into())
            .await?
            .ver_id();
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
                let request = requests::new_cleanup_request(lock.primary_lock, lock.lock_version);
                let plan = crate::request::PlanBuilder::new(pd_client.clone(), request)
                    .resolve_lock(OPTIMISTIC_BACKOFF)
                    .retry_multi_region(DEFAULT_REGION_BACKOFF)
                    .merge(CollectSingle)
                    .post_process_default()
                    .plan();
                let commit_version = plan.execute().await?;
                commit_versions.insert(lock.lock_version, commit_version);
                commit_version
            }
        };

        let cleaned_region = resolve_lock_with_retry(
            &lock.key,
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
    #[allow(clippy::ptr_arg)] key: &Vec<u8>,
    start_version: u64,
    commit_version: u64,
    pd_client: Arc<impl PdClient>,
) -> Result<RegionVerId> {
    debug!("resolving locks with retry");
    // FIXME: Add backoff
    let mut error = None;
    for i in 0..RESOLVE_LOCK_RETRY_LIMIT {
        debug!("resolving locks: attempt {}", (i + 1));
        let store = pd_client.clone().store_for_key(key.into()).await?;
        let ver_id = store.region_with_leader.ver_id();
        let request = requests::new_resolve_lock_request(start_version, commit_version);
        // The only place where single-region is used
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), request)
            .single_region_with_store(store)
            .await?
            .resolve_lock(Backoff::no_backoff())
            .extract_error()
            .plan();
        match plan.execute().await {
            Ok(_) => {
                return Ok(ver_id);
            }
            // Retry on region error
            Err(Error::ExtractedErrors(mut errors)) => {
                // ResolveLockResponse can have at most 1 error
                match errors.pop() {
                    e @ Some(Error::RegionError(_)) => {
                        error = e;
                        continue;
                    }
                    Some(e) => return Err(e),
                    None => unreachable!(),
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(error.expect("no error is impossible"))
}

pub struct LockResolver {
    logger: Logger,
    resolved: RwLock<HashMap<u64, Arc<TransactionStatus>>>, /* TODO: use a LRU cache to limit memory usage. */
}

impl LockResolver {
    pub fn new(logger: Logger) -> Self {
        Self {
            logger,
            resolved: Default::default(),
        }
    }

    /// _Resolves_ the given locks. Returns whether all the given locks are resolved.
    ///
    /// Will rollback not finished transactions.
    /// Use in GC only !
    pub async fn batch_resolve_locks(
        &mut self,
        locks: Vec<kvrpcpb::LockInfo>,
        pd_client: Arc<impl PdClient>, // TODO: make pd_client a member of LockResolver
    ) -> Result<bool> {
        if locks.is_empty() {
            return Ok(true);
        }

        // let txn_infos = HashMap::new();
        for l in locks {
            // if txn_infos.contains_key(&l.get_lock_version()) {
            //     continue;
            // }

            // Use currentTS = math.MaxUint64 means rollback the txn, no matter the lock is expired or not!
            let status = self
                .get_txn_status(
                    pd_client.clone(),
                    l.get_lock_version(),
                    l.get_primary_lock().to_vec(),
                    0,
                    u64::MAX,
                    true,
                    false,
                    l.get_lock_type() == kvrpcpb::Op::PessimisticLock,
                )
                .await?;
            info!(
                self.logger,
                "lock status, primary:{:?}, status:{:?}",
                l.get_primary_lock(),
                status,
            );

            if let TransactionStatusKind::Locked(_, lock_info) = &status.kind {}
        }

        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn get_txn_status(
        &mut self,
        pd_client: Arc<impl PdClient>,
        txn_id: u64,
        primary: Vec<u8>,
        caller_start_ts: u64,
        current_ts: u64,
        rollback_if_not_exist: bool,
        force_sync_commit: bool,
        resolving_pessimistic_lock: bool,
    ) -> Result<Arc<TransactionStatus>> {
        if let Some(txn_status) = self.get_resolved(txn_id).await {
            return Ok(txn_status);
        }

        // CheckTxnStatus may meet the following cases:
        // 1. LOCK
        // 1.1 Lock expired -- orphan lock, fail to update TTL, crash recovery etc.
        // 1.2 Lock TTL -- active transaction holding the lock.
        // 2. NO LOCK
        // 2.1 Txn Committed
        // 2.2 Txn Rollbacked -- rollback itself, rollback by others, GC tomb etc.
        // 2.3 No lock -- pessimistic lock rollback, concurrence prewrite.
        let req = new_check_txn_status_request(
            primary,
            txn_id,
            caller_start_ts,
            current_ts,
            rollback_if_not_exist,
            force_sync_commit,
            resolving_pessimistic_lock,
        );
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), req)
            // .resolve_lock(OPTIMISTIC_BACKOFF)
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .merge(CollectSingle)
            .post_process_default()
            .plan();
        let mut res: TransactionStatus = plan.execute().await?;

        // TODO: remove this assert_eq.
        if let TransactionStatusKind::Locked(_, lock_info) = &res.kind {
            assert_eq!(lock_info.lock_version, txn_id);
        }

        let current = pd_client.clone().get_timestamp().await?;
        res.check_ttl(current);
        let res = Arc::new(res);
        if res.is_cacheable() {
            self.save_resolved(txn_id, res.clone()).await;
        }
        Ok(res)
    }

    async fn check_all_secondaries(lock: &LockInfo, status: Arc<TransactionStatus>) {}

    async fn get_resolved(&self, txn_id: u64) -> Option<Arc<TransactionStatus>> {
        self.resolved.read().await.get(&txn_id).cloned()
    }

    async fn save_resolved(&mut self, txn_id: u64, txn_status: Arc<TransactionStatus>) {
        self.resolved.write().await.insert(txn_id, txn_status);
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
    use crate::mock::{MockKvClient, MockPdClient};
    use std::any::Any;
    use tikv_client_proto::errorpb;

    #[tokio::test]
    async fn test_resolve_lock_with_retry() {
        // Test resolve lock within retry limit
        fail::cfg("region-error", "9*return").unwrap();

        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |_: &dyn Any| {
                fail::fail_point!("region-error", |_| {
                    let resp = kvrpcpb::ResolveLockResponse {
                        region_error: Some(errorpb::Error::default()),
                        ..Default::default()
                    };
                    Ok(Box::new(resp) as Box<dyn Any>)
                });
                Ok(Box::new(kvrpcpb::ResolveLockResponse::default()) as Box<dyn Any>)
            },
        )));

        let key = vec![1];
        let region1 = MockPdClient::region1();
        let resolved_region = resolve_lock_with_retry(&key, 1, 2, client.clone())
            .await
            .unwrap();
        assert_eq!(region1.ver_id(), resolved_region);

        // Test resolve lock over retry limit
        fail::cfg("region-error", "10*return").unwrap();
        let key = vec![100];
        resolve_lock_with_retry(&key, 3, 4, client)
            .await
            .expect_err("should return error");
    }
}
