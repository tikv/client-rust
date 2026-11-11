// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::cmp::Ordering;
use std::sync::atomic;
use std::sync::atomic::AtomicU8;
use std::sync::Arc;

use log::debug;
use tokio::time::sleep;

use super::Transaction;
use super::TransactionStatus;
use crate::backoff::Backoff;
use crate::pd::PdClient;
use crate::pd::PdRpcClient;
use crate::proto::pdpb::Timestamp;
use crate::region::RegionWithLeader;
use crate::request::plan::handle_region_error;
use crate::request::plan::is_grpc_error;
use crate::request::EncodeKeyspace;
use crate::request::KeyMode;
use crate::request::Keyspace;
use crate::request::Plan;
use crate::request::PlanBuilder;
use crate::request::RetryOptions;
use crate::request::TruncateKeyspace;
use crate::store::HasKeyErrors;
use crate::store::HasRegionError;
use crate::transaction::buffer::MutationIterator;
use crate::transaction::lowering::new_scan_request;
use crate::BoundRange;
use crate::Error;
use crate::Key;
use crate::KvPair;
use crate::Result;
use crate::Value;

impl<PdC: PdClient> Transaction<PdC> {
    /// Create a paginated transactional scanner over a range, the equivalent
    /// of client-go's `KVTxn.Iter(start, end)`.
    ///
    /// The returned [`Scanner`] fetches region-sequentially: each page is one
    /// scan request clamped to a single region and limited to
    /// `SCANNER_BATCH_SIZE` pairs. Construction fetches the first non-empty
    /// page, later pages are fetched only as the scanner is advanced.
    /// Fetched pairs are not inserted into the transaction's read
    /// cache; beyond the transaction's existing buffer, the scanner retains at
    /// most one fetched batch and one buffered mutation.
    ///
    /// Construction fetches the first non-empty page, so it fails with an RPC
    /// or lock-resolution error when that fetch fails. It also fails fast if
    /// the transaction no longer allows operations (e.g. it is already
    /// committed or rolled back), or if the range is reversed (start key
    /// greater than end key): reverse iteration is not supported, use
    /// [`Transaction::scan_reverse`] for a bounded reverse scan instead.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use tikv_client::{Key, KvPair, Value, Config, TransactionClient};
    /// # use futures::prelude::*;
    /// # futures::executor::block_on(async {
    /// #   let client = TransactionClient::new(vec!["192.168.0.100", "192.168.0.101"]).await.unwrap();
    /// #   let mut txn = client.begin_optimistic().await.unwrap();
    /// #   let key1: Key = b"bar".to_vec().into();
    /// #   let key2: Key = b"foo".to_vec().into();
    /// #   let mut scanner = txn.scanner(key1..key2).await.unwrap();
    /// #   while let Some(_pair) = scanner.next().await.unwrap() {
    ///       // Process the pair...
    /// #   }
    /// #   // Finish the transaction...
    /// #   txn.commit().await.unwrap();
    /// # });
    /// ```
    pub async fn scanner(&mut self, range: impl Into<BoundRange>) -> Result<Scanner<'_, PdC>> {
        debug!("creating transactional scanner");
        self.check_allow_operation().await?;
        let (start, end) = range.into().into_keys();
        if let Some(end) = &end {
            if !end.is_empty() && start > *end {
                return Err(Error::StringError(format!(
                    "scanner does not support reverse ranges (start key {start:?} is greater than end key {end:?}); use scan_reverse for a bounded reverse scan"
                )));
            }
        }
        let mut scanner = Scanner::new(self, start, end);
        scanner.initialize().await?;
        Ok(scanner)
    }
}

/// A paginated transactional range scanner. Its TiKV side is the equivalent
/// of client-go's `Scanner` (txnkv/txnsnapshot/scan.go), while its merge with
/// buffered mutations corresponds to client-go's `UnionIter`.
///
/// Fetches are region-sequential: each page is a single scan request clamped
/// to one region with a limit of `SCANNER_BATCH_SIZE` pairs, so a page
/// never fans out to every region overlapping the range. Fetched pairs are
/// not inserted into the transaction's read cache either. Buffered mutations
/// are merged one at a time with the TiKV page, so the scanner's additional
/// memory stays bounded regardless of the range size.
///
/// Created by [`Transaction::scanner`]. It borrows the transaction
/// buffer until dropped, preventing mutations from invalidating its local
/// iterator. As in client-go, reaching EOF or returning an error invalidates
/// the scanner; another [`Scanner::next`] call then returns an invalid-scanner
/// error.
pub struct Scanner<'a, PdC: PdClient = PdRpcClient> {
    local: MutationCursor<'a>,
    remote: RemoteScanner<PdC>,
    valid: bool,
}

