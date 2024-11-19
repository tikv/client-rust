// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::ops::{Bound, Range};

use serde_derive::{Deserialize, Serialize};

use crate::transaction::Mutation;
use crate::{proto::kvrpcpb, Key};
use crate::{BoundRange, KvPair};

pub const RAW_KEY_PREFIX: u8 = b'r';
pub const TXN_KEY_PREFIX: u8 = b'x';
pub const KEYSPACE_PREFIX_LEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Keyspace {
    Disable,
    Enable { keyspace_id: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyMode {
    Raw,
    Txn,
}

impl Keyspace {
    pub fn api_version(&self) -> kvrpcpb::ApiVersion {
        match self {
            Keyspace::Disable => kvrpcpb::ApiVersion::V1,
            Keyspace::Enable { .. } => kvrpcpb::ApiVersion::V2,
        }
    }
}

pub trait EncodeKeyspace {
    fn encode_keyspace(self, keyspace: Keyspace, key_mode: KeyMode) -> Self;
}

pub trait TruncateKeyspace {
    fn truncate_keyspace(self, keyspace: Keyspace) -> Self;
}

impl EncodeKeyspace for Key {
    fn encode_keyspace(mut self, keyspace: Keyspace, key_mode: KeyMode) -> Self {
        let prefix = match keyspace {
            Keyspace::Disable => {
                return self;
            }
            Keyspace::Enable { keyspace_id } => keyspace_prefix(keyspace_id, key_mode),
        };

        prepend_bytes(&mut self.0, &prefix);

        self
    }
}

impl EncodeKeyspace for KvPair {
    fn encode_keyspace(mut self, keyspace: Keyspace, key_mode: KeyMode) -> Self {
        self.0 = self.0.encode_keyspace(keyspace, key_mode);
        self
    }
}

impl EncodeKeyspace for BoundRange {
    fn encode_keyspace(mut self, keyspace: Keyspace, key_mode: KeyMode) -> Self {
        self.from = match self.from {
            Bound::Included(key) => Bound::Included(key.encode_keyspace(keyspace, key_mode)),
            Bound::Excluded(key) => Bound::Excluded(key.encode_keyspace(keyspace, key_mode)),
            Bound::Unbounded => {
                let key = Key::from(vec![]);
                Bound::Included(key.encode_keyspace(keyspace, key_mode))
            }
        };
        self.to = match self.to {
            Bound::Included(key) if !key.is_empty() => {
                Bound::Included(key.encode_keyspace(keyspace, key_mode))
            }
            Bound::Excluded(key) if !key.is_empty() => {
                Bound::Excluded(key.encode_keyspace(keyspace, key_mode))
            }
            _ => {
                let key = Key::from(vec![]);
                let keyspace = match keyspace {
                    Keyspace::Disable => Keyspace::Disable,
                    Keyspace::Enable { keyspace_id } => Keyspace::Enable {
                        keyspace_id: keyspace_id + 1,
                    },
                };
                Bound::Excluded(key.encode_keyspace(keyspace, key_mode))
            }
        };
        self
    }
}

impl EncodeKeyspace for Mutation {
    fn encode_keyspace(self, keyspace: Keyspace, key_mode: KeyMode) -> Self {
        match self {
            Mutation::Put(key, val) => Mutation::Put(key.encode_keyspace(keyspace, key_mode), val),
            Mutation::Delete(key) => Mutation::Delete(key.encode_keyspace(keyspace, key_mode)),
        }
    }
}

impl TruncateKeyspace for Key {
    fn truncate_keyspace(mut self, keyspace: Keyspace) -> Self {
        if let Keyspace::Disable = keyspace {
            return self;
        }

        pretruncate_bytes::<KEYSPACE_PREFIX_LEN>(&mut self.0);

        self
    }
}

impl TruncateKeyspace for KvPair {
    fn truncate_keyspace(mut self, keyspace: Keyspace) -> Self {
        self.0 = self.0.truncate_keyspace(keyspace);
        self
    }
}

impl TruncateKeyspace for Range<Key> {
    fn truncate_keyspace(mut self, keyspace: Keyspace) -> Self {
        self.start = self.start.truncate_keyspace(keyspace);
        self.end = self.end.truncate_keyspace(keyspace);
        self
    }
}

impl TruncateKeyspace for Vec<Range<Key>> {
    fn truncate_keyspace(mut self, keyspace: Keyspace) -> Self {
        for range in &mut self {
            take_mut::take(range, |range| range.truncate_keyspace(keyspace));
        }
        self
    }
}

impl TruncateKeyspace for Vec<KvPair> {
    fn truncate_keyspace(mut self, keyspace: Keyspace) -> Self {
        for pair in &mut self {
            take_mut::take(pair, |pair| pair.truncate_keyspace(keyspace));
        }
        self
    }
}

impl TruncateKeyspace for Vec<crate::proto::kvrpcpb::LockInfo> {
    fn truncate_keyspace(mut self, keyspace: Keyspace) -> Self {
        for lock in &mut self {
            take_mut::take(&mut lock.key, |key| {
                Key::from(key).truncate_keyspace(keyspace).into()
            });
            take_mut::take(&mut lock.primary_lock, |primary| {
                Key::from(primary).truncate_keyspace(keyspace).into()
            });
            for secondary in lock.secondaries.iter_mut() {
                take_mut::take(secondary, |secondary| {
                    Key::from(secondary).truncate_keyspace(keyspace).into()
                });
            }
        }
        self
    }
}

fn keyspace_prefix(keyspace_id: u32, key_mode: KeyMode) -> [u8; KEYSPACE_PREFIX_LEN] {
    let mut prefix = keyspace_id.to_be_bytes();
    prefix[0] = match key_mode {
        KeyMode::Raw => RAW_KEY_PREFIX,
        KeyMode::Txn => TXN_KEY_PREFIX,
    };
    prefix
}

fn prepend_bytes<const N: usize>(vec: &mut Vec<u8>, prefix: &[u8; N]) {
    unsafe {
        vec.reserve_exact(N);
        std::ptr::copy(vec.as_ptr(), vec.as_mut_ptr().add(N), vec.len());
        std::ptr::copy_nonoverlapping(prefix.as_ptr(), vec.as_mut_ptr(), N);
        vec.set_len(vec.len() + N);
    }
}

fn pretruncate_bytes<const N: usize>(vec: &mut Vec<u8>) {
    assert!(vec.len() >= N);
    unsafe {
        std::ptr::copy(vec.as_ptr().add(N), vec.as_mut_ptr(), vec.len() - N);
        vec.set_len(vec.len() - N);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyspace_prefix() {
        let key_mode = KeyMode::Raw;
        assert_eq!(keyspace_prefix(0, key_mode), [b'r', 0, 0, 0]);
        assert_eq!(keyspace_prefix(1, key_mode), [b'r', 0, 0, 1]);
        assert_eq!(keyspace_prefix(0xFFFF, key_mode), [b'r', 0, 0xFF, 0xFF]);

        let key_mode = KeyMode::Txn;
        assert_eq!(keyspace_prefix(0, key_mode), [b'x', 0, 0, 0]);
        assert_eq!(keyspace_prefix(1, key_mode), [b'x', 0, 0, 1]);
        assert_eq!(keyspace_prefix(0xFFFF, key_mode), [b'x', 0, 0xFF, 0xFF]);
    }

    #[test]
    fn test_encode_version() {
        let keyspace = Keyspace::Enable {
            keyspace_id: 0xDEAD,
        };
        let key_mode = KeyMode::Raw;

        let key = Key::from(vec![0xBE, 0xEF]);
        let expected_key = Key::from(vec![b'r', 0, 0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(key.encode_keyspace(keyspace, key_mode), expected_key);

        let bound: BoundRange = (Key::from(vec![0xDE, 0xAD])..Key::from(vec![0xBE, 0xEF])).into();
        let expected_bound: BoundRange = (Key::from(vec![b'r', 0, 0xDE, 0xAD, 0xDE, 0xAD])
            ..Key::from(vec![b'r', 0, 0xDE, 0xAD, 0xBE, 0xEF]))
            .into();
        assert_eq!(bound.encode_keyspace(keyspace, key_mode), expected_bound);

        let bound: BoundRange = (..).into();
        let expected_bound: BoundRange =
            (Key::from(vec![b'r', 0, 0xDE, 0xAD])..Key::from(vec![b'r', 0, 0xDE, 0xAE])).into();
        assert_eq!(bound.encode_keyspace(keyspace, key_mode), expected_bound);

        let bound: BoundRange = (Key::from(vec![])..Key::from(vec![])).into();
        let expected_bound: BoundRange =
            (Key::from(vec![b'r', 0, 0xDE, 0xAD])..Key::from(vec![b'r', 0, 0xDE, 0xAE])).into();
        assert_eq!(bound.encode_keyspace(keyspace, key_mode), expected_bound);

        let bound: BoundRange = (Key::from(vec![])..=Key::from(vec![])).into();
        let expected_bound: BoundRange =
            (Key::from(vec![b'r', 0, 0xDE, 0xAD])..Key::from(vec![b'r', 0, 0xDE, 0xAE])).into();
        assert_eq!(bound.encode_keyspace(keyspace, key_mode), expected_bound);

        let mutation = Mutation::Put(Key::from(vec![0xBE, 0xEF]), vec![4, 5, 6]);
        let expected_mutation = Mutation::Put(
            Key::from(vec![b'r', 0, 0xDE, 0xAD, 0xBE, 0xEF]),
            vec![4, 5, 6],
        );
        assert_eq!(
            mutation.encode_keyspace(keyspace, key_mode),
            expected_mutation
        );

        let mutation = Mutation::Delete(Key::from(vec![0xBE, 0xEF]));
        let expected_mutation = Mutation::Delete(Key::from(vec![b'r', 0, 0xDE, 0xAD, 0xBE, 0xEF]));
        assert_eq!(
            mutation.encode_keyspace(keyspace, key_mode),
            expected_mutation
        );
    }

    #[test]
    fn test_truncate_version() {
        let key = Key::from(vec![b'r', 0, 0xDE, 0xAD, 0xBE, 0xEF]);
        let keyspace = Keyspace::Enable {
            keyspace_id: 0xDEAD,
        };
        let expected_key = Key::from(vec![0xBE, 0xEF]);
        assert_eq!(key.truncate_keyspace(keyspace), expected_key);

        let key = Key::from(vec![b'x', 0, 0xDE, 0xAD, 0xBE, 0xEF]);
        let keyspace = Keyspace::Enable {
            keyspace_id: 0xDEAD,
        };
        let expected_key = Key::from(vec![0xBE, 0xEF]);
        assert_eq!(key.truncate_keyspace(keyspace), expected_key);
    }
}
