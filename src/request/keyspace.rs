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
#[non_exhaustive]
pub enum Keyspace {
    Disable,
    Enable {
        keyspace_id: u32,
    },
    /// Use API V2 without adding or removing the API V2 keyspace/key-mode prefix.
    ///
    /// This mode is intended for **server-side embedding** use cases (e.g. embedding this client in
    /// `tikv-server`) where keys are already in API V2 "logical key bytes" form and must be passed
    /// through unchanged.
    ApiV2NoPrefix,
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
            Keyspace::ApiV2NoPrefix => kvrpcpb::ApiVersion::V2,
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
        let Keyspace::Enable { keyspace_id } = keyspace else {
            return self;
        };
        let prefix = keyspace_prefix(keyspace_id, key_mode);

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
            Bound::Unbounded => Bound::Included(Key::EMPTY.encode_keyspace(keyspace, key_mode)),
        };

        self.to = match self.to {
            Bound::Included(key) if !key.is_empty() => {
                Bound::Included(key.encode_keyspace(keyspace, key_mode))
            }
            Bound::Excluded(key) if !key.is_empty() => {
                Bound::Excluded(key.encode_keyspace(keyspace, key_mode))
            }
            _ => match keyspace {
                Keyspace::Enable { keyspace_id } => Bound::Excluded(Key::EMPTY.encode_keyspace(
                    Keyspace::Enable {
                        keyspace_id: keyspace_id + 1,
                    },
                    key_mode,
                )),
                _ => Bound::Excluded(Key::EMPTY),
            },
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
        if !matches!(keyspace, Keyspace::Enable { .. }) {
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
        if !matches!(keyspace, Keyspace::Enable { .. }) {
            return self;
        }
        for range in &mut self {
            take_mut::take(range, |range| range.truncate_keyspace(keyspace));
        }
        self
    }
}

impl TruncateKeyspace for Vec<KvPair> {
    fn truncate_keyspace(mut self, keyspace: Keyspace) -> Self {
        if !matches!(keyspace, Keyspace::Enable { .. }) {
            return self;
        }
        for pair in &mut self {
            take_mut::take(pair, |pair| pair.truncate_keyspace(keyspace));
        }
        self
    }
}

/// Is this key field UNSET, as opposed to carrying the empty logical key?
///
/// An encoded key always carries its 4-byte keyspace prefix, so on the wire an empty
/// byte string means "not set" — never "the empty logical key", which encodes to the
/// bare prefix. Guarding on this keeps the codecs off `pretruncate_bytes`' length
/// assertion for fields a shared-lock wrapper leaves unset, and mirrors client-go's
/// `codecV2.DecodeKey`, which returns early on a zero-length key.
fn is_unset_key(key: &[u8]) -> bool {
    key.is_empty()
}

impl TruncateKeyspace for Vec<crate::proto::kvrpcpb::LockInfo> {
    fn truncate_keyspace(mut self, keyspace: Keyspace) -> Self {
        if !matches!(keyspace, Keyspace::Enable { .. }) {
            return self;
        }
        for lock in &mut self {
            // Convert this lock's OWN key fields, then recurse into any shared-lock
            // members — the shape of client-go's `codecV2.decodeLockInfo`, which
            // decodes Key/PrimaryLock/Secondaries and *then* walks SharedLockInfos,
            // with no wrapper special case.
            //
            // Per TiKV's writer (`SharedLocks::into_lock_info`, txn_types/src/lock.rs)
            // a wrapper sets `lock_type`, `shared_lock_infos` and `key` — the key it
            // locks — and leaves `primary_lock`/`lock_version` at their defaults. Each
            // member is built from that SAME raw key plus its own primary and version.
            // So the wrapper's key must be converted like any other (skipping it would
            // hand `scan_locks` a physical key beside decoded member keys), while its
            // unset fields must be left alone — hence no wrapper special case, just the
            // per-field guard below.
            if !is_unset_key(&lock.key) {
                take_mut::take(&mut lock.key, |key| {
                    Key::from(key).truncate_keyspace(keyspace).into()
                });
            }
            if !is_unset_key(&lock.primary_lock) {
                take_mut::take(&mut lock.primary_lock, |primary| {
                    Key::from(primary).truncate_keyspace(keyspace).into()
                });
            }
            for secondary in lock.secondaries.iter_mut() {
                // Unlike `key`/`primary_lock` above, this guard is not an unset-field
                // case: an unset `secondaries` is an empty Vec, with no elements to
                // visit. A present-but-empty ELEMENT is malformed input (an encoded
                // key always carries its prefix); skipping it merely keeps a corrupt
                // entry from panicking the codec on `pretruncate_bytes`' length
                // assertion.
                if is_unset_key(secondary) {
                    continue;
                }
                take_mut::take(secondary, |secondary| {
                    Key::from(secondary).truncate_keyspace(keyspace).into()
                });
            }
            take_mut::take(&mut lock.shared_lock_infos, |members| {
                members.truncate_keyspace(keyspace)
            });
        }
        self
    }
}

