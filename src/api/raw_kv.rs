use std::fmt::Debug;
use std::io::Result;

use *;

pub struct RawKV;

macro_rules! define_cf_setter {
    () => {
        pub fn cf<C>(mut self, cf: C) -> Self
        where
            C: Into<String>,
        {
            self.cf = Some(cf.into());
            self
        }
    }
}

trait RequestType: Default + Debug + Clone + Copy {}

#[derive(Debug, Clone, Copy, Default)]
pub struct Get;
impl RequestType for Get {}
#[derive(Debug, Clone, Copy, Default)]
pub struct Put;
impl RequestType for Put {}
#[derive(Debug, Clone, Copy, Default)]
pub struct Delete;
impl RequestType for Delete {}
#[derive(Debug, Clone, Copy, Default)]
pub struct Scan {
    limit: u32,
    key_only: bool,
}
impl RequestType for Scan {}

pub struct RawKVRequest<T, P>
where
    T: Default,
{
    t: T,
    payload: P,
    cf: Option<String>,
}

impl<T, P> RawKVRequest<T, P>
where
    T: Default,
{
    pub fn new(payload: P) -> Self {
        RawKVRequest {
            t: T::default(),
            payload,
            cf: None,
        }
    }

    define_cf_setter!();
}

pub struct RawKVBatchRequest<T, P>
where
    T: Default,
    P: Sized,
{
    t: T,
    payloads: Vec<P>,
    cf: Option<String>,
}

impl<T, P> RawKVBatchRequest<T, P>
where
    T: Default,
    P: Sized,
{
    pub fn new<I, K>(payloads: I) -> Self
    where
        I: Iterator<Item = K>,
        K: Into<P>,
    {
        RawKVBatchRequest {
            t: T::default(),
            payloads: payloads.map(Into::into).collect(),
            cf: None,
        }
    }

    define_cf_setter!();
}

pub type RawKVKeyRequest<T> = RawKVRequest<T, Key>;
pub type RawKVBatchKeyRequest<T> = RawKVBatchRequest<T, Key>;
pub type RawKVKvPairRequest<T> = RawKVRequest<T, KvPair>;
pub type RawKVBatchKvPairRequest<T> = RawKVBatchRequest<T, KvPair>;
pub type RawKVRangeRequest<T> = RawKVRequest<T, KeyRange>;
pub type RawKVBatchRangeRequest<T> = RawKVBatchRequest<T, KeyRange>;

pub type RawKVGetRequest = RawKVKeyRequest<Get>;
pub type RawKVBatchGetRequest = RawKVBatchKeyRequest<Get>;
pub type RawKVPutRequest = RawKVKvPairRequest<Put>;
pub type RawKVBatchPutRequest = RawKVBatchKvPairRequest<Put>;
pub type RawKVDeleteRequest = RawKVKeyRequest<Delete>;
pub type RawKVBatchDeleteRequest = RawKVBatchKeyRequest<Delete>;
pub type RawKVScanRequest = RawKVRangeRequest<Scan>;
pub type RawKVBatchScanRequest = RawKVBatchRangeRequest<Scan>;
pub type RawKVDeleteRangeRequest = RawKVRequest<Delete, KeyRange>;

pub type RawKVGetResponse = Result<Value>;
pub type RawKVBatchGetResponse = Result<Vec<KvPair>>;
pub type RawKVPutResponse = Result<()>;
pub type RawKVBatchPutResponse = Result<()>;
pub type RawKVDeleteResponse = Result<()>;
pub type RawKVBatchDeleteResponse = Result<()>;
pub type RawKVDeleteRangeResponse = Result<()>;
pub type RawKVScanResponse = Result<Vec<KvPair>>;
pub type RawKVBatchScanResponse = Result<Vec<KvPair>>;

impl RawKV {
    pub fn get<K>(key: K) -> RawKVGetRequest
    where
        K: Into<Key>,
    {
        RawKVGetRequest::new(key.into())
    }

    pub fn batch_get<I, K>(keys: I) -> RawKVBatchGetRequest
    where
        I: IntoIterator<Item = K>,
        K: Into<Key>,
    {
        RawKVBatchGetRequest::new(keys.into_iter())
    }

