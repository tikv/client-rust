// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use fail::fail_point;
use log::debug;
use log::error;
use log::warn;
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::backoff::Backoff;
use crate::backoff::DEFAULT_REGION_BACKOFF;
use crate::backoff::OPTIMISTIC_BACKOFF;
use crate::kv::HexRepr;
use crate::pd::PdClient;
use crate::Key;

use crate::proto::kvrpcpb;
use crate::proto::kvrpcpb::TxnInfo;
use crate::proto::pdpb::Timestamp;
use crate::region::RegionVerId;
use crate::request::plan::handle_region_error;
use crate::request::plan::is_grpc_error;
use crate::request::CollectSingle;
use crate::request::Keyspace;
use crate::request::Plan;
use crate::store::RegionStore;
use crate::timestamp::TimestampExt;
use crate::transaction::requests;
use crate::transaction::requests::new_check_secondary_locks_request;
use crate::transaction::requests::new_check_txn_status_request;
use crate::transaction::requests::CollectSecondaryLocks;
use crate::transaction::requests::SecondaryLocksStatus;
use crate::transaction::requests::TransactionStatus;
use crate::transaction::requests::TransactionStatusKind;
use crate::Error;
use crate::Result;

pub(crate) fn format_key_for_log(key: &[u8]) -> String {
    let prefix_len = key.len().min(16);
    format!("len={}, prefix={}", key.len(), HexRepr(&key[..prefix_len]))
}

/// Refuse to resolve SHARED locks — loudly, before any of them can be mis-handled.
///
/// The contract (`kvrpcpb.LockInfo.shared_lock_infos`) is explicit: a shared lock's
/// real holders live ONLY in `shared_lock_infos` — "DO NOT read from the wrapper
/// LockInfo", whose own `key`/`lock_version` are unset. This client does not implement
/// shared-lock resolution yet, and every partial handling is worse than none:
/// resolving the wrapper checks transaction 0; filtering on wrapper fields silently
/// drops the members; and the pessimistic-lock special cases in this resolver do not
/// know `SharedPessimisticLock`. Until support lands, an explicit error is the only
/// answer that cannot roll back a live transaction or skip a dead one.
///
/// Servers that predate shared locks never produce them, so this is a no-op there.
pub(crate) fn reject_shared_locks(locks: &[kvrpcpb::LockInfo]) -> Result<()> {
    let shared = |l: &kvrpcpb::LockInfo| {
        !l.shared_lock_infos.is_empty()
            || l.lock_type == kvrpcpb::Op::SharedLock as i32
            || l.lock_type == kvrpcpb::Op::SharedPessimisticLock as i32
    };
    if locks.iter().any(shared) {
        return Err(Error::StringError(
            "shared locks (SharedLock/SharedPessimisticLock) are not supported by this \
             client yet; refusing to resolve them — resolving the wrapper would target \
             the wrong transaction"
                .to_owned(),
        ));
    }
    Ok(())
}

/// _Resolves_ the given locks. Returns locks still live. When there is no live locks, all the given locks are resolved.
///
/// If a key has a lock, the latest status of the key is unknown. We need to "resolve" the lock,
/// which means the key is finally either committed or rolled back, before we read the value of
/// the key. We first use `CheckTxnStatus` to get the transaction's final status (committed or
/// rolled back), then use `ResolveLock` to resolve the remaining locks in the transaction.
///
/// An expired async-commit lock needs an extra step in between: TiKV refuses to roll it back
/// via `CheckTxnStatus`, so the transaction's final status is first recovered from all of its
/// secondary locks (`CheckSecondaryLocks`), and then every lock of the transaction is resolved.
pub async fn resolve_locks(
    locks: Vec<kvrpcpb::LockInfo>,
    timestamp: Timestamp,
    pd_client: Arc<impl PdClient>,
    keyspace: Keyspace,
) -> Result<Vec<kvrpcpb::LockInfo> /* live_locks */> {
    debug!("resolving locks");
    reject_shared_locks(&locks)?;
    let ts = pd_client.clone().get_timestamp().await?;
    let caller_start_ts = timestamp.version();
    let current_ts = ts.version();

    let mut live_locks = Vec::new();
    // Final transaction statuses are cached in the resolver's context, so a transaction met
    // through several locks is queried (or recovered) only once.
    let mut lock_resolver = LockResolver::new(ResolveLocksContext::default());
    // Regions already swept by ResolveLock, per transaction.
    let mut clean_regions: HashMap<u64, HashSet<RegionVerId>> = HashMap::new();
    // We must check txn status for *all* locks, not only TTL-expired ones.
    //
    // TTL only indicates whether a lock is *possibly* orphaned; it does not mean the transaction
    // is still running. A transaction may already be committed/rolled back while its locks are
    // still visible (e.g. cleanup/resolve hasn't finished, retries after region errors, etc.).
    // If we only resolve TTL-expired locks, we can unnecessarily sleep/backoff until TTL even
    // though `CheckTxnStatus` would already report `Committed`/`RolledBack`.
    //
    // This matches the client-go `LockResolver.ResolveLocksWithOpts` flow: query txn status for
    // each encountered lock, then resolve immediately when the status is final.
    for lock in locks {
        let (commit_version, keys) = match lock_resolver
            .action_for_lock(
                &lock,
                caller_start_ts,
                current_ts,
                pd_client.clone(),
                keyspace,
            )
            .await?
        {
            LockAction::Resolve {
                commit_version,
                keys,
            } => (commit_version, keys),
            LockAction::Live(primary_lock) => {
                live_locks.push(primary_lock);
                continue;
            }
            LockAction::Done => continue,
        };
        let regions = clean_regions.entry(lock.lock_version).or_default();
        for key in keys.iter().chain([&lock.key]) {
            resolve_lock_with_retry(
                key.into(),
                lock.lock_version,
                commit_version,
                lock.is_txn_file,
                pd_client.clone(),
                keyspace,
                OPTIMISTIC_BACKOFF,
                regions,
            )
            .await?;
        }
    }
    Ok(live_locks)
}

/// What `resolve_locks` has to do about one encountered lock.
enum LockAction {
    /// The transaction's fate is known: resolve its locks with `commit_version` (zero rolls
    /// back) in the regions of `keys` and of the encountered lock itself.
    Resolve {
        commit_version: u64,
        keys: Vec<Vec<u8>>,
    },
    /// The transaction is still alive; its primary lock is reported back to the caller.
    Live(kvrpcpb::LockInfo),
    /// The lock has been dealt with on its own and nothing else needs resolving.
    Done,
}

