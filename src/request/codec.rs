use std::ops::{Deref, DerefMut};
use tikv_client_common::Error;
use tikv_client_proto::{kvrpcpb, metapb::Region};

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

    fn decode_key(&self, key: Vec<u8>) -> Result<Vec<u8>> {
        Ok(key)
    }

    fn decode_keys(&self, keys: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>> {
        keys.into_iter()
            .map(|key| self.decode_key(key))
            .collect::<Result<Vec<Vec<u8>>>>()
    }

    fn decode_pairs(&self, mut pairs: Vec<kvrpcpb::KvPair>) -> Result<Vec<kvrpcpb::KvPair>> {
        for pair in pairs.iter_mut() {
            *pair.mut_key() = self.decode_key(pair.take_key())?;
        }

        Ok(pairs)
    }

    fn encode_range(&self, start: Vec<u8>, end: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        (start, end)
    }

    fn encode_ranges(&self, mut ranges: Vec<kvrpcpb::KeyRange>) -> Vec<kvrpcpb::KeyRange> {
        for range in ranges.iter_mut() {
            let (start, end) = self.encode_range(range.take_start_key(), range.take_end_key());
            *range.mut_start_key() = start;
            *range.mut_end_key() = end;
        }

        ranges
    }

    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        key
    }

    fn decode_region(&self, region: Region) -> Result<Region> {
        Ok(region)
    }

    fn is_plain(&self) -> bool {
        true
    }
}

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

    fn decode_region(&self, mut region: Region) -> Result<Region> {
        decode_bytes_in_place(region.mut_start_key(), false)?;
        decode_bytes_in_place(region.mut_end_key(), false)?;
        Ok(region)
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

#[derive(Clone,Copy,Default,PartialEq)]
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

    fn decode_key(&self, mut key: Vec<u8>) -> Result<Vec<u8>> {
        let prefix: Prefix = (*self).into();

        if !key.starts_with(&prefix) {
            return Err(Error::CorruptedKeyspace {
                expected: prefix.to_vec(),
                actual: key[..KEYSPACE_PREFIX_LEN].to_vec(),
                key,
            });
        }

        Ok(key.split_off(KEYSPACE_PREFIX_LEN))
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

    fn decode_region(&self, mut region: Region) -> Result<Region> {
        decode_bytes_in_place(region.mut_start_key(), false)?;
        decode_bytes_in_place(region.mut_end_key(), false)?;

        // Map the region's start key to the keyspace's start key.
        if region.get_start_key() < self.mode.min_key().as_slice() {
            *region.mut_start_key() = vec![];
        } else {
            *region.mut_start_key() = self.decode_key(region.get_start_key().to_vec())?;
        }

        // Map the region's end key to the keyspace's end key.
        if region.get_end_key() > self.mode.max_key().as_slice() {
            *region.mut_end_key() = vec![];
        } else {
            *region.mut_end_key() = self.decode_key(region.get_end_key().to_vec())?;
        }

        Ok(region)
    }

    fn is_plain(&self) -> bool {
        false
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

impl RequestCodec for RawKeyspaceCodec {}

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

impl RequestCodec for TxnKeyspaceCodec {}

impl TxnCodec for TxnKeyspaceCodec {}
