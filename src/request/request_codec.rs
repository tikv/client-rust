use tikv_client_proto::metapb::Region;

use crate::Result;

pub trait RequestCodec: Sized + Clone + Sync + Send + 'static {
    fn encode_key(&self, key: Vec<u8>) -> Vec<u8> {
        key
    }
    fn decode_key(&self, key: Vec<u8>) -> Result<Vec<u8>> {
        Ok(key)
    }
    fn encode_range(&self, start: Vec<u8>, end: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        (start, end)
    }
    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        key
    }
    fn decode_region(&self, region: Region) -> Result<Region> {
        Ok(region)
    }
}

#[derive(Clone)]
pub struct RawApiV1;

impl RequestCodec for RawApiV1 {}

#[derive(Clone)]
pub struct TxnApiV1;

impl RequestCodec for TxnApiV1 {}
