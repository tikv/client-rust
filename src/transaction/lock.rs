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

use crate::proto::kvrpcpb;
use crate::proto::kvrpcpb::TxnInfo;
use crate::proto::pdpb::Timestamp;
use crate::region::RegionVerId;
use crate::request::plan::handle_region_error;
use crate::request::plan::is_grpc_error;
use crate::request::Collect;
use crate::request::CollectSingle;
use crate::request::Keyspace;
use crate::request::Plan;
use crate::store::RegionStore;
use crate::timestamp::TimestampExt;
use crate::transaction::requests;
use crate::transaction::requests::new_check_secondary_locks_request;
use crate::transaction::requests::new_check_txn_status_request;
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
    let mut lock_resolver = LockResolver::new(ResolveLocksContext::default());

    // records the commit version of each primary lock (representing the status of the transaction)
    let mut commit_versions: HashMap<u64, u64> = HashMap::new();
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
        let region_ver_id = pd_client
            .region_for_key(&lock.key.clone().into())
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
            Some(&commit_version) => Some(commit_version),
            None => {
                // TODO: handle primary mismatch error.
                let status = lock_resolver
                    .get_txn_status_from_lock(
                        OPTIMISTIC_BACKOFF,
                        &lock,
                        caller_start_ts,
                        current_ts,
                        false,
                        pd_client.clone(),
                        keyspace,
                    )
                    .await?;
                let commit_version = match &status.kind {
                    TransactionStatusKind::Committed(ts) => Some(ts.version()),
                    TransactionStatusKind::RolledBack => Some(0),
                    // An expired async-commit primary: CheckTxnStatus never rolls it back,
                    // because the transaction may already be committed through its
                    // secondaries. Decide the outcome from the secondaries, as client-go's
                    // `resolveAsyncCommitLock` does, instead of waiting on a lock that only
                    // GC would ever clear.
                    TransactionStatusKind::Locked(_, primary)
                        if status.is_expired && primary.use_async_commit =>
                    {
                        let resolved = lock_resolver
                            .resolve_expired_async_commit(
                                &lock,
                                primary,
                                caller_start_ts,
                                current_ts,
                                pd_client.clone(),
                                keyspace,
                            )
                            .await?;
                        match resolved {
                            Ok(commit_version) => {
                                // Resolve the primary too, so later readers of this
                                // transaction find its final status at once.
                                let cleaned_region = resolve_lock_with_retry(
                                    &primary.key,
                                    lock.lock_version,
                                    commit_version,
                                    lock.is_txn_file,
                                    pd_client.clone(),
                                    keyspace,
                                    OPTIMISTIC_BACKOFF,
                                )
                                .await?;
                                clean_regions
                                    .entry(lock.lock_version)
                                    .or_default()
                                    .insert(cleaned_region);
                                Some(commit_version)
                            }
                            Err(live_lock) => {
                                live_locks.push(live_lock);
                                None
                            }
                        }
                    }
                    TransactionStatusKind::Locked(_, lock_info) => {
                        live_locks.push(lock_info.clone());
                        None
                    }
                };
                if let Some(commit_version) = commit_version {
                    commit_versions.insert(lock.lock_version, commit_version);
                }
                commit_version
            }
        };

        // The primary's region may be the one just resolved above.
        if clean_regions
            .get(&lock.lock_version)
            .map(|regions| regions.contains(&region_ver_id))
            .unwrap_or(false)
        {
            continue;
        }

        if let Some(commit_version) = commit_version {
            let cleaned_region = resolve_lock_with_retry(
                &lock.key,
                lock.lock_version,
                commit_version,
                lock.is_txn_file,
                pd_client.clone(),
                keyspace,
                OPTIMISTIC_BACKOFF,
            )
            .await?;
            clean_regions
                .entry(lock.lock_version)
                .or_default()
                .insert(cleaned_region);
        }
    }
    Ok(live_locks)
}

