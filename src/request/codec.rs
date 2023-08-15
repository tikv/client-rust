// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use crate::proto::kvrpcpb;
use crate::request::KvRequest;
use std::borrow::Cow;

pub trait Codec: Clone + Sync + Send + 'static {
    fn encode_request<'a, R: KvRequest>(&self, req: &'a R) -> Cow<'a, R> {
        Cow::Borrowed(req)
    }
}

#[derive(Clone, Default)]
pub struct ApiV1Codec {}

impl Codec for ApiV1Codec {}

#[derive(Clone)]
pub struct ApiV2Codec {
    _keyspace_id: u32,
}

impl ApiV2Codec {
    pub fn new(keyspace_id: u32) -> Self {
        Self {
            _keyspace_id: keyspace_id,
        }
    }
}

impl Codec for ApiV2Codec {
    fn encode_request<'a, R: KvRequest>(&self, req: &'a R) -> Cow<'a, R> {
        let mut req = req.clone();
        req.set_api_version(kvrpcpb::ApiVersion::V2);
        Cow::Owned(req)
    }
}