/// The transaction buffer side of the union iterator.
///
/// It stays positioned on at most one mutation and advances the underlying
/// `BTreeMap` range linearly as entries are consumed.
struct MutationCursor<'a> {
    mutations: Option<MutationIterator<'a>>,
    current: Option<(Key, Option<Value>)>,
    keyspace: Keyspace,
}

/// The TiKV side of the union iterator.
///
/// It owns all RPC, region-retry, and pagination state and stays positioned on
/// at most one remote pair. The public `Scanner` only needs to compare and
/// consume that current pair.
struct RemoteScanner<PdC: PdClient> {
    status: Arc<AtomicU8>,
    pd_client: Arc<PdC>,
    keyspace: Keyspace,
    timestamp: Timestamp,
    retry_options: RetryOptions,
    end: Option<Key>,
    next_start: Key,
    cache: std::vec::IntoIter<KvPair>,
    current: Option<KvPair>,
    eof: bool,
}

impl<'a, PdC: PdClient> Scanner<'a, PdC> {
    fn new(txn: &'a mut Transaction<PdC>, start: Key, end: Option<Key>) -> Scanner<'a, PdC> {
        // TiKV and client-go use an empty end key to represent an unbounded
        // range, so only a non-empty upper bound participates in this check.
        let range_empty = end
            .as_ref()
            .is_some_and(|end| !end.is_empty() && &start >= end);
        let keyspace = txn.keyspace;
        let remote = RemoteScanner::new(txn, start.clone(), end.clone(), range_empty);
        let mutations = if range_empty {
            None
        } else {
            let range: BoundRange = (start.clone(), end.clone()).into();
            let range = range.encode_keyspace(keyspace, KeyMode::Txn);
            Some(txn.buffer.mutation_iter(range))
        };

        Scanner {
            local: MutationCursor::new(mutations, keyspace),
            remote,
            valid: true,
        }
    }

    /// Position both sides of the union iterator. Like client-go's
    /// `newScanner`, this eagerly fetches the first non-empty TiKV page.
    async fn initialize(&mut self) -> Result<()> {
        if let Err(error) = self.remote.ensure_current().await {
            self.valid = false;
            return Err(error);
        }
        Ok(())
    }

    /// Advance to the next pair, fetching the next batch from TiKV only when
    /// the buffered batch is exhausted. The first observation of EOF returns
    /// `Ok(None)` and invalidates the scanner. Calling `next` again after EOF,
    /// or after an earlier call returned an error, returns an invalid-scanner
    /// error.
    pub async fn next(&mut self) -> Result<Option<KvPair>> {
        if !self.valid {
            return Err(invalid_scanner_error());
        }

        loop {
            if let Err(error) = self.remote.ensure_current().await {
                self.valid = false;
                return Err(error);
            }

            let ordering = match (self.remote.current_key(), self.local.current_key()) {
                (None, None) => {
                    self.valid = false;
                    return Ok(None);
                }
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (Some(remote_key), Some(local_key)) => remote_key.cmp(local_key),
            };

            match ordering {
                Ordering::Less => return Ok(Some(self.remote.take_current())),
                Ordering::Greater => {
                    if let Some(pair) = self.local.take_current() {
                        return Ok(Some(pair));
                    }
                }
                Ordering::Equal => {
                    // A buffered mutation overrides the value at the same key.
                    self.remote.take_current();
                    if let Some(pair) = self.local.take_current() {
                        return Ok(Some(pair));
                    }
                }
            }
        }
    }
}

impl<'a> MutationCursor<'a> {
    fn new(mutations: Option<MutationIterator<'a>>, keyspace: Keyspace) -> Self {
        let mut cursor = MutationCursor {
            mutations,
            current: None,
            keyspace,
        };
        cursor.advance();
        cursor
    }

    fn advance(&mut self) {
        let next = self.mutations.as_mut().and_then(Iterator::next);
        match next {
            Some((key, value)) => {
                let key = key.truncate_keyspace(self.keyspace);
                self.current = Some((key, value));
            }
            None => {
                self.current = None;
                self.mutations = None;
            }
        }
    }

    fn current_key(&self) -> Option<&Key> {
        self.current.as_ref().map(|(key, _)| key)
    }