impl EncodeKeyspace for Vec<crate::proto::kvrpcpb::LockInfo> {
    fn encode_keyspace(mut self, keyspace: Keyspace, key_mode: KeyMode) -> Self {
        if !matches!(keyspace, Keyspace::Enable { .. }) {
            return self;
        }
        for lock in &mut self {
            // Deliberately NOT symmetric with the TruncateKeyspace impl above. There,
            // empty bytes can only mean "unset", because an encoded key always carries
            // its 4-byte prefix. Here the input is a LOGICAL key, and the empty logical
            // key is valid in API v2 — it encodes to the bare prefix. Skipping empties
            // would strand a lock on the empty key with no prefix, and `scan_locks` ->
            // `resolve_locks` (transaction/client.rs) round-trips exactly that way, so
            // its region lookup would then use an empty physical key.
            //
            // Shared-lock wrappers do not need the unset-field guard here: resolution
            // refuses them (`reject_shared_locks`) before anything acts on the encoded
            // result.
            take_mut::take(&mut lock.key, |key| {
                Key::from(key).encode_keyspace(keyspace, key_mode).into()
            });
            take_mut::take(&mut lock.primary_lock, |primary| {
                Key::from(primary)
                    .encode_keyspace(keyspace, key_mode)
                    .into()
            });
            for secondary in lock.secondaries.iter_mut() {
                take_mut::take(secondary, |secondary| {
                    Key::from(secondary)
                        .encode_keyspace(keyspace, key_mode)
                        .into()
                });
            }
            take_mut::take(&mut lock.shared_lock_infos, |members| {
                members.encode_keyspace(keyspace, key_mode)
            });
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

        let key_mode = KeyMode::Txn;
        let lock = crate::proto::kvrpcpb::LockInfo {
            key: vec![b'k', b'1'],
            primary_lock: vec![b'p', b'1'],
            secondaries: vec![vec![b's', b'1'], vec![b's', b'2']],
            ..Default::default()
        };
        let locks = vec![lock].encode_keyspace(keyspace, key_mode);
        assert_eq!(locks.len(), 1);
        assert_eq!(locks[0].key, vec![b'x', 0, 0xDE, 0xAD, b'k', b'1']);
        assert_eq!(locks[0].primary_lock, vec![b'x', 0, 0xDE, 0xAD, b'p', b'1']);
        assert_eq!(
            locks[0].secondaries,
            vec![
                vec![b'x', 0, 0xDE, 0xAD, b's', b'1'],
                vec![b'x', 0, 0xDE, 0xAD, b's', b'2']
            ]
        );
    }

    #[test]
    fn test_truncate_version() {
        let keyspace = Keyspace::Enable {
            keyspace_id: 0xDEAD,
        };

        let key = Key::from(vec![b'r', 0, 0xDE, 0xAD, 0xBE, 0xEF]);
        let expected_key = Key::from(vec![0xBE, 0xEF]);
        assert_eq!(key.truncate_keyspace(keyspace), expected_key);

        let key = Key::from(vec![b'x', 0, 0xDE, 0xAD, 0xBE, 0xEF]);
        let expected_key = Key::from(vec![0xBE, 0xEF]);
        assert_eq!(key.truncate_keyspace(keyspace), expected_key);

        let pair = KvPair(Key::from(vec![b'x', 0, 0xDE, 0xAD, b'k']), vec![b'v']);
        let expected_pair = KvPair(Key::from(vec![b'k']), vec![b'v']);
        assert_eq!(pair.truncate_keyspace(keyspace), expected_pair);

        let range = Range {
            start: Key::from(vec![b'x', 0, 0xDE, 0xAD, b'a']),
            end: Key::from(vec![b'x', 0, 0xDE, 0xAD, b'b']),
        };
        let expected_range = Range {
            start: Key::from(vec![b'a']),
            end: Key::from(vec![b'b']),
        };
        assert_eq!(range.truncate_keyspace(keyspace), expected_range);

        let ranges = vec![
            Range {
                start: Key::from(vec![b'x', 0, 0xDE, 0xAD, b'a']),
                end: Key::from(vec![b'x', 0, 0xDE, 0xAD, b'b']),
            },
            Range {
                start: Key::from(vec![b'x', 0, 0xDE, 0xAD, b'c']),
                end: Key::from(vec![b'x', 0, 0xDE, 0xAD, b'd']),
            },
        ];
        let expected_ranges = vec![
            Range {
                start: Key::from(vec![b'a']),
                end: Key::from(vec![b'b']),
            },
            Range {
                start: Key::from(vec![b'c']),
                end: Key::from(vec![b'd']),
            },
        ];
        assert_eq!(ranges.truncate_keyspace(keyspace), expected_ranges);

        let pairs = vec![
            KvPair(Key::from(vec![b'x', 0, 0xDE, 0xAD, b'k']), vec![b'v']),
            KvPair(
                Key::from(vec![b'x', 0, 0xDE, 0xAD, b'k', b'2']),
                vec![b'v', b'2'],
            ),
        ];
        let expected_pairs = vec![
            KvPair(Key::from(vec![b'k']), vec![b'v']),
            KvPair(Key::from(vec![b'k', b'2']), vec![b'v', b'2']),
        ];
        assert_eq!(pairs.truncate_keyspace(keyspace), expected_pairs);

        let lock = crate::proto::kvrpcpb::LockInfo {
            key: vec![b'x', 0, 0xDE, 0xAD, b'k'],
            primary_lock: vec![b'x', 0, 0xDE, 0xAD, b'p'],
            secondaries: vec![vec![b'x', 0, 0xDE, 0xAD, b's']],
            ..Default::default()
        };
        let expected_lock = crate::proto::kvrpcpb::LockInfo {
            key: vec![b'k'],
            primary_lock: vec![b'p'],
            secondaries: vec![vec![b's']],
            ..Default::default()
        };
        assert_eq!(vec![lock].truncate_keyspace(keyspace), vec![expected_lock]);
    }

    #[test]
    fn test_apiv2_no_prefix_api_version() {
        assert_eq!(
            Keyspace::ApiV2NoPrefix.api_version(),
            kvrpcpb::ApiVersion::V2
        );
    }

    #[test]
    fn test_apiv2_no_prefix_encode_is_noop() {
        let keyspace = Keyspace::ApiV2NoPrefix;
        let key_mode = KeyMode::Txn;

        let key = Key::from(vec![b'x', 0, 0, 0, b'k']);
        assert_eq!(key.clone().encode_keyspace(keyspace, key_mode), key);

        let pair = KvPair(Key::from(vec![b'x', 0, 0, 0, b'k']), vec![b'v']);
        assert_eq!(pair.clone().encode_keyspace(keyspace, key_mode), pair);

        let bound: BoundRange =
            (Key::from(vec![b'x', 0, 0, 0, b'a'])..Key::from(vec![b'x', 0, 0, 0, b'b'])).into();
        assert_eq!(bound.clone().encode_keyspace(keyspace, key_mode), bound);

        let mutation = Mutation::Put(Key::from(vec![b'x', 0, 0, 0, b'k']), vec![1, 2, 3]);
        assert_eq!(
            mutation.clone().encode_keyspace(keyspace, key_mode),
            mutation
        );

        let lock = crate::proto::kvrpcpb::LockInfo {
            key: vec![b'x', 0, 0, 0, b'k'],
            primary_lock: vec![b'x', 0, 0, 0, b'p'],
            secondaries: vec![vec![b'x', 0, 0, 0, b's']],
            ..Default::default()
        };
        let locks = vec![lock];
        assert_eq!(locks.clone().encode_keyspace(keyspace, key_mode), locks);

        let lock = crate::proto::kvrpcpb::LockInfo {
            key: vec![b'k', b'1'],
            primary_lock: vec![b'p', b'1'],
            secondaries: vec![vec![b's', b'1']],
            ..Default::default()
        };
        let locks = vec![lock.clone()];
        assert_eq!(
            locks.clone().encode_keyspace(Keyspace::Disable, key_mode),
            locks
        );
    }

    #[test]
    fn test_apiv2_no_prefix_truncate_is_noop() {
        let keyspace = Keyspace::ApiV2NoPrefix;

        let key = Key::from(vec![b'x', 0, 0, 0, b'k']);
        assert_eq!(key.clone().truncate_keyspace(keyspace), key);

        let pair = KvPair(Key::from(vec![b'x', 0, 0, 0, b'k']), vec![b'v']);
        assert_eq!(pair.clone().truncate_keyspace(keyspace), pair);

        let range = Range {
            start: Key::from(vec![b'x', 0, 0, 0, b'a']),
            end: Key::from(vec![b'x', 0, 0, 0, b'b']),
        };
        assert_eq!(range.clone().truncate_keyspace(keyspace), range);

        let pairs = vec![pair];
        assert_eq!(pairs.clone().truncate_keyspace(keyspace), pairs);

        let lock = crate::proto::kvrpcpb::LockInfo {
            key: vec![b'x', 0, 0, 0, b'k'],
            primary_lock: vec![b'x', 0, 0, 0, b'p'],
            secondaries: vec![vec![b'x', 0, 0, 0, b's']],
            ..Default::default()
        };
        let locks = vec![lock];
        assert_eq!(locks.clone().truncate_keyspace(keyspace), locks);
    }

    /// A shared-lock wrapper carries the key it locks, and its members are built from
    /// that same raw key (TiKV's `SharedLocks::into_lock_info`). Both must be converted:
    /// skipping the wrapper would hand `scan_locks` a physical key beside decoded member
    /// keys. The wrapper's *other* fields are a different matter — see
    /// [`unset_lock_key_fields_survive_truncation`].
    #[test]
    fn shared_lock_wrapper_keys_are_converted_alongside_their_members() {
        use crate::proto::kvrpcpb::{LockInfo, Op};
        let keyspace = Keyspace::Enable { keyspace_id: 0 };

        let wrapper = LockInfo {
            key: vec![b'x', 0, 0, 0, b'k'],
            lock_type: Op::SharedLock as i32,
            shared_lock_infos: vec![LockInfo {
                key: vec![b'x', 0, 0, 0, b'm'],
                primary_lock: vec![b'x', 0, 0, 0, b'p'],
                lock_version: 8,
                ..Default::default()
            }],
            ..Default::default()
        };

        let out = vec![wrapper].truncate_keyspace(keyspace);
        assert_eq!(
            out[0].key,
            vec![b'k'],
            "the wrapper's own key must be decoded, not left physical"
        );
        assert_eq!(out[0].shared_lock_infos[0].key, vec![b'm']);
        assert_eq!(out[0].shared_lock_infos[0].primary_lock, vec![b'p']);

        let back = out.encode_keyspace(keyspace, KeyMode::Txn);
        assert_eq!(back[0].key, vec![b'x', 0, 0, 0, b'k']);
        assert_eq!(back[0].shared_lock_infos[0].key, vec![b'x', 0, 0, 0, b'm']);
    }

    /// The empty logical key is VALID in API v2: it encodes to the bare keyspace
    /// prefix. `scan_locks` -> `resolve_locks` round-trips locks through
    /// truncate-then-encode, so a lock on the empty key must regain its prefix —
    /// otherwise resolution would look up the region for an empty physical key.
    /// This is why the encode side does not share the truncate side's unset guard.
    #[test]
    fn a_lock_on_the_empty_logical_key_round_trips() {
        use crate::proto::kvrpcpb::LockInfo;
        let keyspace = Keyspace::Enable { keyspace_id: 0 };

        let physical = vec![LockInfo {
            key: vec![b'x', 0, 0, 0],
            primary_lock: vec![b'x', 0, 0, 0],
            ..Default::default()
        }];
        let logical = physical.clone().truncate_keyspace(keyspace);
        assert!(logical[0].key.is_empty(), "the empty logical key");

        let back = logical.encode_keyspace(keyspace, KeyMode::Txn);
        assert_eq!(
            back, physical,
            "a lock on the empty logical key must regain its keyspace prefix"
        );
    }

    /// TiKV's `SharedLocks::into_lock_info` leaves a wrapper's `primary_lock` at its
    /// default, so the codec meets genuinely unset fields in practice — they must be
    /// skipped, not run into `pretruncate_bytes`' length assertion.
    #[test]
    fn unset_lock_key_fields_survive_truncation() {
        use crate::proto::kvrpcpb::{LockInfo, Op};
        let keyspace = Keyspace::Enable { keyspace_id: 0 };

        // The writer's actual shape: key and members set, primary_lock left default.
        let realistic = LockInfo {
            key: vec![b'x', 0, 0, 0, b'k'],
            lock_type: Op::SharedLock as i32,
            shared_lock_infos: vec![LockInfo {
                key: vec![b'x', 0, 0, 0, b'k'],
                primary_lock: vec![b'x', 0, 0, 0, b'p'],
                ..Default::default()
            }],
            ..Default::default()
        };
        let out = vec![realistic].truncate_keyspace(keyspace);
        assert_eq!(out[0].key, vec![b'k']);
        assert!(
            out[0].primary_lock.is_empty(),
            "the wrapper's unset primary must be skipped, not truncated"
        );
        assert_eq!(out[0].shared_lock_infos[0].primary_lock, vec![b'p']);

        // Defensively, a wrapper with no key at all must not panic either.
        let keyless = LockInfo {
            lock_type: Op::SharedLock as i32,
            ..Default::default()
        };
        let out = vec![keyless].truncate_keyspace(keyspace);
        assert!(out[0].key.is_empty());
    }

    /// An empty element INSIDE `secondaries` is a different case from the unset
    /// fields above: an unset `secondaries` is an empty Vec with no elements, so a
    /// present-but-empty element can only be malformed input. It is tolerated —
    /// skipped rather than run into `pretruncate_bytes`' length assertion — while
    /// the well-formed elements beside it still convert.
    #[test]
    fn a_malformed_empty_secondary_is_tolerated_not_truncated() {
        use crate::proto::kvrpcpb::LockInfo;
        let keyspace = Keyspace::Enable { keyspace_id: 0 };

        let malformed = LockInfo {
            key: vec![b'x', 0, 0, 0, b'k'],
            secondaries: vec![vec![], vec![b'x', 0, 0, 0, b's']],
            ..Default::default()
        };
        let out = vec![malformed].truncate_keyspace(keyspace);
        assert_eq!(out[0].secondaries, vec![vec![], vec![b's']]);
    }
}