impl LockAction {
    /// The action for a transaction whose `CheckTxnStatus` answer is taken as is.
    fn from_status(status: &TransactionStatus) -> Self {
        match &status.kind {
            TransactionStatusKind::Committed(ts) => LockAction::Resolve {
                commit_version: ts.version(),
                keys: Vec::new(),
            },
            TransactionStatusKind::RolledBack => LockAction::Resolve {
                commit_version: 0,
                keys: Vec::new(),
            },
            TransactionStatusKind::Locked(_, primary_lock) => {
                LockAction::Live(primary_lock.clone())
            }
        }
    }
}

/// Roll back one pessimistic lock, leaving the rest of its transaction alone.
async fn pessimistic_rollback_lock(
    lock: &kvrpcpb::LockInfo,
    pd_client: Arc<impl PdClient>,
    keyspace: Keyspace,
) -> Result<()> {
    let for_update_ts = if lock.lock_for_update_ts == 0 {
        u64::MAX
    } else {
        lock.lock_for_update_ts
    };
    let req = requests::new_pessimistic_rollback_request(
        vec![lock.key.clone()],
        lock.lock_version,
        for_update_ts,
    );
    let plan = crate::request::PlanBuilder::new(pd_client, keyspace, req)
        .retry_multi_region(DEFAULT_REGION_BACKOFF)
        .extract_error()
        .plan();
    plan.execute().await?;
    Ok(())
}

/// Resolve every lock of transaction `start_version` in the region holding `key`, unless that
/// region is already in `clean_regions`. ResolveLock sweeps the whole region, so one successful
/// request per region is enough no matter how many of the transaction's keys point to it.
#[allow(clippy::too_many_arguments)]
async fn resolve_lock_with_retry(
    key: &Key,
    start_version: u64,
    commit_version: u64,
    is_txn_file: bool,
    pd_client: Arc<impl PdClient>,
    keyspace: Keyspace,
    mut backoff: Backoff,
    clean_regions: &mut HashSet<RegionVerId>,
) -> Result<()> {
    debug!("resolving locks with retry");
    let mut attempt = 0;
    loop {
        attempt += 1;
        debug!("resolving locks: attempt {}", attempt);
        let store = pd_client.clone().store_for_key(key).await?;
        let ver_id = store.region_with_leader.ver_id();
        if clean_regions.contains(&ver_id) {
            return Ok(());
        }
        let request =
            requests::new_resolve_lock_request(start_version, commit_version, is_txn_file);
        let plan_builder =
            match crate::request::PlanBuilder::new(pd_client.clone(), keyspace, request)
                .single_region_with_store(store.clone())
                .await
            {
                Ok(plan_builder) => plan_builder,
                Err(Error::LeaderNotFound { region }) => {
                    pd_client.invalidate_region_cache(region.clone()).await;
                    match backoff.next_delay_duration() {
                        Some(duration) => {
                            sleep(duration).await;
                            continue;
                        }
                        None => return Err(Error::LeaderNotFound { region }),
                    }
                }
                Err(err) => return Err(err),
            };
        let plan = plan_builder.extract_error().plan();
        match plan.execute().await {
            Ok(_) => {
                clean_regions.insert(ver_id);
                return Ok(());
            }
            // Retry on region error
            Err(Error::ExtractedErrors(mut errors)) => {
                // ResolveLockResponse can have at most 1 error
                match errors.pop() {
                    Some(Error::RegionError(e)) => match backoff.next_delay_duration() {
                        Some(duration) => {
                            let region_error_resolved =
                                handle_region_error(pd_client.clone(), *e, store.clone()).await?;
                            if !region_error_resolved {
                                sleep(duration).await;
                            }
                            continue;
                        }
                        None => return Err(Error::RegionError(e)),
                    },
                    Some(Error::KeyError(key_err)) => {
                        // Keyspace is not truncated here because we need full key info for logging.
                        error!(
                            "resolve_lock error, unexpected resolve err: {:?}, lock: {{key: {}, start_version: {}, commit_version: {}, is_txn_file: {}}}",
                            key_err,
                            format_key_for_log(key),
                            start_version,
                            commit_version,
                            is_txn_file,
                        );
                        return Err(Error::KeyError(key_err));
                    }
                    Some(e) => return Err(e),
                    None => unreachable!(),
                }
            }
            Err(e) if is_grpc_error(&e) => match backoff.next_delay_duration() {
                Some(duration) => {
                    pd_client.invalidate_region_cache(ver_id.clone()).await;
                    if let Ok(store_id) = store.region_with_leader.get_store_id() {
                        pd_client.invalidate_store_cache(store_id).await;
                    }
                    sleep(duration).await;
                    continue;
                }
                None => return Err(e),
            },
            Err(e) => return Err(e),
        }
    }
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
    ctx: ResolveLocksContext,
}

impl LockResolver {
    pub fn new(ctx: ResolveLocksContext) -> Self {
        Self { ctx }
    }

    /// Decide what `resolve_locks` has to do about `lock`: query its transaction's status and,
    /// for an expired async-commit transaction, recover that status from the secondary locks.
    /// This is the per-lock decision of client-go's `LockResolver.resolveLocks`.
    async fn action_for_lock(
        &mut self,
        lock: &kvrpcpb::LockInfo,
        caller_start_ts: u64,
        current_ts: u64,
        pd_client: Arc<impl PdClient>,
        keyspace: Keyspace,
    ) -> Result<LockAction> {
        let status = match self
            .get_txn_status_from_lock(
                OPTIMISTIC_BACKOFF,
                lock,
                caller_start_ts,
                current_ts,
                false,
                pd_client.clone(),
                keyspace,
            )
            .await
        {
            Ok(status) => status,
            Err(Error::KeyError(key_err))
                if key_err.primary_mismatch.is_some()
                    && lock.lock_type == kvrpcpb::Op::PessimisticLock as i32 =>
            {
                // The encountered pessimistic lock points at a stale primary: the transaction
                // changed its primary after writing this lock (pingcap/tidb#42937). Roll back
                // only this stale lock — the transaction itself may still be alive, and a
                // region-wide ResolveLock could roll back its other, legitimate locks.
                pessimistic_rollback_lock(lock, pd_client, keyspace).await?;
                return Ok(LockAction::Done);
            }
            Err(err) => return Err(err),
        };

        let primary_lock = match &status.kind {
            TransactionStatusKind::Locked(_, primary_lock)
                if status.is_expired && primary_lock.use_async_commit =>
            {
                primary_lock
            }
            _ => return Ok(LockAction::from_status(&status)),
        };

        // TiKV will not roll back an expired async-commit primary: the transaction's decision
        // has to be recovered from its secondaries instead.
        let secondary_status = self
            .check_all_secondaries(
                pd_client.clone(),
                keyspace,
                primary_lock.secondaries.clone(),
                lock.lock_version,
            )
            .await?;
        if secondary_status.fallback_2pc {
            // A secondary fell back to plain 2PC, so the primary decides after all. Ask again,
            // this time letting TiKV roll back the expired primary.
            let status = self
                .get_txn_status_from_lock(
                    OPTIMISTIC_BACKOFF,
                    lock,
                    caller_start_ts,
                    current_ts,
                    true,
                    pd_client,
                    keyspace,
                )
                .await?;
            return Ok(LockAction::from_status(&status));
        }

        let commit_version =
            secondary_status.resolved_commit_version(primary_lock.min_commit_ts)?;
        // The recovered status is final: cache it so further locks of this transaction skip
        // the recovery, as client-go's `resolveAsyncCommitLock` does with `saveResolved`.
        self.ctx
            .save_resolved(
                lock.lock_version,
                Arc::new(TransactionStatus::from_commit_version(commit_version)),
            )
            .await;
        // Every lock of the transaction is resolved, not only the encountered one.
        let mut keys = primary_lock.secondaries.clone();
        keys.push(primary_lock.key.clone());
        Ok(LockAction::Resolve {
            commit_version,
            keys,
        })
    }

