// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::ops::{Bound, Range};

use serde_derive::{Deserialize, Serialize};

use crate::transaction::Mutation;
use crate::{proto::apipb, proto::keyspacepb, proto::kvrpcpb, Key};
use crate::{BoundRange, KvPair};

pub const RAW_KEY_PREFIX: u8 = b'r';
pub const TXN_KEY_PREFIX: u8 = b'x';
pub const KEYSPACE_PREFIX_LEN: usize = 4;
pub const API_V3_PREFIX_LEN: usize = 8;
pub const API_V3_MAX_KEYSPACE_ID: u32 = 0xFF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Keyspace {
    Disable,
    Enable {
        keyspace_id: u32,
    },
    /// Use API V3 with user keys on the TiKV KV RPC wire format.
    ApiV3 {
        namespace_id: u32,
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
    pub fn api_v3(namespace_id: u32, keyspace_id: u32) -> crate::Result<Self> {
        if namespace_id == 0 {
            return Err(crate::Error::StringError(
                "V3 keyspace identity namespace_id must be non-zero".to_owned(),
            ));
        }
        if keyspace_id == 0 {
            return Err(crate::Error::StringError(
                "V3 keyspace identity keyspace_id must be non-zero".to_owned(),
            ));
        }
        if keyspace_id > API_V3_MAX_KEYSPACE_ID {
            return Err(crate::Error::StringError(
                "V3 keyspace identity keyspace_id must be less than 2^24".to_owned(),
            ));
        }
        Ok(Keyspace::ApiV3 {
            namespace_id,
            keyspace_id,
        })
    }

    pub fn api_version(&self) -> kvrpcpb::ApiVersion {
        match self {
            Keyspace::Disable => kvrpcpb::ApiVersion::V1,
            Keyspace::Enable { .. } => kvrpcpb::ApiVersion::V2,
            Keyspace::ApiV3 { .. } => kvrpcpb::ApiVersion::V3,
            Keyspace::ApiV2NoPrefix => kvrpcpb::ApiVersion::V2,
        }
    }

    pub fn v3_identity(&self) -> Option<apipb::KeyspaceIdentity> {
        match self {
            Keyspace::ApiV3 {
                namespace_id,
                keyspace_id,
            } => Some(apipb::KeyspaceIdentity {
                namespace_id: *namespace_id,
                keyspace_id: *keyspace_id,
            }),
            _ => None,
        }
    }

    pub fn from_context(context: &Option<kvrpcpb::Context>) -> Self {
        let Some(ctx) = context else {
            return Keyspace::Disable;
        };
        if kvrpcpb::ApiVersion::try_from(ctx.api_version) != Ok(kvrpcpb::ApiVersion::V3) {
            return Keyspace::Disable;
        }
        let Some(kvrpcpb::context::Keyspace::KeyspaceIdentity(identity)) = ctx.keyspace.as_ref()
        else {
            return Keyspace::Disable;
        };
        Keyspace::ApiV3 {
            namespace_id: identity.namespace_id,
            keyspace_id: identity.keyspace_id,
        }
    }

    pub fn v3_route_prefix(&self, key_mode: KeyMode) -> Option<[u8; API_V3_PREFIX_LEN]> {
        let Keyspace::ApiV3 {
            namespace_id,
            keyspace_id,
        } = *self
        else {
            return None;
        };
        Some(api_v3_keyspace_prefix(namespace_id, keyspace_id, key_mode))
    }

    pub fn encode_route_key(&self, key: &Key, key_mode: KeyMode) -> Key {
        let Some(prefix) = self.v3_route_prefix(key_mode) else {
            return key.clone();
        };
        let mut route_key = key.clone();
        prepend_bytes(&mut route_key.0, &prefix);
        route_key
    }

    pub fn encode_route_range(
        &self,
        start_key: Key,
        end_key: Key,
        key_mode: KeyMode,
    ) -> (Key, Key) {
        let Some(prefix) = self.v3_route_prefix(key_mode) else {
            return (start_key, end_key);
        };
        let mut start = start_key;
        prepend_bytes(&mut start.0, &prefix);
        let end = if end_key.is_empty() {
            self.v3_route_range_end(key_mode)
                .map(Key::from)
                .unwrap_or(Key::EMPTY)
        } else {
            let mut end = end_key;
            prepend_bytes(&mut end.0, &prefix);
            end
        };
        (start, end)
    }

    pub fn decode_route_range(
        &self,
        start_key: Key,
        end_key: Key,
        key_mode: KeyMode,
    ) -> (Key, Key) {
        if self.v3_route_prefix(key_mode).is_none() {
            return (start_key, end_key);
        }
        (
            self.decode_route_bound(start_key, key_mode),
            self.decode_route_bound(end_key, key_mode),
        )
    }

    fn decode_route_bound(&self, key: Key, key_mode: KeyMode) -> Key {
        let Some(prefix) = self.v3_route_prefix(key_mode) else {
            return key;
        };
        if key.is_empty() {
            return key;
        }
        if key.0 == prefix {
            return Key::EMPTY;
        }
        if key.0.starts_with(&prefix) {
            return Key::from(key.0[API_V3_PREFIX_LEN..].to_vec());
        }
        if self
            .v3_route_range_end(key_mode)
            .is_some_and(|end_prefix| key.0 >= end_prefix)
        {
            return Key::EMPTY;
        }
        Key::EMPTY
    }

    fn v3_route_range_end(&self, key_mode: KeyMode) -> Option<Vec<u8>> {
        let Keyspace::ApiV3 {
            namespace_id,
            keyspace_id,
        } = *self
        else {
            return None;
        };
        let start = u64::from_be_bytes(api_v3_keyspace_prefix(namespace_id, keyspace_id, key_mode));
        Some(start.wrapping_add(1).to_be_bytes().to_vec())
    }
}

pub(crate) fn keyspace_meta_identity(
    keyspace: &keyspacepb::KeyspaceMeta,
) -> Option<apipb::KeyspaceIdentity> {
    match keyspace.keyspace.as_ref() {
        Some(keyspacepb::keyspace_meta::Keyspace::KeyspaceIdentity(identity)) => {
            Some(identity.clone())
        }
        _ => None,
    }
}

pub(crate) fn keyspace_meta_legacy_id(keyspace: &keyspacepb::KeyspaceMeta) -> Option<u32> {
    match keyspace.keyspace {
        Some(keyspacepb::keyspace_meta::Keyspace::Id(id)) => Some(id),
        _ => None,
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

impl TruncateKeyspace for Vec<crate::proto::kvrpcpb::LockInfo> {
    fn truncate_keyspace(mut self, keyspace: Keyspace) -> Self {
        if !matches!(keyspace, Keyspace::Enable { .. }) {
            return self;
        }
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

impl EncodeKeyspace for Vec<crate::proto::kvrpcpb::LockInfo> {
    fn encode_keyspace(mut self, keyspace: Keyspace, key_mode: KeyMode) -> Self {
        if !matches!(keyspace, Keyspace::Enable { .. }) {
            return self;
        }
        for lock in &mut self {
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

fn api_v3_keyspace_prefix(
    namespace_id: u32,
    keyspace_id: u32,
    key_mode: KeyMode,
) -> [u8; API_V3_PREFIX_LEN] {
    debug_assert!(keyspace_id <= API_V3_MAX_KEYSPACE_ID);
    let namespace_bytes = namespace_id.to_be_bytes();
    let keyspace_bytes = keyspace_id.to_be_bytes();
    [
        match key_mode {
            KeyMode::Raw => RAW_KEY_PREFIX,
            KeyMode::Txn => TXN_KEY_PREFIX,
        },
        namespace_bytes[0],
        namespace_bytes[1],
        namespace_bytes[2],
        namespace_bytes[3],
        keyspace_bytes[1],
        keyspace_bytes[2],
        keyspace_bytes[3],
    ]
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
    fn test_api_v3_keyspace() {
        let keyspace = Keyspace::api_v3(7, 0xDEAD).unwrap();
        assert_eq!(keyspace.api_version(), kvrpcpb::ApiVersion::V3);
        assert_eq!(
            keyspace.v3_identity(),
            Some(apipb::KeyspaceIdentity {
                namespace_id: 7,
                keyspace_id: 0xDEAD,
            })
        );
        assert!(Keyspace::api_v3(0, 1).is_err());
        assert!(Keyspace::api_v3(1, 0).is_err());
        assert!(Keyspace::api_v3(1, API_V3_MAX_KEYSPACE_ID + 1).is_err());
    }

    #[test]
    fn test_api_v3_route_key_and_range() {
        let keyspace = Keyspace::api_v3(0x0102_0304, 0x05_0607).unwrap();

        assert_eq!(
            keyspace.encode_route_key(&Key::from(vec![b'k']), KeyMode::Raw),
            Key::from(vec![b'r', 1, 2, 3, 4, 5, 6, 7, b'k'])
        );
        assert_eq!(
            keyspace.encode_route_key(&Key::from(vec![b'k']), KeyMode::Txn),
            Key::from(vec![b'x', 1, 2, 3, 4, 5, 6, 7, b'k'])
        );

        let route_range =
            keyspace.encode_route_range(Key::from(vec![b'a']), Key::from(vec![b'z']), KeyMode::Txn);
        assert_eq!(
            route_range.0,
            Key::from(vec![b'x', 1, 2, 3, 4, 5, 6, 7, b'a'])
        );
        assert_eq!(
            route_range.1,
            Key::from(vec![b'x', 1, 2, 3, 4, 5, 6, 7, b'z'])
        );
        assert_eq!(
            keyspace.decode_route_range(route_range.0, route_range.1, KeyMode::Txn),
            (Key::from(vec![b'a']), Key::from(vec![b'z']))
        );

        let whole_range = keyspace.encode_route_range(Key::EMPTY, Key::EMPTY, KeyMode::Txn);
        assert_eq!(whole_range.0, Key::from(vec![b'x', 1, 2, 3, 4, 5, 6, 7]));
        assert_eq!(whole_range.1, Key::from(vec![b'x', 1, 2, 3, 4, 5, 6, 8]));
        assert_eq!(
            keyspace.decode_route_range(whole_range.0, whole_range.1, KeyMode::Txn),
            (Key::EMPTY, Key::EMPTY)
        );

        let last_keyspace = Keyspace::api_v3(u32::MAX, API_V3_MAX_KEYSPACE_ID).unwrap();
        let last_whole_range =
            last_keyspace.encode_route_range(Key::EMPTY, Key::EMPTY, KeyMode::Txn);
        assert_eq!(
            last_whole_range.0,
            Key::from(vec![b'x', 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])
        );
        assert_eq!(
            last_whole_range.1,
            Key::from(vec![b'y', 0, 0, 0, 0, 0, 0, 0])
        );
        assert_eq!(
            last_keyspace.decode_route_range(last_whole_range.0, last_whole_range.1, KeyMode::Txn),
            (Key::EMPTY, Key::EMPTY)
        );
    }

    #[test]
    fn test_api_v3_wire_key_is_noop() {
        let keyspace = Keyspace::api_v3(7, 0xDEAD).unwrap();
        let key_mode = KeyMode::Txn;

        let key = Key::from(vec![b'k']);
        assert_eq!(key.clone().encode_keyspace(keyspace, key_mode), key);

        let pair = KvPair(Key::from(vec![b'k']), vec![b'v']);
        assert_eq!(pair.clone().encode_keyspace(keyspace, key_mode), pair);

        let range = Range {
            start: Key::from(vec![b'a']),
            end: Key::from(vec![b'b']),
        };
        assert_eq!(range.clone().truncate_keyspace(keyspace), range);
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
}
