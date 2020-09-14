// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use kvproto::kvrpcpb::KvPair;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone)]
pub struct KvStore {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl Default for KvStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KvStore {
    pub fn new() -> KvStore {
        KvStore {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn raw_get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let data = self.data.read().unwrap();
        data.get(key).map(|v| v.to_vec())
    }

    pub fn raw_batch_get(&self, keys: Vec<Vec<u8>>) -> Vec<KvPair> {
        let data = self.data.read().unwrap();
        keys.into_iter()
            .filter_map(|key| {
                if data.contains_key(&key) {
                    let mut pair = KvPair::default();
                    pair.set_value(data.get(&key).unwrap().to_vec());
                    pair.set_key(key);
                    Some(pair)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn raw_put(&self, key: Vec<u8>, value: Vec<u8>) {
        let mut data = self.data.write().unwrap();
        data.insert(key, value);
    }

    pub fn raw_batch_put(&self, pairs: Vec<KvPair>) {
        let mut data = self.data.write().unwrap();
        data.extend(
            pairs
                .into_iter()
                .map(|mut pair| (pair.take_key(), pair.take_value())),
        );
    }

    // if success, return the key deleted
    pub fn raw_delete(&self, key: &[u8]) {
        let mut data = self.data.write().unwrap();
        data.remove(key);
    }

    // if any of the key does not exist, return non-existent keys; delete other keys
    pub fn raw_batch_delete(&self, keys: Vec<Vec<u8>>) {
        let mut data = self.data.write().unwrap();
        keys.iter().for_each(|k| {
            data.remove(k);
        });
    }
}