    /// _Cleanup_ the given locks. Returns whether all the given locks are resolved.
    ///
    /// Note: Will rollback RUNNING transactions. ONLY use in GC.
    pub async fn cleanup_locks(
        &mut self,
        store: RegionStore,
        locks: Vec<kvrpcpb::LockInfo>,
        pd_client: Arc<impl PdClient>, // TODO: make pd_client a member of LockResolver
        keyspace: Keyspace,
    ) -> Result<()> {
        // Defense in depth: CleanupLocks::execute refuses these before its filters,
        // but this entry point is public within the crate.
        reject_shared_locks(&locks)?;
        if locks.is_empty() {
            return Ok(());
        }

        fail_point!("before-cleanup-locks", |_| { Ok(()) });

        let region = store.region_with_leader.ver_id();

        let mut txn_infos = HashMap::new();
        for l in locks {
            let txn_id = l.lock_version;
            if txn_infos.contains_key(&txn_id) || self.ctx.is_region_cleaned(txn_id, &region).await
            {
                continue;
            }

            // Use currentTS = math.MaxUint64 means rollback the txn, no matter the lock is expired or not!
            let mut status = self
                .check_txn_status(
                    pd_client.clone(),
                    keyspace,
                    txn_id,
                    l.primary_lock.clone(),
                    0,
                    u64::MAX,
                    true,
                    false,
                    l.lock_type == kvrpcpb::Op::PessimisticLock as i32,
                    l.is_txn_file,
                )
                .await?;

            // If the transaction uses async commit, check_txn_status will reject rolling back the primary lock.
            // Then we need to check the secondary locks to determine the final status of the transaction.
            if let TransactionStatusKind::Locked(_, lock_info) = &status.kind {
                let secondary_status = self
                    .check_all_secondaries(
                        pd_client.clone(),
                        keyspace,
                        lock_info.secondaries.clone(),
                        txn_id,
                    )
                    .await?;
                debug!(
                    "secondary status, txn_id:{}, commit_ts:{:?}, min_commit_version:{}, fallback_2pc:{}",
                    txn_id,
                    secondary_status.commit_ts,
                    secondary_status.min_commit_ts,
                    secondary_status.fallback_2pc,
                );

                if secondary_status.fallback_2pc {
                    debug!("fallback to 2pc, txn_id:{}, check_txn_status again", txn_id);
                    status = self
                        .check_txn_status(
                            pd_client.clone(),
                            keyspace,
                            txn_id,
                            l.primary_lock,
                            0,
                            u64::MAX,
                            true,
                            true,
                            l.lock_type == kvrpcpb::Op::PessimisticLock as i32,
                            l.is_txn_file,
                        )
                        .await?;
                } else {
                    let commit_ts =
                        secondary_status.resolved_commit_version(lock_info.min_commit_ts)?;
                    txn_infos.insert(txn_id, (commit_ts, l.is_txn_file));
                    continue;
                }
            }

            match &status.kind {
                TransactionStatusKind::Locked(_, lock_info) => {
                    error!(
                        "cleanup_locks fail to clean locks, this result is not expected. txn_id:{}",
                        txn_id
                    );
                    return Err(Error::ResolveLockError(vec![lock_info.clone()]));
                }
                TransactionStatusKind::Committed(ts) => {
                    txn_infos.insert(txn_id, (ts.version(), l.is_txn_file))
                }
                TransactionStatusKind::RolledBack => txn_infos.insert(txn_id, (0, l.is_txn_file)),
            };
        }

        debug!(
            "batch resolve locks, region:{:?}, txn:{:?}",
            store.region_with_leader.ver_id(),
            txn_infos
        );
        let mut txn_ids = Vec::with_capacity(txn_infos.len());
        let mut txn_info_vec = Vec::with_capacity(txn_infos.len());
        for (txn_id, (commit_ts, is_txn_file)) in txn_infos.into_iter() {
            txn_ids.push(txn_id);
            let mut txn_info = TxnInfo::default();
            txn_info.txn = txn_id;
            txn_info.status = commit_ts;
            txn_info.is_txn_file = is_txn_file;
            txn_info_vec.push(txn_info);
        }
        let cleaned_region = self
            .batch_resolve_locks(pd_client.clone(), keyspace, store.clone(), txn_info_vec)
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
        keyspace: Keyspace,
        txn_id: u64,
        primary: Vec<u8>,
        caller_start_ts: u64,
        current_ts: u64,
        rollback_if_not_exist: bool,
        force_sync_commit: bool,
        resolving_pessimistic_lock: bool,
        is_txn_file: bool,
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
            is_txn_file,
        );
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), keyspace, req)
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .merge(CollectSingle)
            .extract_error()
            .post_process_default()
            .plan();
        let mut status: TransactionStatus = match plan.execute().await {
            Ok(status) => status,
            Err(Error::ExtractedErrors(mut errors)) | Err(Error::MultipleKeyErrors(mut errors)) => {
                match errors.pop() {
                    Some(Error::KeyError(key_err)) => {
                        if let Some(txn_not_found) = key_err.txn_not_found {
                            return Err(Error::TxnNotFound(txn_not_found));
                        }
                        // A PrimaryMismatch error propagates to `resolve_locks`, which rolls
                        // back the stale pessimistic lock it was reported for.
                        return Err(Error::KeyError(key_err));
                    }
                    Some(err) => return Err(err),
                    None => unreachable!(),
                }
            }
            Err(err) => return Err(err),
        };

        let current = pd_client.clone().get_timestamp().await?;
        status.check_ttl(current);
        let res = Arc::new(status);
        if res.is_cacheable() {
            self.ctx.save_resolved(txn_id, res.clone()).await;
        }
        Ok(res)
    }

    async fn check_all_secondaries(
        &mut self,
        pd_client: Arc<impl PdClient>,
        keyspace: Keyspace,
        keys: Vec<Vec<u8>>,
        txn_id: u64,
    ) -> Result<SecondaryLocksStatus> {
        let req = new_check_secondary_locks_request(keys, txn_id);
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), keyspace, req)
            .preserve_shard()
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .extract_error()
            .merge(CollectSecondaryLocks {
                start_version: txn_id,
            })
            .plan();
        plan.execute().await
    }

    async fn batch_resolve_locks(
        &mut self,
        pd_client: Arc<impl PdClient>,
        keyspace: Keyspace,
        store: RegionStore,
        txn_infos: Vec<TxnInfo>,
    ) -> Result<RegionVerId> {
        let ver_id = store.region_with_leader.ver_id();
        let request = requests::new_batch_resolve_lock_request(txn_infos.clone());
        let plan = crate::request::PlanBuilder::new(pd_client.clone(), keyspace, request)
            .single_region_with_store(store.clone())
            .await?
            .extract_error()
            .plan();
        let _ = plan.execute().await?;
        Ok(ver_id)
    }

    #[allow(clippy::too_many_arguments)]
    async fn get_txn_status_from_lock(
        &mut self,
        mut backoff: Backoff,
        lock: &kvrpcpb::LockInfo,
        caller_start_ts: u64,
        current_ts: u64,
        force_sync_commit: bool,
        pd_client: Arc<impl PdClient>,
        keyspace: Keyspace,
    ) -> Result<Arc<TransactionStatus>> {
        let current_ts = if lock.lock_ttl == 0 {
            // NOTE: lock_ttl = 0 is a special protocol!!!
            // When the pessimistic txn prewrite meets locks of a txn, it should resolve the lock **unconditionally**.
            // In this case, TiKV use lock TTL = 0 to notify client, and client should resolve the lock!
            // Set current_ts to max uint64 to make the lock expired.
            u64::MAX
        } else {
            current_ts
        };

        let mut rollback_if_not_exist = false;
        loop {
            match self
                .check_txn_status(
                    pd_client.clone(),
                    keyspace,
                    lock.lock_version,
                    lock.primary_lock.clone(),
                    caller_start_ts,
                    current_ts,
                    rollback_if_not_exist,
                    force_sync_commit,
                    lock.lock_type == kvrpcpb::Op::PessimisticLock as i32,
                    lock.is_txn_file,
                )
                .await
            {
                Ok(status) => return Ok(status),
                Err(Error::TxnNotFound(txn_not_found)) => {
                    let current = pd_client.clone().get_timestamp().await?;
                    if lock_until_expired_ms(lock.lock_version, lock.lock_ttl, current) <= 0 {
                        warn!(
                            "lock txn not found, lock has expired, lock {:?}, caller_start_ts {}, current_ts {}",
                            lock, caller_start_ts, current_ts
                        );
                        rollback_if_not_exist = true;
                        continue;
                    } else if lock.lock_type == kvrpcpb::Op::PessimisticLock as i32 {
                        let status = TransactionStatus {
                            kind: TransactionStatusKind::Locked(lock.lock_ttl, lock.clone()),
                            action: kvrpcpb::Action::NoAction,
                            is_expired: false,
                        };
                        return Ok(Arc::new(status));
                    }

                    if let Some(duration) = backoff.next_delay_duration() {
                        sleep(duration).await;
                        continue;
                    }
                    return Err(Error::TxnNotFound(txn_not_found));
                }
                Err(err) => return Err(err),
            }
        }
    }
}

