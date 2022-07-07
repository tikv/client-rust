use core::intrinsics::copy;
use std::ops::{Deref, DerefMut};

use tikv_client_common::Error;
use tikv_client_proto::{errorpb, kvrpcpb, metapb::Region};

use crate::{kv::codec::decode_bytes_in_place, Key, Result};

const RAW_MODE_PREFIX: u8 = b'r';
const TXN_MODE_PREFIX: u8 = b'x';

const KEYSPACE_PREFIX_LEN: usize = 4;

const RAW_MODE_MIN_KEY: Prefix = [RAW_MODE_PREFIX, 0, 0, 0];
const RAW_MODE_MAX_KEY: Prefix = [RAW_MODE_PREFIX + 1, 0, 0, 0];

const TXN_MODE_MIN_KEY: Prefix = [TXN_MODE_PREFIX, 0, 0, 0];
const TXN_MODE_MAX_KEY: Prefix = [TXN_MODE_PREFIX + 1, 0, 0, 0];

const MAX_KEYSPACE_ID: KeySpaceId = KeySpaceId([0xff, 0xff, 0xff]);

pub trait RequestCodec: Sized + Clone + Sync + Send + 'static {
    fn encode_key(&self, key: Vec<u8>) -> Vec<u8> {
        key
    }

    fn decode_key(&self, _key: &mut Vec<u8>) -> Result<()> {
        Ok(())
    }

    fn encode_range(&self, start: Vec<u8>, end: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        (start, end)
    }

    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        key
    }

    fn decode_region(&self, _region: &mut Region) -> Result<()> {
        Ok(())
    }

    fn version(&self) -> kvrpcpb::ApiVersion {
        kvrpcpb::ApiVersion::V1
    }
}

pub trait RequestCodecExt: RequestCodec {
    fn encode_primary_lock(&self, lock: Vec<u8>) -> Vec<u8> {
        self.encode_key(lock)
    }

    fn encode_primary_key(&self, key: Vec<u8>) -> Vec<u8> {
        self.encode_key(key)
    }

    fn encode_mutations(&self, mutations: Vec<kvrpcpb::Mutation>) -> Vec<kvrpcpb::Mutation> {
        mutations
            .into_iter()
            .map(|mut m| {
                let key = m.take_key();
                m.set_key(self.encode_key(key));
                m
            })
            .collect()
    }

    fn encode_keys(&self, keys: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        keys.into_iter().map(|key| self.encode_key(key)).collect()
    }

    fn encode_pairs(&self, mut pairs: Vec<kvrpcpb::KvPair>) -> Vec<kvrpcpb::KvPair> {
        for pair in pairs.iter_mut() {
            *pair.mut_key() = self.encode_key(pair.take_key());
        }

        pairs
    }

    fn encode_ranges(&self, mut ranges: Vec<kvrpcpb::KeyRange>) -> Vec<kvrpcpb::KeyRange> {
        for range in ranges.iter_mut() {
            let (start, end) = self.encode_range(range.take_start_key(), range.take_end_key());
            *range.mut_start_key() = start;
            *range.mut_end_key() = end;
        }

        ranges
    }

    fn decode_keys(&self, keys: &mut [Vec<u8>]) -> Result<()> {
        for key in keys.iter_mut() {
            self.decode_key(key)?;
        }

        Ok(())
    }

    fn decode_error(&self, err: &mut kvrpcpb::KeyError) -> Result<()> {
        if err.has_locked() {
            let locked = err.mut_locked();
            self.decode_lock_info(locked)?;
        }

        if err.has_conflict() {
            let conflict = err.mut_conflict();
            self.decode_key(conflict.mut_key())?;
            self.decode_key(conflict.mut_primary())?;
        }

        if err.has_already_exist() {
            let already_exist = err.mut_already_exist();
            self.decode_key(already_exist.mut_key())?;
        }

        // We do not decode key in `Deadlock` since there is no use for the key right now in client side.
        // All we need is the key hash to detect deadlock.
        // TODO: while we check the keys against the deadlock key hash, we need to encode the key.

        if err.has_commit_ts_expired() {
            let commit_ts_expired = err.mut_commit_ts_expired();
            self.decode_key(commit_ts_expired.mut_key())?;
        }

        if err.has_txn_not_found() {
            let txn_not_found = err.mut_txn_not_found();
            self.decode_key(txn_not_found.mut_primary_key())?;
        }

        if err.has_assertion_failed() {
            let assertion_failed = err.mut_assertion_failed();
            self.decode_key(assertion_failed.mut_key())?;
        }

        Ok(())
    }

