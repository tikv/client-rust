// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{BoundRange, Key, KvPair, Result, Value};
use std::{
    collections::{BTreeMap, HashMap},
    future::Future,
};
use tikv_client_proto::kvrpcpb;
use tokio::sync::Mutex;

/// A caching layer which buffers reads and writes in a transaction.
#[derive(Default)]
pub struct Buffer {
    mutations: Mutex<BTreeMap<Key, Mutation>>,
}

impl Buffer {
    /// Get a value from the buffer. If the value is not present, run `f` to get
    /// the value.
    pub async fn get_or_else<F, Fut>(&self, key: Key, f: F) -> Result<Option<Value>>
    where
        F: FnOnce(Key) -> Fut,
        Fut: Future<Output = Result<Option<Value>>>,
    {
        match self.get_from_mutations(&key).await {
            MutationValue::Determined(value) => Ok(value),
            MutationValue::Undetermined => {
                let value = f(key.clone()).await?;
                let mut mutations = self.mutations.lock().await;
                mutations.insert(key, Mutation::Cached(value.clone()));
                Ok(value)
            }
        }
    }

    /// Get multiple values from the buffer. If any are not present, run `f` to
    /// get the missing values.
    pub async fn batch_get_or_else<F, Fut>(
        &self,
        keys: impl Iterator<Item = Key>,
        f: F,
    ) -> Result<impl Iterator<Item = KvPair>>
    where
        F: FnOnce(Vec<Key>) -> Fut,
        Fut: Future<Output = Result<Vec<KvPair>>>,
    {
        let (cached_results, undetermined_keys) = {
            let mutations = self.mutations.lock().await;
            // Partition the keys into those we have buffered and those we have to
            // get from the store.
            let (undetermined_keys, cached_results): (
                Vec<(Key, MutationValue)>,
                Vec<(Key, MutationValue)>,
            ) = keys
                .map(|key| {
                    let value = mutations
                        .get(&key)
                        .map(Mutation::get_value)
                        .unwrap_or(MutationValue::Undetermined);
                    (key, value)
                })
                .partition(|(_, v)| *v == MutationValue::Undetermined);

            let cached_results = cached_results
                .into_iter()
                .filter_map(|(k, v)| v.unwrap().map(|v| KvPair(k, v)));

            let undetermined_keys = undetermined_keys.into_iter().map(|(k, _)| k).collect();
            (cached_results, undetermined_keys)
        };

        let fetched_results = f(undetermined_keys).await?;
        let mut mutations = self.mutations.lock().await;
        for pair in &fetched_results {
            mutations.insert(pair.0.clone(), Mutation::Cached(Some(pair.1.clone())));
        }

        let results = cached_results.chain(fetched_results.into_iter());
        Ok(results)
    }

    /// Run `f` to fetch entries in `range` from TiKV. Combine them with mutations in local buffer. Returns the results.
    pub async fn scan_and_fetch<F, Fut>(
        &self,
        range: BoundRange,
        limit: u32,
        f: F,
    ) -> Result<impl Iterator<Item = KvPair>>
    where
        F: FnOnce(BoundRange, u32) -> Fut,
        Fut: Future<Output = Result<Vec<KvPair>>>,
    {
        // read from local buffer
        let mut mutations = self.mutations.lock().await;
        let mutation_range = mutations.range(range.clone());

        // fetch from TiKV
        // fetch more entries because some of them may be deleted.
        let redundant_limit = limit
            + mutation_range
                .clone()
                .filter(|(_, m)| matches!(m, Mutation::Del))
                .count() as u32;

        let mut results = f(range, redundant_limit)
            .await?
            .into_iter()
            .map(|pair| pair.into())
            .collect::<HashMap<Key, Value>>();

        // override using local data
        for (k, m) in mutation_range {
            match m {
                Mutation::Put(v) => {
                    results.insert(k.clone(), v.clone());
                }
                Mutation::Del => {
                    results.remove(k);
                }
                _ => {}
            }
        }

        // update local buffer
        for (k, v) in &results {
            mutations.insert(k.clone(), Mutation::Cached(Some(v.clone())));
        }

        let mut res = results
            .into_iter()
            .map(|(k, v)| KvPair::new(k, v))
            .collect::<Vec<_>>();
        res.sort_by_cached_key(|x| x.key().clone());

        Ok(res.into_iter().take(limit as usize))
    }

    /// Lock the given key if necessary.
    pub async fn lock(&self, key: Key) {
        let mut mutations = self.mutations.lock().await;
        let value = mutations
            .entry(key)
            // Mutated keys don't need a lock.
            .or_insert(Mutation::Lock);
        // But values which we have only read, but not written, do.
        if let Mutation::Cached(_) = value {
            *value = Mutation::Lock
        }
    }