pub trait HasLocks {
    fn take_locks(&mut self) -> Vec<kvrpcpb::LockInfo> {
        Vec::new()
    }
}

// Return duration in milliseconds until lock expired.
// If the lock has expired, return a negative value.
pub fn lock_until_expired_ms(lock_version: u64, ttl: u64, current: Timestamp) -> i64 {
    Timestamp::from_version(lock_version).physical + ttl as i64 - current.physical
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use fail::FailScenario;
    use serial_test::serial;

    use super::*;
    use crate::mock::MockKvClient;
    use crate::mock::MockPdClient;
    use crate::proto::errorpb;
    use crate::proto::metapb;
    use crate::region::RegionWithLeader;
    use crate::request::EncodeKeyspace;
    use crate::request::KeyMode;
    use crate::Key;

    /// A transaction ID whose lock is always expired under the mock PD clock:
    /// `Timestamp::from_version` casts the version to `i64` before shifting, so
    /// `u64::MAX` yields a physical time of -1 (production timestamp arithmetic, not
    /// mock behavior), while `MockPdClient::get_timestamp` stands at physical 0 —
    /// any positive TTL has therefore already lapsed.
    const EXPIRED_TXN_VERSION: u64 = u64::MAX;

    /// The async-commit recovery tests place these on both sides of the mock region
    /// boundary (`MockPdClient::region1` ends at `[10]`), so a full recovery must
    /// resolve two regions.
    const PRIMARY_KEY: &[u8] = &[1]; // mock region 1
    const SECONDARY_KEY: &[u8] = &[11]; // mock region 2

    fn encoded(key: &[u8], keyspace: Keyspace) -> Vec<u8> {
        Key::from(key.to_vec())
            .encode_keyspace(keyspace, KeyMode::Txn)
            .into()
    }

    /// The primary lock of an expired async-commit transaction — the shape returned
    /// by `CheckTxnStatus` and encountered by readers.
    fn async_commit_primary_lock(
        primary_key: &[u8],
        min_commit_ts: u64,
        secondaries: Vec<Vec<u8>>,
    ) -> kvrpcpb::LockInfo {
        kvrpcpb::LockInfo {
            key: primary_key.to_vec(),
            primary_lock: primary_key.to_vec(),
            lock_version: EXPIRED_TXN_VERSION,
            lock_ttl: 1,
            min_commit_ts,
            use_async_commit: true,
            secondaries,
            ..Default::default()
        }
    }

    /// A `CheckTxnStatusResponse` reporting the transaction still locked by `primary`.
    fn still_locked_response(primary: kvrpcpb::LockInfo) -> kvrpcpb::CheckTxnStatusResponse {
        kvrpcpb::CheckTxnStatusResponse {
            lock_ttl: 1,
            lock_info: Some(primary),
            action: kvrpcpb::Action::NoAction as i32,
            ..Default::default()
        }
    }

    /// A still-live async-commit secondary lock.
    fn async_commit_secondary_lock(key: &[u8], min_commit_ts: u64) -> kvrpcpb::LockInfo {
        kvrpcpb::LockInfo {
            key: key.to_vec(),
            lock_version: EXPIRED_TXN_VERSION,
            min_commit_ts,
            use_async_commit: true,
            ..Default::default()
        }
    }

    /// A secondary lock that fell back from async commit to plain 2PC.
    fn fallback_2pc_secondary_lock(key: &[u8]) -> kvrpcpb::LockInfo {
        kvrpcpb::LockInfo {
            key: key.to_vec(),
            lock_version: EXPIRED_TXN_VERSION,
            use_async_commit: false,
            ..Default::default()
        }
    }

    #[test]
    fn shared_locks_are_refused_never_misresolved() {
        let plain = kvrpcpb::LockInfo {
            key: b"k1".to_vec(),
            lock_version: 7,
            ..Default::default()
        };
        assert!(reject_shared_locks(std::slice::from_ref(&plain)).is_ok());

        // A wrapper: key/lock_version deliberately unset per the contract — resolving
        // it would check transaction 0. Must be refused, not resolved or filtered.
        let wrapper = kvrpcpb::LockInfo {
            shared_lock_infos: vec![kvrpcpb::LockInfo {
                key: b"k2".to_vec(),
                lock_version: 8,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(reject_shared_locks(&[plain.clone(), wrapper]).is_err());

        // Also refused when only the op marks it shared (empty member list).
        let by_op = kvrpcpb::LockInfo {
            lock_type: kvrpcpb::Op::SharedPessimisticLock as i32,
            ..Default::default()
        };
        assert!(reject_shared_locks(&[by_op]).is_err());
    }

    #[rstest::rstest]
    #[case(Keyspace::Disable)]
    #[case(Keyspace::Enable { keyspace_id: 0 })]
    #[tokio::test]
    #[serial]
    async fn test_resolve_lock_with_retry(#[case] keyspace: Keyspace) {
        let _scenario = FailScenario::setup();

        const MAX_REGION_ERROR_RETRIES: u32 = 10;
        let backoff = Backoff::no_jitter_backoff(0, 0, MAX_REGION_ERROR_RETRIES);

        // Test resolve lock within retry limit
        fail::cfg(
            "region-error",
            &format!("{}*return", MAX_REGION_ERROR_RETRIES),
        )
        .unwrap();

        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |_: &dyn Any| {
                fail::fail_point!("region-error", |_| {
                    let resp = kvrpcpb::ResolveLockResponse {
                        region_error: Some(errorpb::Error::default()),
                        ..Default::default()
                    };
                    Ok(Box::new(resp) as Box<dyn Any>)
                });
                Ok(Box::<kvrpcpb::ResolveLockResponse>::default() as Box<dyn Any>)
            },
        )));

        let key = vec![1];
        let region1 = MockPdClient::region1();
        let mut clean_regions = HashSet::new();
        resolve_lock_with_retry(
            (&key).into(),
            1,
            2,
            false,
            client.clone(),
            keyspace,
            backoff.clone(),
            &mut clean_regions,
        )
        .await
        .unwrap();
        assert_eq!(clean_regions, HashSet::from([region1.ver_id()]));

        // Test resolve lock over retry limit
        fail::cfg(
            "region-error",
            &format!("{}*return", MAX_REGION_ERROR_RETRIES + 1),
        )
        .unwrap();
        let key = vec![100];
        resolve_lock_with_retry(
            (&key).into(),
            3,
            4,
            false,
            client,
            keyspace,
            backoff,
            &mut clean_regions,
        )
        .await
        .expect_err("should return error");
    }

    #[tokio::test]
    #[serial]
    async fn test_resolve_locks_resolves_committed_even_if_ttl_not_expired() {
        let check_txn_status_count = Arc::new(AtomicUsize::new(0));
        let resolve_lock_count = Arc::new(AtomicUsize::new(0));

        let check_txn_status_count_captured = check_txn_status_count.clone();
        let resolve_lock_count_captured = resolve_lock_count.clone();
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if req.is::<kvrpcpb::CheckTxnStatusRequest>() {
                    check_txn_status_count_captured.fetch_add(1, Ordering::SeqCst);
                    let resp = kvrpcpb::CheckTxnStatusResponse {
                        commit_version: 2,
                        action: kvrpcpb::Action::NoAction as i32,
                        ..Default::default()
                    };
                    return Ok(Box::new(resp) as Box<dyn Any>);
                }
                if req.is::<kvrpcpb::ResolveLockRequest>() {
                    resolve_lock_count_captured.fetch_add(1, Ordering::SeqCst);
                    return Ok(Box::<kvrpcpb::ResolveLockResponse>::default() as Box<dyn Any>);
                }
                panic!("unexpected request type: {:?}", req.type_id());
            },
        )));

        let mut lock = kvrpcpb::LockInfo::default();
        lock.key = vec![1];
        lock.primary_lock = vec![1];
        lock.lock_version = 1;
        lock.lock_ttl = 100; // not expired under MockPdClient's Timestamp::default()

        let live_locks = resolve_locks(vec![lock], Timestamp::default(), client, Keyspace::Disable)
            .await
            .unwrap();

        assert!(live_locks.is_empty());
        assert_eq!(check_txn_status_count.load(Ordering::SeqCst), 1);
        assert_eq!(resolve_lock_count.load(Ordering::SeqCst), 1);
    }

    #[rstest::rstest]
    // With `Keyspace::Enable` every key gains the keyspace prefix, which places them
    // all in mock region 2 — the recovery then resolves one region instead of two.
    #[case(Keyspace::Disable, 2, &[PRIMARY_KEY])]
    #[case(Keyspace::Enable { keyspace_id: 0 }, 1, &[PRIMARY_KEY])]
    #[case(Keyspace::Disable, 2, &[SECONDARY_KEY, PRIMARY_KEY])]
    #[case(Keyspace::Enable { keyspace_id: 0 }, 1, &[SECONDARY_KEY, PRIMARY_KEY])]
    #[tokio::test]
    #[serial]
    async fn test_resolve_locks_recovers_expired_async_commit(
        #[case] keyspace: Keyspace,
        #[case] expected_resolved_regions: usize,
        #[case] encountered_keys: &[&[u8]],
    ) {
        let primary_key = encoded(PRIMARY_KEY, keyspace);
        let secondary_key = encoded(SECONDARY_KEY, keyspace);

        let check_secondary_count = Arc::new(AtomicUsize::new(0));
        let resolve_lock_count = Arc::new(AtomicUsize::new(0));
        let resolved_commit_version = Arc::new(AtomicU64::new(0));

        let check_secondary_count_captured = check_secondary_count.clone();
        let resolve_lock_count_captured = resolve_lock_count.clone();
        let resolved_commit_version_captured = resolved_commit_version.clone();
        let primary_key_captured = primary_key.clone();
        let secondary_key_captured = secondary_key.clone();
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if let Some(req) = req.downcast_ref::<kvrpcpb::CheckTxnStatusRequest>() {
                    assert_eq!(req.primary_key, primary_key_captured);
                    return Ok(Box::new(still_locked_response(async_commit_primary_lock(
                        &primary_key_captured,
                        44,
                        vec![secondary_key_captured.clone()],
                    ))) as Box<dyn Any>);
                }
                if let Some(req) = req.downcast_ref::<kvrpcpb::CheckSecondaryLocksRequest>() {
                    check_secondary_count_captured.fetch_add(1, Ordering::SeqCst);
                    // The secondaries listed in the primary lock are already encoded and
                    // must be sent verbatim — re-encoding them would corrupt the keys.
                    assert_eq!(req.keys, vec![secondary_key_captured.clone()]);
                    let resp = kvrpcpb::CheckSecondaryLocksResponse {
                        locks: vec![async_commit_secondary_lock(&secondary_key_captured, 43)],
                        ..Default::default()
                    };
                    return Ok(Box::new(resp) as Box<dyn Any>);
                }
                if let Some(req) = req.downcast_ref::<kvrpcpb::ResolveLockRequest>() {
                    resolve_lock_count_captured.fetch_add(1, Ordering::SeqCst);
                    resolved_commit_version_captured.store(req.commit_version, Ordering::SeqCst);
                    return Ok(Box::<kvrpcpb::ResolveLockResponse>::default() as Box<dyn Any>);
                }
                panic!("unexpected request type: {:?}", req.type_id());
            },
        )));

        let locks = encountered_keys
            .iter()
            .map(|key| kvrpcpb::LockInfo {
                key: encoded(key, keyspace),
                primary_lock: primary_key.clone(),
                lock_version: EXPIRED_TXN_VERSION,
                lock_ttl: 1,
                use_async_commit: true,
                ..Default::default()
            })
            .collect();

        let live_locks = resolve_locks(locks, Timestamp::default(), client, keyspace)
            .await
            .unwrap();

        assert!(live_locks.is_empty());
        assert_eq!(check_secondary_count.load(Ordering::SeqCst), 1);
        // Every region the transaction wrote to must be resolved, exactly once each.
        assert_eq!(
            resolve_lock_count.load(Ordering::SeqCst),
            expected_resolved_regions
        );
        // The secondary's min_commit_ts (43) is below the primary's (44): the recovered
        // commit version must be the maximum across ALL locks, i.e. the primary's.
        assert_eq!(resolved_commit_version.load(Ordering::SeqCst), 44);
    }

    #[tokio::test]
    #[serial]
    async fn test_resolve_locks_recovers_missing_async_secondary_as_rollback() {
        let check_secondary_count = Arc::new(AtomicUsize::new(0));
        let resolve_lock_count = Arc::new(AtomicUsize::new(0));
        let resolved_commit_version = Arc::new(AtomicU64::new(u64::MAX));

        let check_secondary_count_captured = check_secondary_count.clone();
        let resolve_lock_count_captured = resolve_lock_count.clone();
        let resolved_commit_version_captured = resolved_commit_version.clone();
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if req.is::<kvrpcpb::CheckTxnStatusRequest>() {
                    return Ok(Box::new(still_locked_response(async_commit_primary_lock(
                        PRIMARY_KEY,
                        42,
                        vec![SECONDARY_KEY.to_vec()],
                    ))) as Box<dyn Any>);
                }
                if req.is::<kvrpcpb::CheckSecondaryLocksRequest>() {
                    check_secondary_count_captured.fetch_add(1, Ordering::SeqCst);
                    // The requested secondary is absent and commit_ts is zero: TiKV has
                    // established a rollback tombstone, so the transaction must roll back.
                    return Ok(
                        Box::<kvrpcpb::CheckSecondaryLocksResponse>::default() as Box<dyn Any>
                    );
                }
                if let Some(req) = req.downcast_ref::<kvrpcpb::ResolveLockRequest>() {
                    resolve_lock_count_captured.fetch_add(1, Ordering::SeqCst);
                    resolved_commit_version_captured.store(req.commit_version, Ordering::SeqCst);
                    return Ok(Box::<kvrpcpb::ResolveLockResponse>::default() as Box<dyn Any>);
                }
                panic!("unexpected request type: {:?}", req.type_id());
            },
        )));

        let lock = async_commit_primary_lock(PRIMARY_KEY, 42, vec![SECONDARY_KEY.to_vec()]);

        let live_locks = resolve_locks(vec![lock], Timestamp::default(), client, Keyspace::Disable)
            .await
            .unwrap();

        assert!(live_locks.is_empty());
        assert_eq!(check_secondary_count.load(Ordering::SeqCst), 1);
        // The rollback must reach every region the transaction wrote to: the
        // secondary's region and the primary's.
        assert_eq!(resolve_lock_count.load(Ordering::SeqCst), 2);
        assert_eq!(resolved_commit_version.load(Ordering::SeqCst), 0);
    }

    #[rstest::rstest]
    #[case(55, 0)]
    #[case(0, 0)]
    #[case(0, 1000)]
    #[tokio::test]
    #[serial]
    async fn test_resolve_locks_falls_back_to_2pc_with_real_current_ts(
        #[case] commit_version: u64,
        #[case] lock_ttl: u64,
    ) {
        let check_txn_status_count = Arc::new(AtomicUsize::new(0));
        let force_sync_count = Arc::new(AtomicUsize::new(0));
        let resolve_lock_count = Arc::new(AtomicUsize::new(0));
        let resolved_commit_version = Arc::new(AtomicU64::new(0));

        let check_txn_status_count_captured = check_txn_status_count.clone();
        let force_sync_count_captured = force_sync_count.clone();
        let resolve_lock_count_captured = resolve_lock_count.clone();
        let resolved_commit_version_captured = resolved_commit_version.clone();
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if let Some(req) = req.downcast_ref::<kvrpcpb::CheckTxnStatusRequest>() {
                    check_txn_status_count_captured.fetch_add(1, Ordering::SeqCst);
                    if req.force_sync_commit {
                        force_sync_count_captured.fetch_add(1, Ordering::SeqCst);
                        assert_ne!(req.current_ts, u64::MAX);
                        return Ok(Box::new(kvrpcpb::CheckTxnStatusResponse {
                            commit_version,
                            lock_ttl,
                            lock_info: (lock_ttl != 0).then(|| kvrpcpb::LockInfo {
                                key: PRIMARY_KEY.to_vec(),
                                primary_lock: PRIMARY_KEY.to_vec(),
                                lock_version: EXPIRED_TXN_VERSION,
                                lock_ttl,
                                ..Default::default()
                            }),
                            action: kvrpcpb::Action::NoAction as i32,
                            ..Default::default()
                        }) as Box<dyn Any>);
                    }
                    return Ok(Box::new(still_locked_response(async_commit_primary_lock(
                        PRIMARY_KEY,
                        42,
                        vec![SECONDARY_KEY.to_vec()],
                    ))) as Box<dyn Any>);
                }
                if req.is::<kvrpcpb::CheckSecondaryLocksRequest>() {
                    return Ok(Box::new(kvrpcpb::CheckSecondaryLocksResponse {
                        locks: vec![fallback_2pc_secondary_lock(SECONDARY_KEY)],
                        ..Default::default()
                    }) as Box<dyn Any>);
                }
                if let Some(req) = req.downcast_ref::<kvrpcpb::ResolveLockRequest>() {
                    resolve_lock_count_captured.fetch_add(1, Ordering::SeqCst);
                    resolved_commit_version_captured.store(req.commit_version, Ordering::SeqCst);
                    return Ok(Box::<kvrpcpb::ResolveLockResponse>::default() as Box<dyn Any>);
                }
                panic!("unexpected request type: {:?}", req.type_id());
            },
        )));

        let lock = async_commit_primary_lock(PRIMARY_KEY, 42, vec![SECONDARY_KEY.to_vec()]);

        let live_locks = resolve_locks(vec![lock], Timestamp::default(), client, Keyspace::Disable)
            .await
            .unwrap();

        assert_eq!(check_txn_status_count.load(Ordering::SeqCst), 2);
        assert_eq!(force_sync_count.load(Ordering::SeqCst), 1);
        if lock_ttl == 0 {
            assert!(live_locks.is_empty());
            // After the fallback the transaction is plain 2PC: only the encountered lock's
            // region is resolved, as client-go does.
            assert_eq!(resolve_lock_count.load(Ordering::SeqCst), 1);
            assert_eq!(
                resolved_commit_version.load(Ordering::SeqCst),
                commit_version
            );
        } else {
            assert_eq!(resolve_lock_count.load(Ordering::SeqCst), 0);
            assert_eq!(live_locks.len(), 1);
            assert_eq!(live_locks[0].lock_ttl, lock_ttl);
            assert!(!live_locks[0].use_async_commit);
        }
    }

    /// A region error on `CheckSecondaryLocks` makes the plan re-shard against fresh
    /// region boundaries. Every retried sub-request must be paired with its OWN keys
    /// (`preserve_shard` + `Collect::merge` contract): pairing a stale shard would make
    /// the shorter per-region lock lists below look like missing locks and roll back a
    /// committable transaction.
    #[tokio::test]
    #[serial]
    async fn test_check_secondary_locks_reshards_on_region_error() {
        // The transaction spans primary [1] and secondaries [2] and [3]. Every key
        // starts out in one region; after the simulated split, [2] and [3] live in two.
        fn mock_region(id: u64, start_key: Vec<u8>, end_key: Vec<u8>) -> RegionWithLeader {
            let mut region = RegionWithLeader::default();
            region.region.id = id;
            region.region.start_key = start_key;
            region.region.end_key = end_key;
            region.region.region_epoch = Some(metapb::RegionEpoch {
                conf_ver: 0,
                version: 1,
            });
            region.leader = Some(metapb::Peer {
                store_id: 41,
                ..Default::default()
            });
            region
        }

        let split = Arc::new(AtomicBool::new(false));
        let check_secondary_count = Arc::new(AtomicUsize::new(0));
        let resolve_lock_count = Arc::new(AtomicUsize::new(0));
        let resolved_commit_version = Arc::new(AtomicU64::new(u64::MAX));

        let split_in_dispatch = split.clone();
        let check_secondary_count_captured = check_secondary_count.clone();
        let resolve_lock_count_captured = resolve_lock_count.clone();
        let resolved_commit_version_captured = resolved_commit_version.clone();
        let split_in_region_hook = split.clone();
        let client = Arc::new(
            MockPdClient::new(MockKvClient::with_dispatch_hook(move |req: &dyn Any| {
                if req.is::<kvrpcpb::CheckTxnStatusRequest>() {
                    return Ok(Box::new(still_locked_response(async_commit_primary_lock(
                        &[1],
                        40,
                        vec![vec![2], vec![3]],
                    ))) as Box<dyn Any>);
                }
                if let Some(req) = req.downcast_ref::<kvrpcpb::CheckSecondaryLocksRequest>() {
                    check_secondary_count_captured.fetch_add(1, Ordering::SeqCst);
                    if req.keys == vec![vec![2], vec![3]] {
                        // First attempt, before the split: fail the whole shard.
                        split_in_dispatch.store(true, Ordering::SeqCst);
                        let resp = kvrpcpb::CheckSecondaryLocksResponse {
                            region_error: Some(errorpb::Error::default()),
                            ..Default::default()
                        };
                        return Ok(Box::new(resp) as Box<dyn Any>);
                    }
                    let min_commit_ts = match req.keys.as_slice() {
                        [key] if key.as_slice() == [2] => 41,
                        [key] if key.as_slice() == [3] => 42,
                        keys => panic!("unexpected CheckSecondaryLocks shard: {:?}", keys),
                    };
                    let resp = kvrpcpb::CheckSecondaryLocksResponse {
                        locks: vec![async_commit_secondary_lock(&req.keys[0], min_commit_ts)],
                        ..Default::default()
                    };
                    return Ok(Box::new(resp) as Box<dyn Any>);
                }
                if let Some(req) = req.downcast_ref::<kvrpcpb::ResolveLockRequest>() {
                    resolve_lock_count_captured.fetch_add(1, Ordering::SeqCst);
                    resolved_commit_version_captured.store(req.commit_version, Ordering::SeqCst);
                    return Ok(Box::<kvrpcpb::ResolveLockResponse>::default() as Box<dyn Any>);
                }
                panic!("unexpected request type: {:?}", req.type_id());
            }))
            .with_region_for_key_hook(move |key: &Key| {
                let key: &[u8] = key.into();
                if !split_in_region_hook.load(Ordering::SeqCst) {
                    Ok(mock_region(1, vec![], vec![10]))
                } else if key < &[3][..] {
                    Ok(mock_region(1, vec![], vec![3]))
                } else {
                    Ok(mock_region(4, vec![3], vec![10]))
                }
            }),
        );

        let lock = async_commit_primary_lock(&[1], 40, vec![vec![2], vec![3]]);

        let live_locks = resolve_locks(vec![lock], Timestamp::default(), client, Keyspace::Disable)
            .await
            .unwrap();

        assert!(live_locks.is_empty());
        // One failed pre-split request plus one per post-split region.
        assert_eq!(check_secondary_count.load(Ordering::SeqCst), 3);
        // Both locks survived, so the transaction must COMMIT at the maximum
        // min_commit_ts (42) — a stale shard pairing would have inferred a missing
        // lock instead and rolled the transaction back (commit version 0).
        assert_eq!(resolved_commit_version.load(Ordering::SeqCst), 42);
        assert_eq!(resolve_lock_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_resolve_locks_retries_expired_missing_primary_with_rollback() {
        let check_txn_status_count = Arc::new(AtomicUsize::new(0));
        let resolve_lock_count = Arc::new(AtomicUsize::new(0));

        let check_txn_status_count_captured = check_txn_status_count.clone();
        let resolve_lock_count_captured = resolve_lock_count.clone();
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if let Some(req) = req.downcast_ref::<kvrpcpb::CheckTxnStatusRequest>() {
                    check_txn_status_count_captured.fetch_add(1, Ordering::SeqCst);
                    if !req.rollback_if_not_exist {
                        return Ok(Box::new(kvrpcpb::CheckTxnStatusResponse {
                            error: Some(kvrpcpb::KeyError {
                                txn_not_found: Some(kvrpcpb::TxnNotFound {
                                    start_ts: req.lock_ts,
                                    primary_key: req.primary_key.clone(),
                                }),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }) as Box<dyn Any>);
                    }
                    return Ok(Box::<kvrpcpb::CheckTxnStatusResponse>::default() as Box<dyn Any>);
                }
                if req.is::<kvrpcpb::ResolveLockRequest>() {
                    resolve_lock_count_captured.fetch_add(1, Ordering::SeqCst);
                    return Ok(Box::<kvrpcpb::ResolveLockResponse>::default() as Box<dyn Any>);
                }
                panic!("unexpected request type: {:?}", req.type_id());
            },
        )));

        let lock = kvrpcpb::LockInfo {
            key: vec![1],
            primary_lock: vec![2],
            lock_version: EXPIRED_TXN_VERSION,
            lock_ttl: 1,
            ..Default::default()
        };

        let live_locks = resolve_locks(vec![lock], Timestamp::default(), client, Keyspace::Disable)
            .await
            .unwrap();

        assert!(live_locks.is_empty());
        assert_eq!(check_txn_status_count.load(Ordering::SeqCst), 2);
        assert_eq!(resolve_lock_count.load(Ordering::SeqCst), 1);
    }

    /// A pessimistic lock whose recorded primary is stale (the transaction changed its
    /// primary, pingcap/tidb#42937) makes TiKV answer CheckTxnStatus with PrimaryMismatch.
    /// Only that stale lock may be rolled back — the transaction itself may still be alive,
    /// so a region-wide ResolveLock is out of the question: each stale lock gets its own
    /// single-key PessimisticRollback, and a second stale lock of the same transaction in
    /// the same region must NOT be skipped or swept along.
    #[tokio::test]
    #[serial]
    async fn test_resolve_locks_rolls_back_stale_pessimistic_lock_on_primary_mismatch() {
        let rolled_back_keys = Arc::new(std::sync::Mutex::new(Vec::new()));

        let rolled_back_keys_captured = rolled_back_keys.clone();
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if req.is::<kvrpcpb::CheckTxnStatusRequest>() {
                    return Ok(Box::new(kvrpcpb::CheckTxnStatusResponse {
                        error: Some(kvrpcpb::KeyError {
                            primary_mismatch: Some(kvrpcpb::PrimaryMismatch {
                                lock_info: Some(kvrpcpb::LockInfo::default()),
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }) as Box<dyn Any>);
                }
                if let Some(req) = req.downcast_ref::<kvrpcpb::PessimisticRollbackRequest>() {
                    assert_eq!(req.start_version, EXPIRED_TXN_VERSION);
                    assert_eq!(req.for_update_ts, u64::MAX);
                    assert_eq!(req.keys.len(), 1, "rollback must target a single key");
                    rolled_back_keys_captured
                        .lock()
                        .unwrap()
                        .push(req.keys[0].clone());
                    return Ok(
                        Box::<kvrpcpb::PessimisticRollbackResponse>::default() as Box<dyn Any>
                    );
                }
                if req.is::<kvrpcpb::ResolveLockRequest>() {
                    panic!("must not sweep a region for a stale pessimistic lock");
                }
                panic!("unexpected request type: {:?}", req.type_id());
            },
        )));

        let make_lock = |key: u8| kvrpcpb::LockInfo {
            key: vec![key],
            primary_lock: vec![9],
            lock_version: EXPIRED_TXN_VERSION,
            lock_ttl: 1,
            lock_type: kvrpcpb::Op::PessimisticLock as i32,
            ..Default::default()
        };
        // Two stale pessimistic locks of the same transaction in the same mock region.
        let locks = vec![make_lock(1), make_lock(2)];

        let live_locks = resolve_locks(locks, Timestamp::default(), client, Keyspace::Disable)
            .await
            .unwrap();

        assert!(live_locks.is_empty());
        assert_eq!(
            *rolled_back_keys.lock().unwrap(),
            vec![vec![1], vec![2]],
            "each stale lock must be rolled back individually"
        );
    }

    /// A PrimaryMismatch for a non-pessimistic lock is unexpected (client-go treats it as an
    /// error too) and must propagate instead of rolling anything back.
    #[tokio::test]
    #[serial]
    async fn test_resolve_locks_propagates_primary_mismatch_for_non_pessimistic_lock() {
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if req.is::<kvrpcpb::CheckTxnStatusRequest>() {
                    return Ok(Box::new(kvrpcpb::CheckTxnStatusResponse {
                        error: Some(kvrpcpb::KeyError {
                            primary_mismatch: Some(kvrpcpb::PrimaryMismatch {
                                lock_info: Some(kvrpcpb::LockInfo::default()),
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }) as Box<dyn Any>);
                }
                if req.is::<kvrpcpb::ResolveLockRequest>() {
                    panic!("must not resolve a lock on unexpected primary mismatch");
                }
                panic!("unexpected request type: {:?}", req.type_id());
            },
        )));

        let lock = kvrpcpb::LockInfo {
            key: PRIMARY_KEY.to_vec(),
            primary_lock: vec![2],
            lock_version: EXPIRED_TXN_VERSION,
            lock_ttl: 1,
            ..Default::default()
        };

        let result =
            resolve_locks(vec![lock], Timestamp::default(), client, Keyspace::Disable).await;
        assert!(matches!(result, Err(Error::KeyError(_))));
    }

    #[test]
    fn format_key_for_log_hex_encodes_the_prefix() {
        assert_eq!(format_key_for_log(b"hello"), "len=5, prefix=68656C6C6F");
    }

    #[test]
    fn format_key_for_log_truncates_the_prefix_to_16_bytes() {
        let key: Vec<u8> = (0u8..20).collect();
        assert_eq!(
            format_key_for_log(&key),
            "len=20, prefix=000102030405060708090A0B0C0D0E0F"
        );
    }
}
