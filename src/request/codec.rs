use tikv_client_common::Error;
use tikv_client_proto::metapb::Region;

use crate::{kv::codec::decode_bytes_in_place, Key, Result};

const RAW_MODE_PREFIX: u8 = b'r';
const TXN_MODE_PREFIX: u8 = b'x';

const KEYSPACE_PREFIX_LEN: usize = 4;

const RAW_MODE_MIN_KEY: Prefix = [RAW_MODE_PREFIX, 0, 0, 0];
const RAW_MODE_MAX_KEY: Prefix = [RAW_MODE_PREFIX + 1, 0, 0, 0];

const TXN_MODE_MIN_KEY: Prefix = [TXN_MODE_PREFIX, 0, 0, 0];
const TXN_MODE_MAX_KEY: Prefix = [TXN_MODE_PREFIX + 1, 0, 0, 0];

pub trait RequestCodec: Sized + Clone + Sync + Send + 'static {
    fn encode_key(&self, key: Key) -> Key {
        key
    }
    fn decode_key(&self, key: Key) -> Result<Key> {
        Ok(key)
    }
    fn encode_range(&self, start: Key, end: Key) -> (Key, Key) {
        (start, end)
    }
    fn encode_pd_query(&self, key: Key) -> Key {
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
    fn encode_pd_query(&self, key: Key) -> Key {
        key.to_encoded()
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
    fn min_key(self) -> Key {
        match self {
            KeyMode::Raw => Key::from(RAW_MODE_MIN_KEY.to_vec()),
            KeyMode::Txn => Key::from(TXN_MODE_MIN_KEY.to_vec()),
        }
    }

    fn max_key(self) -> Key {
        match self {
            KeyMode::Raw => Key::from(RAW_MODE_MAX_KEY.to_vec()),
            KeyMode::Txn => Key::from(TXN_MODE_MAX_KEY.to_vec()),
        }
    }
}

type Prefix = [u8; KEYSPACE_PREFIX_LEN];

type KeySpaceId = [u8; 3];

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
    fn encode_key(&self, mut key: Key) -> Key {
        let this = self.clone();
        let mut encoded = Vec::with_capacity(key.len() + KEYSPACE_PREFIX_LEN);
        let prefix: Prefix = this.into();
        encoded.extend_from_slice(&prefix);
        encoded.append(&mut key);
        encoded.into()
    }

    fn decode_key(&self, mut key: Key) -> Result<Key> {
        let prefix: Prefix = self.clone().into();

        if !key.starts_with(&prefix) {
            return Err(Error::CorruptedKeyspace {
                expected: prefix.to_vec(),
                actual: key[..KEYSPACE_PREFIX_LEN].to_vec(),
                key: key.into(),
            });
        }

        Ok(key.split_off(KEYSPACE_PREFIX_LEN).into())
    }

    fn encode_range(&self, start: Key, end: Key) -> (Key, Key) {
        (self.encode_key(start), self.encode_key(end))
    }

    fn encode_pd_query(&self, key: Key) -> Key {
        self.encode_key(key).to_encoded()
    }

    fn decode_region(&self, mut region: Region) -> Result<Region> {
        decode_bytes_in_place(region.mut_start_key(), false)?;
        decode_bytes_in_place(region.mut_end_key(), false)?;

        if region.get_start_key() < self.mode.min_key().as_slice() {
            *region.mut_start_key() = vec![];
        } else {
            *region.mut_start_key() = self
                .decode_key(region.get_start_key().to_vec().into())?
                .into();
        }

        if region.get_end_key() > self.mode.max_key().as_slice() {
            *region.mut_end_key() = vec![];
        } else {
            *region.mut_end_key() = self
                .decode_key(region.get_end_key().to_vec().into())?
                .into();
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