    /// Insert a value into the buffer (does not write through).
    pub async fn put(&self, key: Key, value: Value) {
        self.mutations
            .lock()
            .await
            .insert(key, Mutation::Put(value));
    }

    /// Mark a value as deleted.
    pub async fn delete(&self, key: Key) {
        self.mutations.lock().await.insert(key, Mutation::Del);
    }

    /// Converts the buffered mutations to the proto buffer version
    pub async fn to_proto_mutations(&self) -> Vec<kvrpcpb::Mutation> {
        self.mutations
            .lock()
            .await
            .iter()
            .filter_map(|(key, mutation)| mutation.to_proto_with_key(key))
            .collect()
    }

    async fn get_from_mutations(&self, key: &Key) -> MutationValue {
        self.mutations
            .lock()
            .await
            .get(&key)
            .map(Mutation::get_value)
            .unwrap_or(MutationValue::Undetermined)
    }
}

// The state of a value in the buffer.
#[derive(Debug, Clone)]
enum Mutation {
    // The value has been read from the server. None means there is no entry.
    Cached(Option<Value>),
    // Value has been written.
    Put(Value),
    // Value has been deleted.
    Del,
    // Key has been locked.
    Lock,
}

impl Mutation {
    fn to_proto_with_key(&self, key: &Key) -> Option<kvrpcpb::Mutation> {
        let mut pb = kvrpcpb::Mutation::default();
        match self {
            Mutation::Cached(_) => return None,
            Mutation::Put(v) => {
                pb.set_op(kvrpcpb::Op::Put);
                pb.set_value(v.clone());
            }
            Mutation::Del => pb.set_op(kvrpcpb::Op::Del),
            Mutation::Lock => pb.set_op(kvrpcpb::Op::Lock),
        };
        pb.set_key(key.clone().into());
        Some(pb)
    }

    fn get_value(&self) -> MutationValue {
        match self {
            Mutation::Cached(value) => MutationValue::Determined(value.clone()),
            Mutation::Put(value) => MutationValue::Determined(Some(value.clone())),
            Mutation::Del => MutationValue::Determined(None),
            Mutation::Lock => MutationValue::Undetermined,
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

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn set_and_get_from_buffer() {
        let buffer = Buffer::default();
        buffer
            .put(b"key1".to_vec().into(), b"value1".to_vec().into())
            .await;
        buffer
            .put(b"key2".to_vec().into(), b"value2".to_vec().into())
            .await;
        assert_eq!(
            block_on(buffer.get_or_else(b"key1".to_vec().into(), move |_| ready(panic!())))
                .unwrap()
                .unwrap(),
            b"value1".to_vec()
        );

        buffer.delete(b"key2".to_vec().into()).await;
        buffer
            .put(b"key1".to_vec().into(), b"value".to_vec().into())
            .await;
        assert_eq!(
            block_on(buffer.batch_get_or_else(
                vec![b"key2".to_vec().into(), b"key1".to_vec().into()].into_iter(),
                move |_| ready(Ok(vec![])),
            ))
            .unwrap()
            .collect::<Vec<_>>(),
            vec![KvPair(
                Key::from(b"key1".to_vec()),
                Value::from(b"value".to_vec())
            ),]
        );
    }

    #[test]
    #[allow(unreachable_code)]
    fn repeat_reads_are_cached() {
        let k1: Key = b"key1".to_vec().into();
        let k1_ = k1.clone();
        let k2: Key = b"key2".to_vec().into();
        let k2_ = k2.clone();
        let v1: Value = b"value1".to_vec().into();
        let v1_ = v1.clone();
        let v1__ = v1.clone();
        let v2: Value = b"value2".to_vec().into();
        let v2_ = v2.clone();

        let buffer = Buffer::default();
        let r1 = block_on(buffer.get_or_else(k1.clone(), move |_| ready(Ok(Some(v1_)))));
        let r2 = block_on(buffer.get_or_else(k1.clone(), move |_| ready(panic!())));
        assert_eq!(r1.unwrap().unwrap(), v1.clone());
        assert_eq!(r2.unwrap().unwrap(), v1.clone());

        let buffer = Buffer::default();
        let r1 = block_on(
            buffer.batch_get_or_else(vec![k1.clone(), k2.clone()].into_iter(), move |_| {
                ready(Ok(vec![(k1_, v1__).into(), (k2_, v2_).into()]))
            }),
        );
        let r2 = block_on(buffer.get_or_else(k2.clone(), move |_| ready(panic!())));
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
        assert_eq!(r2.unwrap().unwrap(), v2.clone());
        assert_eq!(
            r3.unwrap().collect::<Vec<_>>(),
            vec![
                KvPair(k1.clone(), v1.clone()),
                KvPair(k2.clone(), v2.clone())
            ]
        );
    }
}
