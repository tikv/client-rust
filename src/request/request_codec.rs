use tikv_client_common::Error;
use tikv_client_proto::metapb::Region;
use tikv_client_store::Request;

use crate::{Key, kv::codec::decode_bytes_in_place, Result};

const KEYSPACE_PREFIX_LEN: usize = 4;
const RAW_MODE_PREFIX: u8 = b'r';
const TXN_MODE_PREFIX: u8 = b'x';
const RAW_MODE_MAX_KEY: Prefix = [RAW_MODE_PREFIX + 1, 0, 0, 0];
const TXN_MODE_MAX_KEY: Prefix = [TXN_MODE_PREFIX + 1, 0, 0, 0];
const MAX_KEYSPACE_ID: KeySpaceId = [0xff, 0xff, 0xff];

#[macro_export]
macro_rules! plain_request {
    ($req: ident, $codec: ident) => {
        if $codec.is_plain() {
            return ::std::borrow::Cow::Borrowed($req);
        }
    };
}

#[macro_export]
macro_rules! plain_response {
    ($resp: ident, $codec: ident) => {
        if $codec.is_plain() {
            return Ok($resp);
        }
    };
}

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
        todo!()
    }

    fn decode_region(&self, region: Region) -> Result<Region> {
        todo!()
    }

    fn is_plain(&self) -> bool {
        todo!()
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