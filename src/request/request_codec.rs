use tikv_client_proto::metapb::Region;

use crate::{kv::codec::decode_bytes_in_place, Key, Result};

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

#[derive(Clone)]
pub struct RawApiV1;

impl RequestCodec for RawApiV1 {}

#[derive(Clone)]
pub struct TxnApiV1;

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

pub trait RawCodec: RequestCodec {}
pub trait TxnCodec: RequestCodec {}

impl RawCodec for RawApiV1 {}
impl TxnCodec for TxnApiV1 {}