    fn decode_errors(&self, errors: &mut [kvrpcpb::KeyError]) -> Result<()> {
        for err in errors.iter_mut() {
            self.decode_error(err)?;
        }

        Ok(())
    }

    fn decode_lock_info(&self, lock: &mut kvrpcpb::LockInfo) -> Result<()> {
        self.decode_key(lock.mut_primary_lock())?;
        self.decode_key(lock.mut_key())?;
        self.decode_keys(lock.mut_secondaries())
    }

    fn decode_locks(&self, locks: &mut [kvrpcpb::LockInfo]) -> Result<()> {
        for lock in locks.iter_mut() {
            self.decode_lock_info(lock)?;
        }

        Ok(())
    }

    fn decode_pairs(&self, pairs: &mut [kvrpcpb::KvPair]) -> Result<()> {
        for pair in pairs.iter_mut() {
            self.decode_key(pair.mut_key())?;
        }

        Ok(())
    }

    fn decode_kvs(&self, kvs: &mut [kvrpcpb::KvPair]) -> Result<()> {
        self.decode_pairs(kvs)
    }

    fn decode_regions(&self, regions: &mut [Region]) -> Result<()> {
        for region in regions.iter_mut() {
            self.decode_region(region)?;
        }
        Ok(())
    }

    fn decode_region_error(&self, err: &mut errorpb::Error) -> Result<()> {
        if err.has_epoch_not_match() {
            self.decode_regions(err.mut_epoch_not_match().mut_current_regions())?;
        }
        Ok(())
    }
}

impl<T: RequestCodec> RequestCodecExt for T {}

#[derive(Clone)]
pub struct TxnApiV1;

#[derive(Clone)]
pub struct RawApiV1;

pub trait RawCodec: RequestCodec {}

pub trait TxnCodec: RequestCodec {}

impl RequestCodec for RawApiV1 {}

impl RequestCodec for TxnApiV1 {
    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        Key::from(key).to_encoded().into()
    }

    fn decode_region(&self, region: &mut Region) -> Result<()> {
        decode_bytes_in_place(region.mut_start_key(), false)?;
        decode_bytes_in_place(region.mut_end_key(), false)?;
        Ok(())
    }
}

impl RawCodec for RawApiV1 {}

impl TxnCodec for TxnApiV1 {}

#[derive(Copy, Clone)]
enum KeyMode {
    Raw,
    Txn,
}

impl From<KeyMode> for u8 {
    fn from(mode: KeyMode) -> u8 {
        match mode {
            KeyMode::Raw => b'r',
            KeyMode::Txn => b'x',
        }
    }
}

impl KeyMode {
    fn min_key(self) -> Vec<u8> {
        match self {
            KeyMode::Raw => RAW_MODE_MIN_KEY.to_vec(),
            KeyMode::Txn => TXN_MODE_MIN_KEY.to_vec(),
        }
    }

    fn max_key(self) -> Vec<u8> {
        match self {
            KeyMode::Raw => RAW_MODE_MAX_KEY.to_vec(),
            KeyMode::Txn => TXN_MODE_MAX_KEY.to_vec(),
        }
    }
}

type Prefix = [u8; KEYSPACE_PREFIX_LEN];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct KeySpaceId([u8; 3]);

impl Deref for KeySpaceId {
    type Target = [u8; 3];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KeySpaceId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Copy, Clone)]
struct KeySpaceCodec {
    mode: KeyMode,
    id: KeySpaceId,
}

impl From<KeySpaceCodec> for Prefix {
    fn from(codec: KeySpaceCodec) -> Self {
        [codec.mode.into(), codec.id[0], codec.id[1], codec.id[2]]
    }
}

