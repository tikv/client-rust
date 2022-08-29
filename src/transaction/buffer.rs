// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::{PdClient, PdRpcClient},
    request::{Collect, Plan, PlanBuilder},
    transaction_lowering::new_scan_request,
    BoundRange, Key, KvPair, Result, RetryOptions, Value,
};
use futures::stream::{self, StreamExt};
use std::{
    collections::{btree_map::Entry, BTreeMap, HashMap},
    convert::TryInto,
    future::Future,
    sync::Arc,
};
use tikv_client_proto::{kvrpcpb, pdpb::Timestamp};

pub struct Scanner<PdC: PdClient = PdRpcClient> {
    pdc: Arc<PdC>,
    timestamp: Timestamp,
    range: BoundRange,
    batch_size: u32,
    current: usize,
    limit: u32,
    cache: Vec<KvPair>,
    next_start_key: Key,
    next_end_key: Key,
    reverse: bool,
    key_only: bool,
    retry_options: RetryOptions,
}

impl<PdC: PdClient> Scanner<PdC> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pdc: Arc<PdC>,
        timestamp: Timestamp,
        range: BoundRange,
        batch_size: u32,
        limit: u32,
        reverse: bool,
        key_only: bool,
        retry_options: RetryOptions,
    ) -> Self {
        let (start, end) = range.clone().into_keys();
        Self {
            pdc,
            timestamp,
            range,
            batch_size,
            current: 0,
            limit,
            cache: Vec::new(),
            next_start_key: start,
            next_end_key: end.unwrap_or(Key::EMPTY),
            reverse,
            key_only,
            retry_options,
        }
    }

    pub async fn fetch_data(&self) -> Result<Vec<KvPair>> {
        let request = new_scan_request(
            self.range.clone(),
            self.timestamp.clone(),
            self.batch_size,
            self.key_only,
            self.reverse,
        );
        let retry_options = self.retry_options.clone();
        let plan = PlanBuilder::new(self.pdc.clone(), request)
            .resolve_lock(retry_options.lock_backoff)
            .retry_multi_region(retry_options.region_backoff)
            .merge(Collect)
            .plan();
        plan.execute()
            .await
            .map(|r| r.into_iter().map(Into::into).collect())
    }
}

/// A caching layer which buffers reads and writes in a transaction.
pub struct Buffer {
    primary_key: Option<Key>,
    entry_map: BTreeMap<Key, BufferEntry>,
    is_pessimistic: bool,
}

impl Buffer {
    pub fn new(is_pessimistic: bool) -> Buffer {
        Buffer {
            primary_key: None,
            entry_map: BTreeMap::new(),
            is_pessimistic,
        }
    }

    /// Get the primary key of the buffer.
    pub fn get_primary_key(&self) -> Option<Key> {
        self.primary_key.clone()
    }

    /// Set the primary key if it is not set
    pub fn primary_key_or(&mut self, key: &Key) {
        self.primary_key.get_or_insert_with(|| key.clone());
    }

    /// Get a value from the buffer.
    /// If the returned value is None, it means the key doesn't exist in buffer yet.
    pub fn get(&self, key: &Key) -> Option<Value> {
        match self.get_from_mutations(key) {
            MutationValue::Determined(value) => value,
            MutationValue::Undetermined => None,
        }
    }

    /// Get a value from the buffer. If the value is not present, run `f` to get
    /// the value.
    pub async fn get_or_else<F, Fut>(&mut self, key: Key, f: F) -> Result<Option<Value>>
    where
        F: FnOnce(Key) -> Fut,
        Fut: Future<Output = Result<Option<Value>>>,
    {
        match self.get_from_mutations(&key) {
            MutationValue::Determined(value) => Ok(value),
            MutationValue::Undetermined => {
                let value = f(key.clone()).await?;
                self.update_cache(key, value.clone());
                Ok(value)
            }
        }
    }