    fn take_current(&mut self) -> Option<KvPair> {
        let (key, value) = self
            .current
            .take()
            .expect("local cursor must be valid when it is selected");
        self.advance();
        value.map(|value| KvPair::new(key, value))
    }
}

impl<PdC: PdClient> RemoteScanner<PdC> {
    fn new(txn: &Transaction<PdC>, start: Key, end: Option<Key>, range_empty: bool) -> Self {
        RemoteScanner {
            status: txn.status.clone(),
            pd_client: txn.rpc.clone(),
            keyspace: txn.keyspace,
            timestamp: txn.timestamp,
            retry_options: txn.options.retry_options.clone(),
            end,
            next_start: start,
            cache: Vec::new().into_iter(),
            current: None,
            eof: range_empty,
        }
    }

    /// Position on a TiKV pair, skipping empty regions as needed.
    async fn ensure_current(&mut self) -> Result<()> {
        while self.current.is_none() {
            if let Some(pair) = self.cache.next() {
                self.current = Some(pair);
            } else if self.eof {
                break;
            } else {
                let page = self.fetch_remote_page().await?;
                self.cache = page.into_iter();
            }
        }
        Ok(())
    }

    fn current_key(&self) -> Option<&Key> {
        self.current.as_ref().map(KvPair::key)
    }

    fn take_current(&mut self) -> KvPair {
        let pair = self
            .current
            .take()
            .expect("remote scanner must be valid when it is selected");
        self.current = self.cache.next();
        pair
    }

    /// Fetch the next TiKV page, scanning exactly one currently located region.
    ///
    /// The request is clamped to the first region overlapping the remaining
    /// range and its RPC limit is always `SCANNER_BATCH_SIZE`. A region error
    /// causes the current start key to be located again, so a split never turns
    /// one page into a multi-region fan-out.
    ///
    /// The page state machine follows client-go's `Scanner.getData`
    /// (txnkv/txnsnapshot/scan.go): a full page means more data may remain in
    /// this region span; resume after the last returned key with `next_key()`
    /// (the key+'\x00' successor trick client-go's `NextKey` uses). When that
    /// lands exactly on the range end the scan is done; otherwise, if the
    /// span holds an exact multiple of the batch size, the next page can come
    /// back empty, same as client-go discovering EOF on its next getData. A
    /// short page means the TiKV span has been fully emitted: if it was the
    /// last region in the range the remote stream is done, otherwise the next
    /// page starts at the region's end key.
    async fn fetch_remote_page(&mut self) -> Result<Vec<KvPair>> {
        self.check_allow_operation()?;
        let keyspace = self.keyspace;
        let mut region_backoff = self.retry_options.region_backoff.clone();

        loop {
            let range: BoundRange = (self.next_start.clone(), self.end.clone()).into();
            let (enc_start, enc_end) = range.encode_keyspace(keyspace, KeyMode::Txn).into_keys();
            let enc_start: Vec<u8> = enc_start.into();
            let enc_end: Vec<u8> = enc_end.map(Into::into).unwrap_or_default();

            // Locate only the region holding the current start key; paginating
            // region-sequentially never needs the rest of the range. On every
            // retry this is rebuilt from `next_start`, so a split can only
            // select the first child. A start key with no region is an error
            // rather than EOF.
            let region = self
                .pd_client
                .region_for_key(&Key::from(enc_start.clone()))
                .await?;

            // Clamp the request to the intersection of the region and the
            // remaining range. The start key already lies inside the region,
            // so only the end needs clamping; an empty end key is unbounded.
            let region_end: Vec<u8> = region.end_key().into();
            let (span, span_end) =
                if region_end.is_empty() || (!enc_end.is_empty() && region_end > enc_end) {
                    // The region covers the rest of the range.
                    let span: BoundRange = if enc_end.is_empty() {
                        BoundRange::range_from(enc_start.clone().into())
                    } else {
                        (enc_start.clone(), enc_end.clone()).into()
                    };
                    (span, enc_end.clone())
                } else {
                    let span: BoundRange = (enc_start.clone(), region_end.clone()).into();
                    (span, region_end)
                };

            let region_store = match self
                .pd_client
                .clone()
                .map_region_to_store(region.clone())
                .await
            {
                Ok(store) => store,
                Err(error) => {
                    self.retry_region_client_error(&region, &mut region_backoff, error)
                        .await?;
                    continue;
                }
            };

            let request = new_scan_request(span, self.timestamp, SCANNER_BATCH_SIZE, false, false);
            let plan = match PlanBuilder::new(self.pd_client.clone(), keyspace, request)
                .single_region_with_store(region_store.clone())
                .await
            {
                Ok(builder) => builder
                    .resolve_lock(
                        self.timestamp,
                        self.retry_options.lock_backoff.clone(),
                        keyspace,
                    )
                    .plan(),
                Err(error) => {
                    self.retry_region_client_error(&region, &mut region_backoff, error)
                        .await?;
                    continue;
                }
            };

            let mut response = match plan.execute().await {
                Ok(response) => response,
                Err(error) if is_grpc_error(&error) => {
                    self.retry_region_client_error(&region, &mut region_backoff, error)
                        .await?;
                    continue;
                }
                Err(error) => return Err(error),
            };

            if let Some(region_error) = response.region_error() {
                let Some(delay) = region_backoff.next_delay_duration() else {
                    return Err(Error::RegionError(Box::new(region_error)));
                };
                let retry_immediately =
                    handle_region_error(self.pd_client.clone(), region_error, region_store).await?;
                if !retry_immediately {
                    sleep(delay).await;
                }
                continue;
            }
            if let Some(errors) = response.key_errors() {
                return Err(Error::MultipleKeyErrors(errors));
            }

            let page: Vec<KvPair> = response.pairs.into_iter().map(Into::into).collect();
            if page.len() as u32 == SCANNER_BATCH_SIZE {
                let last = page.last().expect("a full page is never empty");
                let next_start = last.key().clone().truncate_keyspace(keyspace).next_key();
                // A full page can land exactly on the range end; stop there
                // instead of issuing an empty-range RPC for the next page.
                if self
                    .end
                    .as_ref()
                    .is_some_and(|end| !end.is_empty() && next_start >= *end)
                {
                    self.eof = true;
                }
                self.next_start = next_start;
            } else if span_end.is_empty()
                || (!enc_end.is_empty() && span_end.as_slice() >= enc_end.as_slice())
            {
                self.eof = true;
            } else {
                self.next_start = Key::from(span_end).truncate_keyspace(keyspace);
            }
            return Ok(page.truncate_keyspace(keyspace));
        }
    }

