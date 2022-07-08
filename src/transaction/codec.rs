use tikv_client_proto::{kvrpcpb, metapb::Region};

use crate::{
    impl_request_codec_for_new_type,
    kv::codec::decode_bytes_in_place,
    request::codec::{KeyMode, KeySpaceCodec, KeySpaceId, RequestCodec, TxnCodec},
    Key, Result,
};

#[derive(Clone)]
pub struct ApiV1;

impl RequestCodec for ApiV1 {
    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        Key::from(key).to_encoded().into()
    }

    fn decode_region(&self, region: &mut Region) -> Result<()> {
        decode_bytes_in_place(region.mut_start_key(), false)?;
        decode_bytes_in_place(region.mut_end_key(), false)?;
        Ok(())
    }
}

impl TxnCodec for ApiV1 {}

#[derive(Clone)]
pub struct Keyspace(KeySpaceCodec);

impl Keyspace {
    pub fn new(id: KeySpaceId) -> Self {
        Keyspace(KeySpaceCodec::new(KeyMode::Txn, id))
    }
}

impl_request_codec_for_new_type!(Keyspace);

impl TxnCodec for Keyspace {}
