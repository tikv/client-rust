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

    pub fn raw_get(&self, key: &[u8]) -> Vec<u8> {
        let data = self.data.read().unwrap();
        data.get(key).map(|v| v.to_vec()).unwrap_or_else(Vec::new)
    }

    pub fn raw_batch_get(&self, keys: &[Vec<u8>]) -> Vec<KvPair> {
        let data = self.data.read().unwrap();
        keys.iter()
            .map(|key| {
                let mut pair = KvPair::default();
                pair.set_value(data.get(key).map(|v| v.to_vec()).unwrap_or_else(Vec::new));
                pair.set_key(key.to_vec());
                pair
            })
            .collect()
    }

    pub fn raw_put(&self, key: Vec<u8>, value: Vec<u8>) {
        let mut data = self.data.write().unwrap();
        data.insert(key, value);
    }

    pub fn raw_batch_put(&self, pairs: &[KvPair]) {
        let mut data = self.data.write().unwrap();
        for pair in pairs {
            data.insert(pair.get_key().to_vec(), pair.get_value().to_vec());
        }
    }

    // if success, return the key deleted
    pub fn raw_delete(&self, key: &[u8]) -> Option<Vec<u8>> {
        let mut data = self.data.write().unwrap();
        data.remove(key)
    }

    // if any of the key does not exist, return non-existent keys; delete other keys
    pub fn raw_batch_delete(&self, keys: &[Vec<u8>]) -> Result<(), Vec<Vec<u8>>> {
        let mut data = self.data.write().unwrap();
        let non_exist: Vec<_> = keys
            .iter()
            .filter_map(|k| data.remove(k).xor(Some(k.to_vec())))
            .collect();
        if non_exist.is_empty() {
            Ok(())
        } else {
            Err(non_exist)
        }
    }
}