    /// Invalidate the failed location and apply the transaction's region
    /// backoff before locating the current start key again.
    async fn retry_region_client_error(
        &self,
        region: &RegionWithLeader,
        backoff: &mut Backoff,
        error: Error,
    ) -> Result<()> {
        self.pd_client
            .invalidate_region_cache(region.ver_id())
            .await;
        if is_grpc_error(&error) {
            if let Ok(store_id) = region.get_store_id() {
                self.pd_client.invalidate_store_cache(store_id).await;
            }
        }
        let Some(delay) = backoff.next_delay_duration() else {
            return Err(error);
        };
        sleep(delay).await;
        Ok(())
    }

    fn check_allow_operation(&self) -> Result<()> {
        match TransactionStatus::from(self.status.load(atomic::Ordering::Acquire)) {
            TransactionStatus::ReadOnly | TransactionStatus::Active => Ok(()),
            TransactionStatus::Committed
            | TransactionStatus::Rolledback
            | TransactionStatus::StartedCommit
            | TransactionStatus::StartedRollback
            | TransactionStatus::Dropped => Err(Error::OperationAfterCommitError),
        }
    }
}

fn invalid_scanner_error() -> Error {
    Error::StringError("scanner iterator is invalid".to_owned())
}

/// Batch size used internally by `scanner` to paginate through a range.
/// Matches client-go's `DefaultScanBatchSize` (txnkv/txnsnapshot/snapshot.go).
const SCANNER_BATCH_SIZE: u32 = 256;

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::collections::BTreeMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::super::Transaction;
    use super::super::TransactionOptions;
    use crate::mock::MockKvClient;
    use crate::mock::MockPdClient;
    use crate::pd::PdClient;
    use crate::proto::errorpb;
    use crate::proto::keyspacepb;
    use crate::proto::kvrpcpb;
    use crate::proto::metapb;
    use crate::proto::pdpb::Timestamp;
    use crate::region::RegionId;
    use crate::region::RegionVerId;
    use crate::region::RegionWithLeader;
    use crate::request::EncodeKeyspace;
    use crate::request::KeyMode;
    use crate::request::Keyspace;
    use crate::store::RegionStore;
    use crate::store::Store;
    use crate::Key;
    use crate::KvPair;

    fn mock_scan_response(
        data: &BTreeMap<Vec<u8>, Vec<u8>>,
        scan: &kvrpcpb::ScanRequest,
    ) -> kvrpcpb::ScanResponse {
        let pairs = data
            .range(scan.start_key.clone()..)
            .take_while(|(key, _)| {
                scan.end_key.is_empty() || key.as_slice() < scan.end_key.as_slice()
            })
            .take(scan.limit as usize)
            .map(|(key, value)| kvrpcpb::KvPair {
                key: key.clone(),
                value: value.clone(),
                ..Default::default()
            })
            .collect();
        kvrpcpb::ScanResponse {
            pairs,
            ..Default::default()
        }
    }