    /// Get multiple values from the buffer. If any are not present, run `f` to
    /// get the missing values.
    ///
    /// only used for snapshot read (i.e. not for `batch_get_for_update`)
    pub async fn batch_get_or_else<F, Fut>(
        &mut self,
        keys: impl Iterator<Item = Key>,
        f: F,
    ) -> Result<impl Iterator<Item = KvPair>>
    where
        F: FnOnce(Box<dyn Iterator<Item = Key> + Send>) -> Fut,
        Fut: Future<Output = Result<Vec<KvPair>>>,
    {
        let (cached_results, undetermined_keys) = {
            // Partition the keys into those we have buffered and those we have to
            // get from the store.
            let (undetermined_keys, cached_results): (Vec<_>, Vec<_>) = keys
                .map(|key| {
                    let value = self
                        .entry_map
                        .get(&key)
                        .map(BufferEntry::get_value)
                        .unwrap_or(MutationValue::Undetermined);
                    (key, value)
                })
                .partition(|(_, v)| *v == MutationValue::Undetermined);

            let cached_results = cached_results
                .into_iter()
                .filter_map(|(k, v)| v.unwrap().map(|v| KvPair(k, v)));

            let undetermined_keys = undetermined_keys.into_iter().map(|(k, _)| k);
            (cached_results, undetermined_keys)
        };

        let fetched_results = f(Box::new(undetermined_keys)).await?;
        for kvpair in &fetched_results {
            let key = kvpair.0.clone();
            let value = Some(kvpair.1.clone());
            self.update_cache(key, value);
        }

        let results = cached_results.chain(fetched_results.into_iter());
        Ok(results)
    }

    /// Run `f` to fetch entries in `range` from TiKV. Combine them with mutations in local buffer. Returns the results.
    #[allow(clippy::too_many_arguments)]
    pub async fn scan_and_fetch<PdC: PdClient>(
        &mut self,
        pdc: Arc<PdC>,
        range: BoundRange,
        timestamp: Timestamp,
        batch_size: u32,
        limit: u32,
        update_cache: bool,
        reverse: bool,
        retry_options: RetryOptions,
    ) -> Result<impl Iterator<Item = KvPair>> {
        // read from local buffer
        let mutation_range = self.entry_map.range(range.clone());

        // fetch from TiKV
        // fetch more entries because some of them may be deleted.
        let redundant_limit = limit
            + mutation_range
                .clone()
                .filter(|(_, m)| matches!(m, BufferEntry::Del))
                .count() as u32;

        let scanner = Scanner::new(
            pdc,
            timestamp,
            range,
            batch_size,
            redundant_limit,
            reverse,
            !update_cache,
            retry_options,
        );

        let kv_stream = stream::unfold(scanner, |mut scanner| async move {
            // try read from cache
            // cache is consumed done, so we need to fetch new data from server, and fill the cache
            if scanner.current >= scanner.cache.len() {
                // scan ended, because cache is not full-filled in the last fetch
                if !scanner.cache.is_empty()
                    && scanner.cache.len() < scanner.batch_size.try_into().unwrap()
                {
                    return None;
                }
                // fill cache
                // generate new bound range
                let range = scanner.next_start_key.clone()..scanner.next_end_key.clone();
                scanner.range = range.into();
                if scanner.limit < scanner.batch_size {
                    scanner.batch_size = scanner.limit;
                };

                if let Ok(data) = scanner.fetch_data().await {
                    scanner.cache = data;
                } else {
                    return None;
                }

                if scanner.cache.is_empty() {
                    // scan ended
                    return None;
                }

                scanner.limit -= scanner.cache.len() as u32;
                // reset current index to start
                scanner.current = 0;

                // update bound range for next fetch
                let last_key = scanner.cache.last().map(|kv| kv.0.clone()).unwrap();
                if !scanner.reverse {
                    scanner.next_start_key = last_key.next_key();
                } else {
                    scanner.next_end_key = last_key;
                }
            }

            let ret = scanner.cache[scanner.current].clone();

            scanner.current += 1;
            Some((ret, scanner))
        });
        let kv_pairs = kv_stream.collect::<Vec<_>>().await;
        let mut results: HashMap<Key, Value> = kv_pairs.into_iter().map(|kv| kv.into()).collect();

        // override using local data
        for (k, m) in mutation_range {
            match m {
                BufferEntry::Put(v) => {
                    results.insert(k.clone(), v.clone());
                }
                BufferEntry::Del => {
                    results.remove(k);
                }
                _ => {}
            }
        }

        // update local buffer
        if update_cache {
            for (k, v) in &results {
                self.update_cache(k.clone(), Some(v.clone()));
            }
        }

        let mut res = results
            .into_iter()
            .map(|(k, v)| KvPair::new(k, v))
            .collect::<Vec<_>>();

        // TODO: use `BTreeMap` instead of `HashMap` to avoid sorting.
        if reverse {
            res.sort_unstable_by(|a, b| b.key().cmp(a.key()));
        } else {
            res.sort_unstable_by(|a, b| a.key().cmp(b.key()));
        }

        Ok(res.into_iter().take(limit as usize))
    }

    /// Lock the given key if necessary.
    pub fn lock(&mut self, key: Key) {
        self.primary_key.get_or_insert_with(|| key.clone());
        let value = self
            .entry_map
            .entry(key)
            // Mutated keys don't need a lock.
            .or_insert(BufferEntry::Locked(None));
        // But values which we have only read, but not written, do.
        if let BufferEntry::Cached(v) = value {
            *value = BufferEntry::Locked(Some(v.take()))
        }
    }