    pub fn put<P>(pair: P) -> RawKVPutRequest
    where
        P: Into<KvPair>,
    {
        RawKVPutRequest::new(pair.into())
    }

    pub fn batch_put<I, P>(pairs: I) -> RawKVBatchPutRequest
    where
        I: IntoIterator<Item = P>,
        P: Into<KvPair>,
    {
        RawKVBatchPutRequest::new(pairs.into_iter())
    }

    pub fn delete<K>(key: K) -> RawKVDeleteRequest
    where
        K: Into<Key>,
    {
        RawKVDeleteRequest::new(key.into())
    }

    pub fn batch_delete<I, K>(keys: I) -> RawKVBatchDeleteRequest
    where
        I: IntoIterator<Item = K>,
        K: Into<Key>,
    {
        RawKVBatchDeleteRequest::new(keys.into_iter())
    }

    pub fn scan<R>(range: R, limit: u32) -> RawKVScanRequest
    where
        R: Into<KeyRange>,
    {
        RawKVScanRequest::new(range.into()).limit(limit)
    }

    pub fn batch_scan<I, R>(ranges: I, each_limit: u32) -> RawKVBatchScanRequest
    where
        I: IntoIterator<Item = R>,
        R: Into<KeyRange>,
    {
        RawKVBatchScanRequest::new(ranges.into_iter()).each_limit(each_limit)
    }

    pub fn delete_range<R>(range: R) -> RawKVDeleteRangeRequest
    where
        R: Into<KeyRange>,
    {
        RawKVDeleteRangeRequest::new(range.into())
    }
}

impl RawKVScanRequest {
    pub fn limit(mut self, limit: u32) -> Self {
        self.t.limit = limit;
        self
    }

    pub fn key_only(mut self) -> Self {
        self.t.key_only = true;
        self
    }
}

impl RawKVBatchScanRequest {
    pub fn each_limit(mut self, each_limit: u32) -> Self {
        self.t.limit = each_limit;
        self
    }

    pub fn key_only(mut self) -> Self {
        self.t.key_only = true;
        self
    }
}

impl Request for RawKVGetRequest {
    type Response = RawKVGetResponse;

    fn execute(self, _kv: &TiKV) -> RawKVGetResponse {
        unimplemented!()
    }
}

impl Request for RawKVBatchGetRequest {
    type Response = RawKVBatchGetResponse;

    fn execute(self, _kv: &TiKV) -> RawKVBatchGetResponse {
        unimplemented!()
    }
}

impl Request for RawKVPutRequest {
    type Response = RawKVPutResponse;

    fn execute(self, _kv: &TiKV) -> RawKVPutResponse {
        unimplemented!()
    }
}

impl Request for RawKVBatchPutRequest {
    type Response = RawKVBatchPutResponse;

    fn execute(self, _kv: &TiKV) -> RawKVBatchPutResponse {
        unimplemented!()
    }
}

impl Request for RawKVDeleteRequest {
    type Response = RawKVDeleteResponse;

    fn execute(self, _kv: &TiKV) -> RawKVDeleteResponse {
        unimplemented!()
    }
}

impl Request for RawKVBatchDeleteRequest {
    type Response = RawKVBatchDeleteResponse;

    fn execute(self, _kv: &TiKV) -> RawKVBatchDeleteResponse {
        unimplemented!()
    }
}

impl Request for RawKVScanRequest {
    type Response = RawKVScanResponse;

    fn execute(self, _kv: &TiKV) -> RawKVScanResponse {
        unimplemented!()
    }
}

impl Request for RawKVBatchScanRequest {
    type Response = RawKVBatchScanResponse;

    fn execute(self, _kv: &TiKV) -> RawKVBatchScanResponse {
        unimplemented!()
    }
}

impl Request for RawKVDeleteRangeRequest {
    type Response = RawKVDeleteRangeResponse;

    fn execute(self, _kv: &TiKV) -> RawKVDeleteRangeResponse {
        unimplemented!()
    }
}
