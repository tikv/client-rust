// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    backoff::{Backoff, DEFAULT_REGION_BACKOFF, OPTIMISTIC_BACKOFF},
    pd::PdClient,
    region::RegionVerId,
    request::{Collect, CollectSingle, Plan},
    store::RegionStore,
    timestamp::TimestampExt,
    transaction::{
        requests,
        requests::{
            new_check_secondary_locks_request, new_check_txn_status_request, SecondaryLocksStatus,
            TransactionStatus, TransactionStatusKind,
        },
    },
    Error, Result,
};
use fail::fail_point;
use log::debug;
use slog::Logger;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tikv_client_proto::{kvrpcpb, kvrpcpb::TxnInfo, pdpb::Timestamp};
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

#[derive(Default, Clone)]
pub struct ResolveLocksContext {
    // Record the status of each transaction.
    pub(crate) resolved: Arc<RwLock<HashMap<u64, Arc<TransactionStatus>>>>,
    pub(crate) clean_regions: Arc<RwLock<HashMap<u64, HashSet<RegionVerId>>>>,
}

#[derive(Clone, Copy, Debug)]
pub struct ResolveLocksOptions {
    pub async_commit_only: bool,
    pub batch_size: u32,
}

impl Default for ResolveLocksOptions {
    fn default() -> Self {
        Self {
            async_commit_only: false,
            batch_size: 1024,
        }
    }
}

impl ResolveLocksContext {
    pub async fn get_resolved(&self, txn_id: u64) -> Option<Arc<TransactionStatus>> {
        self.resolved.read().await.get(&txn_id).cloned()
    }

    pub async fn save_resolved(&mut self, txn_id: u64, txn_status: Arc<TransactionStatus>) {
        self.resolved.write().await.insert(txn_id, txn_status);
    }

    pub async fn is_region_cleaned(&self, txn_id: u64, region: &RegionVerId) -> bool {
        self.clean_regions
            .read()
            .await
            .get(&txn_id)
            .map(|regions| regions.contains(region))
            .unwrap_or(false)
    }

    pub async fn save_cleaned_region(&mut self, txn_id: u64, region: RegionVerId) {
        self.clean_regions
            .write()
            .await
            .entry(txn_id)
            .or_insert_with(HashSet::new)
            .insert(region);
    }
}

pub struct LockResolver {
    logger: Logger,
    ctx: ResolveLocksContext,
}

impl LockResolver {
    pub fn new(logger: Logger, ctx: ResolveLocksContext) -> Self {
        Self { logger, ctx }
    }