    /// Unlock the given key if locked.
    pub fn unlock(&mut self, key: &Key) {
        if let Some(value) = self.entry_map.get_mut(key) {
            if let BufferEntry::Locked(v) = value {
                if let Some(v) = v {
                    *value = BufferEntry::Cached(v.take());
                } else {
                    self.entry_map.remove(key);
                }
            }
        }
    }

    /// Put a value into the buffer (does not write through).
    pub fn put(&mut self, key: Key, value: Value) {
        let mut entry = self.entry_map.entry(key.clone());
        match entry {
            Entry::Occupied(ref mut o)
                if matches!(o.get(), BufferEntry::Insert(_))
                    || matches!(o.get(), BufferEntry::CheckNotExist) =>
            {
                o.insert(BufferEntry::Insert(value));
            }
            _ => self.insert_entry(key, BufferEntry::Put(value)),
        }
    }

    /// Mark a value as Insert mutation into the buffer (does not write through).
    pub fn insert(&mut self, key: Key, value: Value) {
        let mut entry = self.entry_map.entry(key.clone());
        match entry {
            Entry::Occupied(ref mut o) if matches!(o.get(), BufferEntry::Del) => {
                o.insert(BufferEntry::Put(value));
            }
            _ => self.insert_entry(key, BufferEntry::Insert(value)),
        }
    }

    /// Mark a value as deleted.
    pub fn delete(&mut self, key: Key) {
        let is_pessimistic = self.is_pessimistic;
        let mut entry = self.entry_map.entry(key.clone());

        match entry {
            Entry::Occupied(ref mut o)
                if !is_pessimistic
                    && (matches!(o.get(), BufferEntry::Insert(_))
                        || matches!(o.get(), BufferEntry::CheckNotExist)) =>
            {
                o.insert(BufferEntry::CheckNotExist);
            }
            _ => self.insert_entry(key, BufferEntry::Del),
        }
    }

    /// Converts the buffered mutations to the proto buffer version
    pub fn to_proto_mutations(&self) -> Vec<kvrpcpb::Mutation> {
        self.entry_map
            .iter()
            .filter_map(|(key, mutation)| mutation.to_proto_with_key(key))
            .collect()
    }

    pub fn get_write_size(&self) -> usize {
        self.entry_map
            .iter()
            .map(|(k, v)| match v {
                BufferEntry::Put(val) | BufferEntry::Insert(val) => val.len() + k.len(),
                BufferEntry::Del => k.len(),
                _ => 0,
            })
            .sum()
    }

    fn get_from_mutations(&self, key: &Key) -> MutationValue {
        self.entry_map
            .get(key)
            .map(BufferEntry::get_value)
            .unwrap_or(MutationValue::Undetermined)
    }

    fn update_cache(&mut self, key: Key, value: Option<Value>) {
        match self.entry_map.get(&key) {
            Some(BufferEntry::Locked(None)) => {
                self.entry_map.insert(key, BufferEntry::Locked(Some(value)));
            }
            None => {
                self.entry_map.insert(key, BufferEntry::Cached(value));
            }
            Some(BufferEntry::Cached(v)) | Some(BufferEntry::Locked(Some(v))) => {
                assert!(&value == v);
            }
            Some(BufferEntry::Put(v)) => assert!(value.as_ref() == Some(v)),
            Some(BufferEntry::Del) => {
                assert!(value.is_none());
            }
            Some(BufferEntry::Insert(v)) => assert!(value.as_ref() == Some(v)),
            Some(BufferEntry::CheckNotExist) => {
                assert!(value.is_none());
            }
        }
    }

    fn insert_entry(&mut self, key: impl Into<Key>, entry: BufferEntry) {
        let key = key.into();
        if !matches!(entry, BufferEntry::Cached(_) | BufferEntry::CheckNotExist) {
            self.primary_key.get_or_insert_with(|| key.clone());
        }
        self.entry_map.insert(key, entry);
    }
}