    fn assert_invalid_scanner(result: crate::Result<Option<KvPair>>) {
        assert!(matches!(
            result,
            Err(crate::Error::StringError(message))
                if message == "scanner iterator is invalid"
        ));
    }

    fn scanner_test_region(
        id: RegionId,
        start_key: Vec<u8>,
        end_key: Vec<u8>,
        version: u64,
    ) -> RegionWithLeader {
        RegionWithLeader {
            region: metapb::Region {
                id,
                start_key,
                end_key,
                region_epoch: Some(metapb::RegionEpoch {
                    conf_ver: 0,
                    version,
                }),
                ..Default::default()
            },
            leader: Some(metapb::Peer {
                store_id: 100 + id,
                ..Default::default()
            }),
        }
    }

    struct SplitPdClient {
        inner: Arc<MockPdClient>,
        split: Arc<AtomicBool>,
    }

    impl SplitPdClient {
        const SPLIT_KEY: &'static [u8] = b"k0150";

        fn unsplit_region() -> RegionWithLeader {
            scanner_test_region(10, vec![10], vec![250, 250], 0)
        }

        fn left_region() -> RegionWithLeader {
            scanner_test_region(11, vec![10], Self::SPLIT_KEY.to_vec(), 1)
        }

        fn right_region() -> RegionWithLeader {
            scanner_test_region(12, Self::SPLIT_KEY.to_vec(), vec![250, 250], 1)
        }
    }

    #[async_trait]
    impl PdClient for SplitPdClient {
        type KvClient = MockKvClient;

        async fn map_region_to_store(
            self: Arc<Self>,
            region: RegionWithLeader,
        ) -> crate::Result<RegionStore> {
            self.inner.clone().map_region_to_store(region).await
        }

        async fn region_for_key(&self, key: &Key) -> crate::Result<RegionWithLeader> {
            if !self.split.load(Ordering::SeqCst) {
                return Ok(Self::unsplit_region());
            }
            let key: &[u8] = key.into();
            if key < Self::SPLIT_KEY {
                Ok(Self::left_region())
            } else {
                Ok(Self::right_region())
            }
        }

        async fn region_for_id(&self, id: RegionId) -> crate::Result<RegionWithLeader> {
            match id {
                10 => Ok(Self::unsplit_region()),
                11 => Ok(Self::left_region()),
                12 => Ok(Self::right_region()),
                _ => Err(crate::Error::RegionNotFoundInResponse { region_id: id }),
            }
        }

        async fn all_stores(&self) -> crate::Result<Vec<Store>> {
            self.inner.all_stores().await
        }

        async fn get_timestamp(self: Arc<Self>) -> crate::Result<Timestamp> {
            Ok(Timestamp::default())
        }

        async fn update_safepoint(self: Arc<Self>, _safepoint: u64) -> crate::Result<bool> {
            Ok(true)
        }

        async fn load_keyspace(&self, _keyspace: &str) -> crate::Result<keyspacepb::KeyspaceMeta> {
            unreachable!("scanner tests do not load keyspaces")
        }

        async fn update_leader(
            &self,
            _ver_id: RegionVerId,
            _leader: metapb::Peer,
        ) -> crate::Result<()> {
            Ok(())
        }

        async fn invalidate_region_cache(&self, _ver_id: RegionVerId) {}

        async fn invalidate_store_cache(&self, _store_id: u64) {}
    }

