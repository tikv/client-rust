use std::fmt::Debug;

use *;

pub struct RawKv;

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

pub struct RawRequest<T, P>
where
    T: Default,
{
    t: T,
    payload: P,
    cf: Option<String>,
}

impl<T, P> RawRequest<T, P>
where
    T: Default,
{
    pub fn new(payload: P) -> Self {
        RawRequest {
            t: T::default(),
            payload,
            cf: None,
        }
    }

    define_cf_setter!();
}

pub struct RawBatchRequest<T, P>
where
    T: Default,
    P: Sized,
{
    t: T,
    payloads: Vec<P>,
    cf: Option<String>,
}

impl<T, P> RawBatchRequest<T, P>
where
    T: Default,
    P: Sized,
{
    pub fn new<I, K>(payloads: I) -> Self
    where
        I: Iterator<Item = K>,
        K: Into<P>,
    {
        RawBatchRequest {
            t: T::default(),
            payloads: payloads.map(Into::into).collect(),
            cf: None,
        }
    }

    define_cf_setter!();
}

pub type RawKeyRequest<T> = RawRequest<T, Key>;
pub type RawBatchKeyRequest<T> = RawBatchRequest<T, Key>;
pub type RawKvPairRequest<T> = RawRequest<T, KvPair>;
pub type RawBatchKvPairRequest<T> = RawBatchRequest<T, KvPair>;
pub type RawRangeRequest<T> = RawRequest<T, KeyRange>;
pub type RawBatchRangeRequest<T> = RawBatchRequest<T, KeyRange>;

pub type RawGetRequest = RawKeyRequest<Get>;
pub type RawBatchGetRequest = RawBatchKeyRequest<Get>;
pub type RawPutRequest = RawKvPairRequest<Put>;
pub type RawBatchPutRequest = RawBatchKvPairRequest<Put>;
pub type RawDeleteRequest = RawKeyRequest<Delete>;
pub type RawBatchDeleteRequest = RawBatchKeyRequest<Delete>;
pub type RawScanRequest = RawRangeRequest<Scan>;
pub type RawBatchScanRequest = RawBatchRangeRequest<Scan>;
pub type RawDeleteRangeRequest = RawRequest<Delete, KeyRange>;

impl RawKv {
    pub fn get<K>(key: K) -> RawGetRequest
    where
        K: Into<Key>,
    {
        RawGetRequest::new(key.into())
    }

    pub fn batch_get<I, K>(keys: I) -> RawBatchGetRequest
    where
        I: IntoIterator<Item = K>,
        K: Into<Key>,
    {
        RawBatchGetRequest::new(keys.into_iter())
    }

    pub fn put<P>(pair: P) -> RawPutRequest
    where
        P: Into<KvPair>,
    {
        RawPutRequest::new(pair.into())
    }

    pub fn batch_put<I, P>(pairs: I) -> RawBatchPutRequest
    where
        I: IntoIterator<Item = P>,
        P: Into<KvPair>,
    {
        RawBatchPutRequest::new(pairs.into_iter())
    }

    pub fn delete<K>(key: K) -> RawDeleteRequest
    where
        K: Into<Key>,
    {
        RawDeleteRequest::new(key.into())
    }

    pub fn batch_delete<I, K>(keys: I) -> RawBatchDeleteRequest
    where
        I: IntoIterator<Item = K>,
        K: Into<Key>,
    {
        RawBatchDeleteRequest::new(keys.into_iter())
    }

    pub fn scan<R>(range: R, limit: u32) -> RawScanRequest
    where
        R: Into<KeyRange>,
    {
        RawScanRequest::new(range.into()).limit(limit)
    }

    pub fn batch_scan<I, R>(ranges: I, each_limit: u32) -> RawBatchScanRequest
    where
        I: IntoIterator<Item = R>,
        R: Into<KeyRange>,
    {
        RawBatchScanRequest::new(ranges.into_iter()).each_limit(each_limit)
    }

    pub fn delete_range<R>(range: R) -> RawDeleteRangeRequest
    where
        R: Into<KeyRange>,
    {
        RawDeleteRangeRequest::new(range.into())
    }
}

impl RawScanRequest {
    pub fn limit(mut self, limit: u32) -> Self {
        self.t.limit = limit;
        self
    }

    pub fn key_only(mut self) -> Self {
        self.t.key_only = true;
        self
    }
}

impl RawBatchScanRequest {
    pub fn each_limit(mut self, each_limit: u32) -> Self {
        self.t.limit = each_limit;
        self
    }

    pub fn key_only(mut self) -> Self {
        self.t.key_only = true;
        self
    }
}

impl Request for RawGetRequest {
    type Response = Value;

    fn execute(self, _kv: &TiKv) -> TiKvFuture<Self::Response> {
        unimplemented!()
    }
}

impl Request for RawBatchGetRequest {
    type Response = KvPair;

    fn execute(self, _kv: &TiKv) -> TiKvFuture<Self::Response> {
        unimplemented!()
    }
}

impl Request for RawPutRequest {
    type Response = ();

    fn execute(self, _kv: &TiKv) -> TiKvFuture<Self::Response> {
        unimplemented!()
    }
}

impl Request for RawBatchPutRequest {
    type Response = ();

    fn execute(self, _kv: &TiKv) -> TiKvFuture<Self::Response> {
        unimplemented!()
    }
}

impl Request for RawDeleteRequest {
    type Response = ();

    fn execute(self, _kv: &TiKv) -> TiKvFuture<Self::Response> {
        unimplemented!()
    }
}

impl Request for RawBatchDeleteRequest {
    type Response = ();

    fn execute(self, _kv: &TiKv) -> TiKvFuture<Self::Response> {
        unimplemented!()
    }
}

impl Request for RawScanRequest {
    type Response = Vec<KvPair>;

    fn execute(self, _kv: &TiKv) -> TiKvFuture<Self::Response> {
        unimplemented!()
    }
}

impl Request for RawBatchScanRequest {
    type Response = Vec<KvPair>;

    fn execute(self, _kv: &TiKv) -> TiKvFuture<Self::Response> {
        unimplemented!()
    }
}

impl Request for RawDeleteRangeRequest {
    type Response = ();

    fn execute(self, _kv: &TiKv) -> TiKvFuture<Self::Response> {
        unimplemented!()
    }
}