// The state of a key-value pair in the buffer.
// It includes two kinds of state:
//
// Mutations:
//   - `Put`
//   - `Del`
//   - `Insert`
//   - `CheckNotExist`, a constraint to ensure the key doesn't exist. See https://github.com/pingcap/tidb/pull/14968.
// Cache of read requests:
//   - `Cached`, generated by normal read requests
//   - `ReadLockCached`, generated by lock commands (`lock_keys`, `get_for_update`) and optionally read requests
//
#[derive(Debug, Clone)]
enum BufferEntry {
    // The value has been read from the server. None means there is no entry.
    // Also means the entry isn't locked.
    Cached(Option<Value>),
    // Key is locked.
    //
    // Cached value:
    //   - Outer Option: Whether there is cached value
    //   - Inner Option: Whether the value is empty
    //   - Note: The cache is not what the lock request reads, but what normal read (`get`) requests read.
    //
    // In optimistic transaction:
    //   The key is locked by `lock_keys`.
    //   It means letting the server check for conflicts when committing
    //
    // In pessimistic transaction:
    //   The key is locked by `get_for_update` or `batch_get_for_update`
    Locked(Option<Option<Value>>),
    // Value has been written.
    Put(Value),
    // Value has been deleted.
    Del,
    // Key should be check not exists before.
    Insert(Value),
    // Key should be check not exists before.
    CheckNotExist,
}

impl BufferEntry {
    fn to_proto_with_key(&self, key: &Key) -> Option<kvrpcpb::Mutation> {
        let mut pb = kvrpcpb::Mutation::default();
        match self {
            BufferEntry::Cached(_) => return None,
            BufferEntry::Put(v) => {
                pb.set_op(kvrpcpb::Op::Put);
                pb.set_value(v.clone());
            }
            BufferEntry::Del => pb.set_op(kvrpcpb::Op::Del),
            BufferEntry::Locked(_) => pb.set_op(kvrpcpb::Op::Lock),
            BufferEntry::Insert(v) => {
                pb.set_op(kvrpcpb::Op::Insert);
                pb.set_value(v.clone());
            }
            BufferEntry::CheckNotExist => pb.set_op(kvrpcpb::Op::CheckNotExists),
        };
        pb.set_key(key.clone().into());
        Some(pb)
    }

    fn get_value(&self) -> MutationValue {
        match self {
            BufferEntry::Cached(value) => MutationValue::Determined(value.clone()),
            BufferEntry::Put(value) => MutationValue::Determined(Some(value.clone())),
            BufferEntry::Del => MutationValue::Determined(None),
            BufferEntry::Locked(None) => MutationValue::Undetermined,
            BufferEntry::Locked(Some(value)) => MutationValue::Determined(value.clone()),
            BufferEntry::Insert(value) => MutationValue::Determined(Some(value.clone())),
            BufferEntry::CheckNotExist => MutationValue::Determined(None),
        }
    }
}

// The state of a value as known by the buffer.
#[derive(Eq, PartialEq, Debug)]
enum MutationValue {
    // The buffer can determine the value.
    Determined(Option<Value>),
    // The buffer cannot determine the value.
    Undetermined,
}