    #[tokio::test]
    async fn scanner_paginates_lazily_after_eager_initial_page() {
        // One pair beyond the batch boundary is enough to exercise pagination.
        let data: BTreeMap<Vec<u8>, Vec<u8>> = (0..257u32)
            .map(|i| {
                (
                    format!("k{i:04}").into_bytes(),
                    format!("v{i}").into_bytes(),
                )
            })
            .collect();
        let expected: Vec<KvPair> = data
            .iter()
            .map(|(key, value)| KvPair::new(key.clone(), value.clone()))
            .collect();
        let scan_calls = Arc::new(AtomicUsize::new(0));
        let scan_calls_cloned = scan_calls.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                let scan = req
                    .downcast_ref::<kvrpcpb::ScanRequest>()
                    .expect("only scan requests are expected");
                scan_calls_cloned.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(mock_scan_response(&data, scan)) as Box<dyn Any>)
            },
        )));

        let mut txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic().read_only(),
            Keyspace::Disable,
        );
        let mut scanner = txn
            .scanner("k0000".to_owned()..="k9999".to_owned())
            .await
            .unwrap();
        // Construction eagerly positions the remote iterator, like client-go.
        assert_eq!(scan_calls.load(Ordering::SeqCst), 1);
        let mut result: Vec<KvPair> = Vec::new();
        for _ in 0..super::SCANNER_BATCH_SIZE {
            result.push(scanner.next().await.unwrap().unwrap());
        }
        // Consuming the initial page does not prefetch the next one.
        assert_eq!(scan_calls.load(Ordering::SeqCst), 1);
        while let Some(pair) = scanner.next().await.unwrap() {
            result.push(pair);
        }

        assert_eq!(scan_calls.load(Ordering::SeqCst), 2);
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn scanner_rejects_inverted_range() {
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |_req: &dyn Any| {
                panic!("an inverted range must not issue an RPC");
            },
        )));
        let mut txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic().read_only(),
            Keyspace::Disable,
        );

        // A reversed range fails fast at construction instead of being
        // silently treated as empty.
        assert!(matches!(
            txn.scanner("z".to_owned().."a".to_owned()).await,
            Err(crate::Error::StringError(_))
        ));
        // An empty (but not reversed) range is still valid and yields nothing.
        let mut scanner = txn.scanner("a".to_owned().."a".to_owned()).await.unwrap();
        assert!(scanner.next().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn scanner_checks_transaction_status_at_construction() {
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |_req: &dyn Any| {
                panic!("a finished transaction must not issue an RPC");
            },
        )));
        let mut txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic(),
            Keyspace::Disable,
        );
        txn.rollback().await.unwrap();

        // Even an empty range, which fetches nothing, fails fast once the
        // transaction no longer allows operations.
        assert!(matches!(
            txn.scanner("a".to_owned().."a".to_owned()).await,
            Err(crate::Error::OperationAfterCommitError)
        ));
        assert!(matches!(
            txn.scanner("k0000".to_owned()..).await,
            Err(crate::Error::OperationAfterCommitError)
        ));
    }

    #[tokio::test]
    async fn scanner_returns_initial_page_error_from_construction() {
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |req: &dyn Any| {
                req.downcast_ref::<kvrpcpb::ScanRequest>()
                    .expect("only scan requests are expected");
                Err(crate::Error::StringError("initial scan failed".to_owned()))
            },
        )));
        let mut txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic().read_only(),
            Keyspace::Disable,
        );

        assert!(matches!(
            txn.scanner("k0000".to_owned()..).await,
            Err(crate::Error::StringError(message)) if message == "initial scan failed"
        ));
    }

    #[tokio::test]
    async fn scanner_errors_when_range_has_no_region() {
        let pd_client = Arc::new(
            MockPdClient::new(MockKvClient::with_dispatch_hook(|_req: &dyn Any| {
                panic!("a range with no region must not issue an RPC");
            }))
            .with_region_for_key_hook(|key| {
                Err(crate::Error::StringError(format!(
                    "no region is found for key {key:?}"
                )))
            }),
        );
        let mut txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic().read_only(),
            Keyspace::Disable,
        );

        // A range whose start key has no region is an error, not an empty scan.
        assert!(matches!(
            txn.scanner("k0000".to_owned()..).await,
            Err(crate::Error::StringError(message)) if message.starts_with("no region is found for key")
        ));
    }

    #[tokio::test]
    async fn scanner_is_invalid_after_page_error() {
        let data: BTreeMap<Vec<u8>, Vec<u8>> = (0..super::SCANNER_BATCH_SIZE)
            .map(|i| {
                (
                    format!("k{i:04}").into_bytes(),
                    format!("v{i}").into_bytes(),
                )
            })
            .collect();
        let scan_calls = Arc::new(AtomicUsize::new(0));
        let scan_calls_cloned = scan_calls.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                let scan = req
                    .downcast_ref::<kvrpcpb::ScanRequest>()
                    .expect("only scan requests are expected");
                let call = scan_calls_cloned.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    Ok(Box::new(mock_scan_response(&data, scan)) as Box<dyn Any>)
                } else {
                    Err(crate::Error::StringError("scan page failed".to_owned()))
                }
            },
        )));
        let mut txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic().read_only(),
            Keyspace::Disable,
        );
        let mut scanner = txn.scanner("k0000".to_owned()..).await.unwrap();

        for _ in 0..super::SCANNER_BATCH_SIZE {
            assert!(scanner.next().await.unwrap().is_some());
        }
        assert!(matches!(
            scanner.next().await,
            Err(crate::Error::StringError(message)) if message == "scan page failed"
        ));
        assert_invalid_scanner(scanner.next().await);
        assert_eq!(scan_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn scanner_scans_region_sequentially() {
        // The mock PD client serves three regions: [.., [10]), [[10], [250, 250])
        // and [[250, 250], ..). The first is empty, the second holds more than
        // one batch, and the third is non-empty, exercising both pagination
        // and region transitions without treating a short page as global EOF.
        let mut data: BTreeMap<Vec<u8>, Vec<u8>> = (0..300u32)
            .map(|i| {
                (
                    format!("r2k{i:04}").into_bytes(),
                    format!("v{i}").into_bytes(),
                )
            })
            .collect();
        for i in 0..5u8 {
            data.insert(vec![250, 250, i], format!("r3v{i}").into_bytes());
        }
        let expected: Vec<KvPair> = data
            .iter()
            .map(|(key, value)| KvPair::new(key.clone(), value.clone()))
            .collect();

        let requests = Arc::new(Mutex::new(Vec::new()));
        let requests_cloned = requests.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                let scan = req
                    .downcast_ref::<kvrpcpb::ScanRequest>()
                    .expect("only scan requests are expected");
                requests_cloned
                    .lock()
                    .unwrap()
                    .push((scan.start_key.clone(), scan.end_key.clone()));
                Ok(Box::new(mock_scan_response(&data, scan)) as Box<dyn Any>)
            },
        )));

        let mut txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic().read_only(),
            Keyspace::Disable,
        );
        let mut scanner = txn.scanner(..).await.unwrap();
        // Construction skips the empty first region and positions on the
        // first non-empty page, like client-go.
        assert_eq!(requests.lock().unwrap().len(), 2);
        let mut result: Vec<KvPair> = Vec::new();
        while let Some(pair) = scanner.next().await.unwrap() {
            result.push(pair);
        }

        assert_eq!(result, expected);

        // Each page is clamped to a single region — pages never merge across
        // regions — and a short page does not terminate the scan: the empty
        // region1 page is followed by region2's two pages, then region3's page.
        let expected: Vec<(Vec<u8>, Vec<u8>)> = vec![
            (vec![], vec![10]),
            (vec![10], vec![250, 250]),
            (b"r2k0255\0".to_vec(), vec![250, 250]),
            (vec![250, 250], vec![]),
        ];
        assert_eq!(*requests.lock().unwrap(), expected);
    }

    #[tokio::test]
    async fn scanner_relocates_only_one_region_after_split() {
        let data: BTreeMap<Vec<u8>, Vec<u8>> = (0..300u32)
            .map(|i| {
                (
                    format!("k{i:04}").into_bytes(),
                    format!("v{i}").into_bytes(),
                )
            })
            .collect();
        let split = Arc::new(AtomicBool::new(false));
        let first_request = Arc::new(AtomicBool::new(true));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let split_cloned = split.clone();
        let first_request_cloned = first_request.clone();
        let requests_cloned = requests.clone();
        let kv_client = MockKvClient::with_dispatch_hook(move |req: &dyn Any| {
            let scan = req
                .downcast_ref::<kvrpcpb::ScanRequest>()
                .expect("only scan requests are expected");
            requests_cloned
                .lock()
                .unwrap()
                .push((scan.start_key.clone(), scan.end_key.clone()));

            if first_request_cloned.swap(false, Ordering::SeqCst) {
                // Simulate the located region splitting before it handles the
                // request. The retry must relocate only the child containing
                // the current start key.
                split_cloned.store(true, Ordering::SeqCst);
                return Ok(Box::new(kvrpcpb::ScanResponse {
                    region_error: Some(errorpb::Error {
                        epoch_not_match: Some(Default::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }) as Box<dyn Any>);
            }

            Ok(Box::new(mock_scan_response(&data, scan)) as Box<dyn Any>)
        });
        let pd_client = Arc::new(SplitPdClient {
            inner: Arc::new(MockPdClient::new(kv_client)),
            split,
        });
        let mut txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic().read_only(),
            Keyspace::Disable,
        );
        let mut scanner = txn
            .scanner("k0000".to_owned()..="k0299".to_owned())
            .await
            .unwrap();

        let first = scanner.next().await.unwrap().unwrap();
        assert_eq!(first.key(), &"k0000".to_owned().into());
        // The failed old-region request and the left-child retry have run. A
        // fan-out retry would already have contacted the right child as well.
        assert_eq!(
            *requests.lock().unwrap(),
            vec![
                (b"k0000".to_vec(), b"k0299\0".to_vec()),
                (b"k0000".to_vec(), b"k0150".to_vec()),
            ]
        );

        let mut result = vec![first];
        while let Some(pair) = scanner.next().await.unwrap() {
            result.push(pair);
        }
        assert_eq!(result.len(), 300);
        assert_eq!(result.last().unwrap().key(), &"k0299".to_owned().into());
        assert_eq!(
            *requests.lock().unwrap(),
            vec![
                (b"k0000".to_vec(), b"k0299\0".to_vec()),
                (b"k0000".to_vec(), b"k0150".to_vec()),
                (b"k0150".to_vec(), b"k0299\0".to_vec()),
            ]
        );
    }

    #[rstest::rstest]
    #[case(Keyspace::Disable)]
    #[case(Keyspace::Enable { keyspace_id: 0 })]
    #[tokio::test]
    async fn scanner_overlays_buffer_without_caching_it(#[case] keyspace: Keyspace) {
        // 300 pairs, all in the mock's region2 ([10], [250, 250]).
        let data: BTreeMap<Vec<u8>, Vec<u8>> = (0..300u32)
            .map(|i| {
                (
                    Key::from(format!("k{i:04}"))
                        .encode_keyspace(keyspace, KeyMode::Txn)
                        .into(),
                    format!("v{i}").into_bytes(),
                )
            })
            .collect();
        let scan_calls = Arc::new(AtomicUsize::new(0));
        let get_calls = Arc::new(AtomicUsize::new(0));
        let scan_calls_cloned = scan_calls.clone();
        let get_calls_cloned = get_calls.clone();
        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            move |req: &dyn Any| {
                if let Some(scan) = req.downcast_ref::<kvrpcpb::ScanRequest>() {
                    scan_calls_cloned.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(scan.limit, super::SCANNER_BATCH_SIZE);
                    Ok(Box::new(mock_scan_response(&data, scan)) as Box<dyn Any>)
                } else if let Some(get) = req.downcast_ref::<kvrpcpb::GetRequest>() {
                    get_calls_cloned.fetch_add(1, Ordering::SeqCst);
                    let value = data.get(&get.key).cloned().unwrap_or_default();
                    Ok(Box::new(kvrpcpb::GetResponse {
                        value,
                        ..Default::default()
                    }) as Box<dyn Any>)
                } else if req
                    .downcast_ref::<kvrpcpb::BatchRollbackRequest>()
                    .is_some()
                {
                    Ok(Box::new(kvrpcpb::BatchRollbackResponse::default()) as Box<dyn Any>)
                } else {
                    panic!("unexpected request type");
                }
            },
        )));

        let mut txn = Transaction::new(
            Timestamp::default(),
            pd_client,
            TransactionOptions::new_optimistic(),
            keyspace,
        );
        // Cover overriding puts, inserted keys, and deletes both with and
        // without a matching remote key. The request assertion above ensures
        // local mutations do not inflate the TiKV RPC limit.
        txn.put("k0005".to_owned(), "local".to_owned())
            .await
            .unwrap();
        txn.insert("k0005a".to_owned(), "inserted".to_owned())
            .await
            .unwrap();
        txn.delete("k0005b".to_owned()).await.unwrap();
        txn.delete("k0006".to_owned()).await.unwrap();

        let mut result: Vec<KvPair> = Vec::new();
        let mut scanner = txn
            .scanner("k0000".to_owned()..="k0299".to_owned())
            .await
            .unwrap();
        while let Some(pair) = scanner.next().await.unwrap() {
            result.push(pair);
        }
        drop(scanner);

        // Buffered puts/inserts win and deletes hide their keys.
        assert_eq!(result.len(), 300);
        assert_eq!(result[5].key(), &"k0005".to_owned().into());
        assert_eq!(result[5].value(), &b"local".to_vec());
        assert_eq!(result[6].key(), &"k0005a".to_owned().into());
        assert_eq!(result[6].value(), &b"inserted".to_vec());
        assert!(result.iter().all(|p| p.key() != &"k0006".to_owned().into()));
        // The remote stream still takes exactly two fixed-size pages; local
        // mutations are merged separately and never inflate an RPC limit.
        assert_eq!(scan_calls.load(Ordering::SeqCst), 2);

        // Fetched pairs must not be inserted into the read cache: a get for an
        // already-scanned key still goes to TiKV.
        let value = txn.get("k0010".to_owned()).await.unwrap();
        assert_eq!(value, Some(b"v10".to_vec()));
        assert_eq!(get_calls.load(Ordering::SeqCst), 1);

        txn.rollback().await.unwrap();
    }
}