async fn resolve_lock_with_retry(
    #[allow(clippy::ptr_arg)] key: &Vec<u8>,
    start_version: u64,
    commit_version: u64,
    is_txn_file: bool,
    pd_client: Arc<impl PdClient>,
    keyspace: Keyspace,
    mut backoff: Backoff,
) -> Result<RegionVerId> {
    debug!("resolving locks with retry");
    let mut attempt = 0;
    loop {
        attempt += 1;
        debug!("resolving locks: attempt {}", attempt);
        let store = pd_client.clone().store_for_key(key.into()).await?;
        let ver_id = store.region_with_leader.ver_id();
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
                return Ok(ver_id);
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
                match self
                    .async_commit_version(pd_client.clone(), keyspace, lock_info)
                    .await?
                {
                    Some(commit_ts) => {
                        txn_infos.insert(txn_id, (commit_ts, l.is_txn_file));
                        continue;
                    }
                    None => {
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
                    }
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
            // A cached "locked" status answers a plain check only: forcing sync commit
            // exists to get a different answer for an expired async-commit lock.
            let locked = matches!(txn_status.kind, TransactionStatusKind::Locked(..));
            if !(force_sync_commit && locked) {
                return Ok(txn_status);
            }
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
            Err(Error::ExtractedErrors(mut errors)) => match errors.pop() {
                Some(Error::KeyError(key_err)) => {
                    if let Some(txn_not_found) = key_err.txn_not_found {
                        return Err(Error::TxnNotFound(txn_not_found));
                    }
                    // TODO: handle primary mismatch error.
                    return Err(Error::KeyError(key_err));
                }
                Some(err) => return Err(err),
                None => unreachable!(),
            },
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
            .retry_multi_region(DEFAULT_REGION_BACKOFF)
            .extract_error()
            .merge(Collect)
            .plan();
        plan.execute().await
    }

    /// The outcome of the async-commit transaction whose primary lock is `primary`,
    /// decided from its secondary locks (see
    /// [`SecondaryLocksStatus::async_commit_version`]): its commit version, 0 if it is
    /// rolled back, or `None` if it has to be resolved as a two-phase commit.
    async fn async_commit_version(
        &mut self,
        pd_client: Arc<impl PdClient>,
        keyspace: Keyspace,
        primary: &kvrpcpb::LockInfo,
    ) -> Result<Option<u64>> {
        let txn_id = primary.lock_version;
        let secondaries = if primary.secondaries.is_empty() {
            SecondaryLocksStatus::default()
        } else {
            self.check_all_secondaries(pd_client, keyspace, primary.secondaries.clone(), txn_id)
                .await?
        };
        let version = secondaries.async_commit_version(primary.min_commit_ts);
        debug!(
            "async commit secondaries, txn_id:{}, status:{:?}, primary min_commit_ts:{}, outcome:{:?}",
            txn_id, secondaries, primary.min_commit_ts, version
        );
        Ok(version)
    }

    /// Resolves the status of the transaction of `lock` whose primary, `primary`, is an
    /// expired async-commit lock. Returns its commit version (0 if it is rolled back), or
    /// the lock that is still live when the transaction fell back to two-phase commit and
    /// its primary is not expired after all.
    async fn resolve_expired_async_commit(
        &mut self,
        lock: &kvrpcpb::LockInfo,
        primary: &kvrpcpb::LockInfo,
        caller_start_ts: u64,
        current_ts: u64,
        pd_client: Arc<impl PdClient>,
        keyspace: Keyspace,
    ) -> Result<std::result::Result<u64, kvrpcpb::LockInfo>> {
        if let Some(commit_version) = self
            .async_commit_version(pd_client.clone(), keyspace, primary)
            .await?
        {
            return Ok(Ok(commit_version));
        }
        // A lock of the transaction is not an async-commit lock: the primary decides,
        // as in two-phase commit.
        debug!(
            "fallback to 2pc, txn_id:{}, check_txn_status with force_sync_commit",
            lock.lock_version
        );
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
        Ok(match &status.kind {
            TransactionStatusKind::Committed(ts) => Ok(ts.version()),
            TransactionStatusKind::RolledBack => Ok(0),
            TransactionStatusKind::Locked(_, lock_info) => Err(lock_info.clone()),
        })
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
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use fail::FailScenario;
    use serial_test::serial;

    use super::*;
    use crate::mock::MockKvClient;
    use crate::mock::MockPdClient;
    use crate::proto::errorpb;

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
        let resolved_region =
            resolve_lock_with_retry(&key, 1, 2, false, client.clone(), keyspace, backoff.clone())
                .await
                .unwrap();
        assert_eq!(region1.ver_id(), resolved_region);

        // Test resolve lock over retry limit
        fail::cfg(
            "region-error",
            &format!("{}*return", MAX_REGION_ERROR_RETRIES + 1),
        )
        .unwrap();
        let key = vec![100];
        resolve_lock_with_retry(&key, 3, 4, false, client, keyspace, backoff)
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

    /// What the fake TiKV saw while `resolve_locks` handled one lock of an async-commit
    /// transaction (start ts 5, primary key [1] in region 1, secondaries [20] in region 2
    /// and [251, 0] in region 3).
    #[derive(Debug, Default)]
    struct AsyncCommitRun {
        live_locks: usize,
        /// `force_sync_commit` of each CheckTxnStatus.
        check_txn_status: Vec<bool>,
        check_secondary_locks: usize,
        /// (region id, commit version) of each ResolveLock.
        resolved: Vec<(u64, u64)>,
    }

    const TXN: u64 = 5;

    fn async_commit_lock(key: Vec<u8>, min_commit_ts: u64) -> kvrpcpb::LockInfo {
        kvrpcpb::LockInfo {
            primary_lock: vec![1],
            lock_version: TXN,
            key,
            lock_ttl: 3000,
            use_async_commit: true,
            min_commit_ts,
            ..Default::default()
        }
    }

    /// Runs `resolve_locks` on the secondary lock at [20]. The primary lock answered by
    /// CheckTxnStatus has `primary_ttl` (0: expired under the mock's clock) and
    /// `min_commit_ts` 11; `secondaries` answers CheckSecondaryLocks for the keys of one
    /// region, and `forced` answers a CheckTxnStatus with `force_sync_commit`.
    async fn resolve_async_commit_lock(
        primary_ttl: u64,
        secondaries: impl Fn(&[Vec<u8>]) -> kvrpcpb::CheckSecondaryLocksResponse + Send + Sync + 'static,
        forced: kvrpcpb::CheckTxnStatusResponse,
    ) -> AsyncCommitRun {
        let seen = Arc::new(std::sync::Mutex::new(AsyncCommitRun::default()));
        let captured = seen.clone();
        let client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                let mut seen = captured.lock().unwrap();
                if let Some(req) = req.downcast_ref::<kvrpcpb::CheckTxnStatusRequest>() {
                    assert_eq!(req.primary_key, vec![1]);
                    seen.check_txn_status.push(req.force_sync_commit);
                    if req.force_sync_commit {
                        return Ok(Box::new(forced.clone()) as Box<dyn Any>);
                    }
                    let mut primary = async_commit_lock(vec![1], 11);
                    primary.secondaries = vec![vec![20], vec![251, 0]];
                    let resp = kvrpcpb::CheckTxnStatusResponse {
                        lock_ttl: primary_ttl,
                        lock_info: Some(primary),
                        action: kvrpcpb::Action::NoAction as i32,
                        ..Default::default()
                    };
                    return Ok(Box::new(resp) as Box<dyn Any>);
                }
                if let Some(req) = req.downcast_ref::<kvrpcpb::CheckSecondaryLocksRequest>() {
                    assert_eq!(req.start_version, TXN);
                    seen.check_secondary_locks += 1;
                    return Ok(Box::new(secondaries(&req.keys)) as Box<dyn Any>);
                }
                if let Some(req) = req.downcast_ref::<kvrpcpb::ResolveLockRequest>() {
                    assert_eq!(req.start_version, TXN);
                    let region = req.context.as_ref().unwrap().region_id;
                    seen.resolved.push((region, req.commit_version));
                    return Ok(Box::<kvrpcpb::ResolveLockResponse>::default() as Box<dyn Any>);
                }
                panic!("unexpected request type: {:?}", req.type_id());
            },
        )));

        let live_locks = resolve_locks(
            vec![async_commit_lock(vec![20], 10)],
            Timestamp::default(),
            client,
            Keyspace::Disable,
        )
        .await
        .unwrap();
        let mut run = std::mem::take(&mut *seen.lock().unwrap());
        run.live_locks = live_locks.len();
        run.resolved.sort_unstable();
        run
    }

    fn locked(keys: &[Vec<u8>], min_commit_ts: u64) -> kvrpcpb::CheckSecondaryLocksResponse {
        kvrpcpb::CheckSecondaryLocksResponse {
            locks: keys
                .iter()
                .map(|key| async_commit_lock(key.clone(), min_commit_ts))
                .collect(),
            ..Default::default()
        }
    }

    fn unused_forced() -> kvrpcpb::CheckTxnStatusResponse {
        kvrpcpb::CheckTxnStatusResponse::default()
    }

    #[tokio::test]
    #[serial]
    async fn expired_async_commit_with_every_secondary_locked_commits_at_the_max_min_commit_ts() {
        let run = resolve_async_commit_lock(
            0,
            |keys| {
                if keys[0] == [20] {
                    locked(keys, 10)
                } else {
                    locked(keys, 12)
                }
            },
            unused_forced(),
        )
        .await;
        assert_eq!(run.live_locks, 0);
        assert_eq!(run.check_txn_status, vec![false]);
        assert_eq!(run.check_secondary_locks, 2);
        // The primary (region 1) and the lock that was read (region 2), at
        // max(10, 12, primary's 11).
        assert_eq!(run.resolved, vec![(1, 12), (2, 12)]);
    }

    #[tokio::test]
    #[serial]
    async fn expired_async_commit_with_a_rolled_back_secondary_is_rolled_back() {
        let run = resolve_async_commit_lock(
            0,
            |keys| {
                if keys[0] == [20] {
                    locked(keys, 10)
                } else {
                    // TiKV wrote a rollback for the missing lock: no locks, no commit_ts.
                    kvrpcpb::CheckSecondaryLocksResponse::default()
                }
            },
            unused_forced(),
        )
        .await;
        assert_eq!(run.live_locks, 0);
        assert_eq!(run.resolved, vec![(1, 0), (2, 0)]);
    }

    #[tokio::test]
    #[serial]
    async fn expired_async_commit_with_a_committed_secondary_commits_at_its_ts() {
        let run = resolve_async_commit_lock(
            0,
            |keys| {
                if keys[0] == [20] {
                    locked(keys, 10)
                } else {
                    kvrpcpb::CheckSecondaryLocksResponse {
                        commit_ts: 15,
                        ..Default::default()
                    }
                }
            },
            unused_forced(),
        )
        .await;
        assert_eq!(run.live_locks, 0);
        assert_eq!(run.resolved, vec![(1, 15), (2, 15)]);
    }

    #[tokio::test]
    #[serial]
    async fn async_commit_that_fell_back_to_2pc_is_resolved_by_its_primary() {
        let run = resolve_async_commit_lock(
            0,
            |keys| {
                let mut resp = locked(keys, 10);
                if keys[0] == [20] {
                    resp.locks[0].use_async_commit = false;
                }
                resp
            },
            kvrpcpb::CheckTxnStatusResponse {
                action: kvrpcpb::Action::TtlExpireRollback as i32,
                ..Default::default()
            },
        )
        .await;
        assert_eq!(run.live_locks, 0);
        assert_eq!(run.check_txn_status, vec![false, true]);
        assert_eq!(run.resolved, vec![(1, 0), (2, 0)]);
    }

    #[tokio::test]
    #[serial]
    async fn unexpired_async_commit_primary_stays_live() {
        let run = resolve_async_commit_lock(100, |_| panic!("not expired"), unused_forced()).await;
        assert_eq!(run.live_locks, 1);
        assert_eq!(run.check_secondary_locks, 0);
        assert!(run.resolved.is_empty());
    }

    #[test]
    fn secondary_locks_status_decides_the_async_commit_outcome() {
        let all_locked = SecondaryLocksStatus {
            min_commit_ts: 12,
            ..Default::default()
        };
        assert_eq!(all_locked.async_commit_version(11), Some(12));
        assert_eq!(all_locked.async_commit_version(13), Some(13));
        let rolled_back = SecondaryLocksStatus {
            min_commit_ts: 12,
            rolled_back: true,
            ..Default::default()
        };
        assert_eq!(rolled_back.async_commit_version(11), Some(0));
        let committed = SecondaryLocksStatus {
            commit_ts: Some(Timestamp::from_version(15)),
            fallback_2pc: true,
            ..Default::default()
        };
        assert_eq!(committed.async_commit_version(11), Some(15));
        let fallback = SecondaryLocksStatus {
            min_commit_ts: 12,
            fallback_2pc: true,
            ..Default::default()
        };
        assert_eq!(fallback.async_commit_version(11), None);
    }

    #[test]
    fn merging_secondary_locks_refuses_contradictions() {
        use crate::request::Collect;
        use crate::request::Merge;
        let committed = |ts| {
            Ok(kvrpcpb::CheckSecondaryLocksResponse {
                commit_ts: ts,
                ..Default::default()
            })
        };
        let rolled_back = || Ok(kvrpcpb::CheckSecondaryLocksResponse::default());

        let merged = Collect
            .merge(vec![Ok(locked(&[vec![20]], 10)), rolled_back()])
            .unwrap();
        assert!(merged.rolled_back && merged.commit_ts.is_none());
        assert!(Collect.merge(vec![committed(15), rolled_back()]).is_err());
        assert!(Collect.merge(vec![committed(15), committed(16)]).is_err());
        let merged = Collect.merge(vec![committed(15), committed(15)]).unwrap();
        assert_eq!(merged.commit_ts.unwrap().version(), 15);
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