impl MutationValue {
    fn unwrap(self) -> Option<Value> {
        match self {
            MutationValue::Determined(v) => v,
            MutationValue::Undetermined => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{executor::block_on, future::ready};
    use tikv_client_common::internal_err;

    #[test]
    fn set_and_get_from_buffer() {
        let mut buffer = Buffer::new(false);
        buffer.put(b"key1".to_vec().into(), b"value1".to_vec());
        buffer.put(b"key2".to_vec().into(), b"value2".to_vec());
        assert_eq!(
            block_on(
                buffer.get_or_else(b"key1".to_vec().into(), move |_| ready(Err(internal_err!(
                    ""
                ))))
            )
            .unwrap()
            .unwrap(),
            b"value1".to_vec()
        );

        buffer.delete(b"key2".to_vec().into());
        buffer.put(b"key1".to_vec().into(), b"value".to_vec());
        assert_eq!(
            block_on(buffer.batch_get_or_else(
                vec![b"key2".to_vec().into(), b"key1".to_vec().into()].into_iter(),
                move |_| ready(Ok(vec![])),
            ))
            .unwrap()
            .collect::<Vec<_>>(),
            vec![KvPair(Key::from(b"key1".to_vec()), b"value".to_vec(),),]
        );
    }

    #[test]
    fn insert_and_get_from_buffer() {
        let mut buffer = Buffer::new(false);
        buffer.insert(b"key1".to_vec().into(), b"value1".to_vec());
        buffer.insert(b"key2".to_vec().into(), b"value2".to_vec());
        assert_eq!(
            block_on(
                buffer.get_or_else(b"key1".to_vec().into(), move |_| ready(Err(internal_err!(
                    ""
                ))))
            )
            .unwrap()
            .unwrap(),
            b"value1".to_vec()
        );

        buffer.delete(b"key2".to_vec().into());
        buffer.insert(b"key1".to_vec().into(), b"value".to_vec());
        assert_eq!(
            block_on(buffer.batch_get_or_else(
                vec![b"key2".to_vec().into(), b"key1".to_vec().into()].into_iter(),
                move |_| ready(Ok(vec![])),
            ))
            .unwrap()
            .collect::<Vec<_>>(),
            vec![KvPair(Key::from(b"key1".to_vec()), b"value".to_vec()),]
        );
    }

    #[test]
    fn repeat_reads_are_cached() {
        let k1: Key = b"key1".to_vec().into();
        let k1_ = k1.clone();
        let k2: Key = b"key2".to_vec().into();
        let k2_ = k2.clone();
        let v1: Value = b"value1".to_vec();
        let v1_ = v1.clone();
        let v1__ = v1.clone();
        let v2: Value = b"value2".to_vec();
        let v2_ = v2.clone();

        let mut buffer = Buffer::new(false);
        let r1 = block_on(buffer.get_or_else(k1.clone(), move |_| ready(Ok(Some(v1_)))));
        let r2 = block_on(buffer.get_or_else(k1.clone(), move |_| ready(Err(internal_err!("")))));
        assert_eq!(r1.unwrap().unwrap(), v1);
        assert_eq!(r2.unwrap().unwrap(), v1);

        let mut buffer = Buffer::new(false);
        let r1 = block_on(
            buffer.batch_get_or_else(vec![k1.clone(), k2.clone()].into_iter(), move |_| {
                ready(Ok(vec![(k1_, v1__).into(), (k2_, v2_).into()]))
            }),
        );
        let r2 = block_on(buffer.get_or_else(k2.clone(), move |_| ready(Err(internal_err!("")))));
        let r3 = block_on(
            buffer.batch_get_or_else(vec![k1.clone(), k2.clone()].into_iter(), move |_| {
                ready(Ok(vec![]))
            }),
        );
        assert_eq!(
            r1.unwrap().collect::<Vec<_>>(),
            vec![
                KvPair(k1.clone(), v1.clone()),
                KvPair(k2.clone(), v2.clone())
            ]
        );
        assert_eq!(r2.unwrap().unwrap(), v2);
        assert_eq!(
            r3.unwrap().collect::<Vec<_>>(),
            vec![KvPair(k1, v1), KvPair(k2, v2)]
        );
    }

    // Check that multiple writes to the same key combine in the correct way.
    #[test]
    fn state_machine() {
        let mut buffer = Buffer::new(false);

        macro_rules! assert_entry {
            ($key: ident, $p: pat) => {
                assert!(matches!(buffer.entry_map.get(&$key), Some(&$p),))
            };
        }

        macro_rules! assert_entry_none {
            ($key: ident) => {
                assert!(matches!(buffer.entry_map.get(&$key), None,))
            };
        }

        // Insert + Delete = CheckNotExists
        let key: Key = b"key1".to_vec().into();
        buffer.insert(key.clone(), b"value1".to_vec());
        buffer.delete(key.clone());
        assert_entry!(key, BufferEntry::CheckNotExist);

        // CheckNotExists + Delete = CheckNotExists
        buffer.delete(key.clone());
        assert_entry!(key, BufferEntry::CheckNotExist);

        // CheckNotExists + Put = Insert
        buffer.put(key.clone(), b"value2".to_vec());
        assert_entry!(key, BufferEntry::Insert(_));

        // Insert + Put = Insert
        let key: Key = b"key2".to_vec().into();
        buffer.insert(key.clone(), b"value1".to_vec());
        buffer.put(key.clone(), b"value2".to_vec());
        assert_entry!(key, BufferEntry::Insert(_));

        // Delete + Insert = Put
        let key: Key = b"key3".to_vec().into();
        buffer.delete(key.clone());
        buffer.insert(key.clone(), b"value1".to_vec());
        assert_entry!(key, BufferEntry::Put(_));

        // Lock + Unlock = None
        let key: Key = b"key4".to_vec().into();
        buffer.lock(key.clone());
        buffer.unlock(&key);
        assert_entry_none!(key);

        // Cached + Lock + Unlock = Cached
        let key: Key = b"key5".to_vec().into();
        let val: Value = b"value5".to_vec();
        let val_ = val.clone();
        let r = block_on(buffer.get_or_else(key.clone(), move |_| ready(Ok(Some(val_)))));
        assert_eq!(r.unwrap().unwrap(), val);
        buffer.lock(key.clone());
        buffer.unlock(&key);
        assert_entry!(key, BufferEntry::Cached(Some(_)));
        assert_eq!(
            block_on(buffer.get_or_else(key, move |_| ready(Err(internal_err!("")))))
                .unwrap()
                .unwrap(),
            val
        );
    }
}
