use tikv_client_proto::{kvrpcpb, metapb::Region};

use crate::{
    impl_request_codec_for_new_type,
    request::codec::{KeyMode, KeySpaceCodec, KeySpaceId, RawCodec, RequestCodec, TxnCodec},
    Result,
};

#[derive(Clone)]
pub struct ApiV1;

impl RequestCodec for ApiV1 {}

impl RawCodec for ApiV1 {}

#[derive(Clone)]
pub struct Keyspace(KeySpaceCodec);

impl Keyspace {
    pub fn new(id: KeySpaceId) -> Self {
        Keyspace(KeySpaceCodec::new(KeyMode::Raw, id))
    }
}

impl_request_codec_for_new_type!(Keyspace);

impl RawCodec for Keyspace {}