    /// _Cleanup_ the given locks. Returns whether all the given locks are resolved.
    ///
    /// Note: Will rollback RUNNING transactions. ONLY use in GC.
    pub async fn cleanup_locks(
        &mut self,
        store: RegionStore,
        locks: Vec<kvrpcpb::LockInfo>,
        pd_client: Arc<impl PdClient>, // TODO: make pd_client a member of LockResolver
    ) -> Result<()> {
        if locks.is_empty() {
            return Ok(());
        }

        fail_point!("before-cleanup-locks", |_| { Ok(()) });

        let region = store.region_with_leader.ver_id();

        let mut txn_infos = HashMap::new();
        for l in locks {
            let txn_id = l.get_lock_version();
            if txn_infos.contains_key(&txn_id) || self.ctx.is_region_cleaned(txn_id, &region).await
            {
                continue;
            }

            // Use currentTS = math.MaxUint64 means rollback the txn, no matter the lock is expired or not!
            let mut status = self
                .check_txn_status(
                    pd_client.clone(),
                    txn_id,
                    l.get_primary_lock().to_vec(),
                    0,
                    u64::MAX,
                    true,
                    false,
                    l.get_lock_type() == kvrpcpb::Op::PessimisticLock,
                )
                .await?;

            // If the transaction uses async commit, check_txn_status will reject rolling back the primary lock.
            // Then we need to check the secondary locks to determine the final status of the transaction.
            if let TransactionStatusKind::Locked(_, lock_info) = &status.kind {
                let secondary_status = self
                    .check_all_secondaries(
                        pd_client.clone(),
                        lock_info.get_secondaries().to_vec(),
                        txn_id,
                    )
                    .await?;
                slog_debug!(
                    self.logger,
                    "secondary status, txn_id:{}, commit_ts:{:?}, min_commit_version:{}, fallback_2pc:{}",
                    txn_id,
                    secondary_status.commit_ts.as_ref().map_or(0, |ts| ts.version()),
                    secondary_status.min_commit_ts,
                    secondary_status.fallback_2pc,
                );

                if secondary_status.fallback_2pc {
                    slog_debug!(
                        self.logger,
                        "fallback to 2pc, txn_id:{}, check_txn_status again",
                        txn_id
                    );
                    status = self
                        .check_txn_status(
                            pd_client.clone(),
                            txn_id,
                            l.get_primary_lock().to_vec(),
                            0,
                            u64::MAX,
                            true,
                            true,
                            l.get_lock_type() == kvrpcpb::Op::PessimisticLock,
                        )
                        .await?;
                } else {
                    let commit_ts = if let Some(commit_ts) = &secondary_status.commit_ts {
                        commit_ts.version()
                    } else {
                        secondary_status.min_commit_ts
                    };
                    txn_infos.insert(txn_id, commit_ts);
                    continue;
                }
            }

            match &status.kind {
                TransactionStatusKind::Locked(..) => {
                    error!(
                        self.logger,
                        "cleanup_locks fail to clean locks, this result is not expected. txn_id:{}",
                        txn_id
                    );
                    return Err(Error::ResolveLockError);
                }
                TransactionStatusKind::Committed(ts) => txn_infos.insert(txn_id, ts.version()),
                TransactionStatusKind::RolledBack => txn_infos.insert(txn_id, 0),
            };
        }

        slog_debug!(
            self.logger,
            "batch resolve locks, region:{:?}, txn:{:?}",
            store.region_with_leader.ver_id(),
            txn_infos
        );
        let mut txn_ids = Vec::with_capacity(txn_infos.len());
        let mut txn_info_vec = Vec::with_capacity(txn_infos.len());
        for (txn_id, commit_ts) in txn_infos.into_iter() {
            txn_ids.push(txn_id);
            let mut txn_info = TxnInfo::default();
            txn_info.set_txn(txn_id);
            txn_info.set_status(commit_ts);
            txn_info_vec.push(txn_info);
        }
        let cleaned_region = self
            .batch_resolve_locks(pd_client.clone(), store.clone(), txn_info_vec)
            .await?;
        for txn_id in txn_ids {
            self.ctx
                .save_cleaned_region(txn_id, cleaned_region.clone())
                .await;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn check_txn_status(
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
        if let Some(txn_status) = self.ctx.get_resolved(txn_id).await {
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
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .merge(CollectSingle)
            .extract_error()
            .post_process_default()
            .plan();
        let mut res: TransactionStatus = plan.execute().await?;

        let current = pd_client.clone().get_timestamp().await?;
        res.check_ttl(current);
        let res = Arc::new(res);
        if res.is_cacheable() {
            self.ctx.save_resolved(txn_id, res.clone()).await;
        }
        Ok(res)
    }

    async fn check_all_secondaries(
        &mut self,
        pd_client: Arc<impl PdClient>,
        keys: Vec<Vec<u8>>,
        txn_id: u64,
    ) -> Result<SecondaryLocksStatus> {
        let req = new_check_secondary_locks_request(keys, txn_id);
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), req)
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .extract_error()
            .merge(Collect)
            .plan();
        plan.execute().await
    }

    async fn batch_resolve_locks(
        &mut self,
        pd_client: Arc<impl PdClient>,
        store: RegionStore,
        txn_infos: Vec<TxnInfo>,
    ) -> Result<RegionVerId> {
        let ver_id = store.region_with_leader.ver_id();
        let request = requests::new_batch_resolve_lock_request(txn_infos.clone());
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), request)
            .single_region_with_store(store.clone())
            .await?
            .extract_error()
            .plan();
        let _ = plan.execute().await?;
        Ok(ver_id)
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
                        region_error: Some(errorpb::Error::default()).into(),
                        ..Default::default()
                    };
                    Ok(Box::new(resp) as Box<dyn Any>)
                });
                Ok(Box::<kvrpcpb::ResolveLockResponse>::default() as Box<dyn Any>)
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