impl RequestCodec for KeySpaceCodec {
    fn encode_key(&self, mut key: Vec<u8>) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(key.len() + KEYSPACE_PREFIX_LEN);
        let prefix: Prefix = (*self).into();
        encoded.extend_from_slice(&prefix);
        encoded.append(&mut key);
        encoded
    }

    fn decode_key(&self, key: &mut Vec<u8>) -> Result<()> {
        let prefix: Prefix = (*self).into();

        if !key.starts_with(&prefix) {
            return Err(Error::CorruptedKeyspace {
                expected: prefix.to_vec(),
                actual: key[..KEYSPACE_PREFIX_LEN].to_vec(),
                key: key.to_vec(),
            });
        }

        unsafe {
            let trimmed_len = key.len() - KEYSPACE_PREFIX_LEN;
            let ptr = key.as_mut_ptr();
            let trimmed = key[KEYSPACE_PREFIX_LEN..].as_mut_ptr();

            copy(trimmed, ptr, trimmed_len);

            key.set_len(trimmed_len);
        }
        Ok(())
    }

    fn encode_range(&self, start: Vec<u8>, end: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        if self.id == MAX_KEYSPACE_ID {
            (self.encode_key(start), self.mode.max_key())
        } else {
            (self.encode_key(start), self.encode_key(end))
        }
    }

    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        Key::from(self.encode_key(key)).to_encoded().into()
    }

    fn decode_region(&self, region: &mut Region) -> Result<()> {
        decode_bytes_in_place(region.mut_start_key(), false)?;
        decode_bytes_in_place(region.mut_end_key(), false)?;

        // Map the region's start key to the keyspace's start key.
        if region.get_start_key() < self.mode.min_key().as_slice() {
            *region.mut_start_key() = vec![];
        } else {
            self.decode_key(region.mut_start_key())?;
        }

        // Map the region's end key to the keyspace's end key.
        if region.get_end_key().is_empty() || region.get_end_key() > self.mode.max_key().as_slice()
        {
            *region.mut_end_key() = vec![];
        } else {
            self.decode_key(region.mut_end_key())?;
        }

        Ok(())
    }

    fn version(&self) -> kvrpcpb::ApiVersion {
        kvrpcpb::ApiVersion::V2
    }
}

#[derive(Clone)]
pub struct RawKeyspaceCodec(KeySpaceCodec);

impl RawKeyspaceCodec {
    pub fn new(id: KeySpaceId) -> Self {
        RawKeyspaceCodec(KeySpaceCodec {
            mode: KeyMode::Raw,
            id,
        })
    }
}

impl RequestCodec for RawKeyspaceCodec {
    fn encode_key(&self, key: Vec<u8>) -> Vec<u8> {
        self.0.encode_key(key)
    }

    fn decode_key(&self, key: &mut Vec<u8>) -> Result<()> {
        self.0.decode_key(key)
    }

    fn encode_range(&self, start: Vec<u8>, end: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        self.0.encode_range(start, end)
    }

    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        self.0.encode_pd_query(key)
    }

    fn decode_region(&self, region: &mut Region) -> Result<()> {
        self.0.decode_region(region)
    }

    fn version(&self) -> kvrpcpb::ApiVersion {
        self.0.version()
    }
}

impl RawCodec for RawKeyspaceCodec {}

#[derive(Clone)]
pub struct TxnKeyspaceCodec(KeySpaceCodec);

impl TxnKeyspaceCodec {
    pub fn new(id: KeySpaceId) -> Self {
        TxnKeyspaceCodec(KeySpaceCodec {
            mode: KeyMode::Txn,
            id,
        })
    }
}

impl RequestCodec for TxnKeyspaceCodec {
    fn encode_key(&self, key: Vec<u8>) -> Vec<u8> {
        self.0.encode_key(key)
    }

    fn decode_key(&self, key: &mut Vec<u8>) -> Result<()> {
        self.0.decode_key(key)
    }

    fn encode_range(&self, start: Vec<u8>, end: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        self.0.encode_range(start, end)
    }

    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        self.0.encode_pd_query(key)
    }

    fn decode_region(&self, region: &mut Region) -> Result<()> {
        self.0.decode_region(region)
    }

    fn version(&self) -> kvrpcpb::ApiVersion {
        self.0.version()
    }
}

impl TxnCodec for TxnKeyspaceCodec {}
