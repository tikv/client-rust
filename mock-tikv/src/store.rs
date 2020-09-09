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
        data.get(key).unwrap_or(&vec![]).to_vec()
    }

    pub fn raw_batch_get(&self, keys: &[Vec<u8>]) -> Vec<KvPair> {
        let data = self.data.read().unwrap();
        let mut pairs = vec![];
        for key in keys {
            let mut pair = KvPair::default();
            pair.set_value(data.get(key).unwrap_or(&vec![]).to_vec());
            pair.set_key(key.to_vec());
            pairs.push(pair);
        }
        pairs
    }

    pub fn raw_put(&self, key: &[u8], value: &[u8]) {
        let mut data = self.data.write().unwrap();
        *data.entry(key.to_vec()).or_default() = value.to_vec();
    }

    pub fn raw_batch_put(&self, pairs: &[KvPair]) {
        let mut data = self.data.write().unwrap();
        for pair in pairs {
            *data.entry(pair.get_key().to_vec()).or_default() = pair.get_value().to_vec();
        }
    }

    // if success, return the key deleted
    pub fn raw_delete(&self, key: &[u8]) -> Result<Vec<u8>, ()> {
        let mut data = self.data.write().unwrap();
        data.remove(key).ok_or(())
    }

    // if any of the key does not exist, return non-existent keys
    pub fn raw_batch_delete<'a>(&self, keys: &'a [Vec<u8>]) -> Result<(), Vec<&'a str>> {
        let mut data = self.data.write().unwrap();
        let mut non_exist_keys = vec![];
        keys.iter()
            .filter(|&key| !data.contains_key(key))
            .for_each(|key| non_exist_keys.push(std::str::from_utf8(key).unwrap()));
        if !non_exist_keys.is_empty() {
            Err(non_exist_keys)
        } else {
            keys.iter().for_each(|key| {
                data.remove(key).unwrap();
            });
            Ok(())
        }
    }
}
