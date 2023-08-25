// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use crate::proto::kvrpcpb;
use crate::request::KvRequest;

pub trait Codec: Clone + Sync + Send + 'static {
    fn encode_request<R: KvRequest>(&self, _req: &mut R) {}
    // TODO: fn decode_response()
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
    fn encode_request<R: KvRequest>(&self, req: &mut R) {
        req.set_api_version(kvrpcpb::ApiVersion::V2);
        // TODO: req.encode_request(self);
    }
}

// EncodeRequest is just a type wrapper to avoid passing not encoded request to `PlanBuilder` by mistake.
#[derive(Clone)]
pub struct EncodedRequest<Req: KvRequest> {
    pub inner: Req,
}

impl<Req: KvRequest> EncodedRequest<Req> {
    pub fn new<C: Codec>(mut req: Req, codec: &C) -> Self {
        codec.encode_request(&mut req);
        Self { inner: req }
    }
}
