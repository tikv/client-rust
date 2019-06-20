// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::ColumnFamily;
use crate::{rpc::RpcClient, BoundRange, Error, Key, KvPair, Result, Value};
use futures::future::Either;
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::{pin::Pin, sync::Arc, u32};

const MAX_RAW_KV_SCAN_LIMIT: u32 = 10240;

trait RequestInner: Sized {
    type Resp;
    type F: Future<Output = Result<Self::Resp>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F;
}

enum RequestState<Inner>
where
    Inner: RequestInner,
{
    Uninitiated(Option<(Arc<RpcClient>, Inner, Option<ColumnFamily>)>),
    Initiated(Inner::F),
}

impl<Inner> RequestState<Inner>
where
    Inner: RequestInner,
{
    pub fn new(client: Arc<RpcClient>, inner: Inner) -> Self {
        RequestState::Uninitiated(Some((client, inner, None)))
    }

    fn cf(&mut self, new_cf: impl Into<ColumnFamily>) {
        if let RequestState::Uninitiated(Some((_, _, ref mut cf))) = self {
            cf.replace(new_cf.into());
        }
    }

    fn inner_mut(&mut self) -> Option<&mut Inner> {
        match self {
            RequestState::Uninitiated(Some((_, ref mut inner, _))) => Some(inner),
            _ => None,
        }
    }

    fn assure_initialized<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Self> {
        unsafe {
            let mut this = Pin::get_unchecked_mut(self);
            if let RequestState::Uninitiated(state) = &mut this {
                let (client, inner, cf) = state.take().unwrap();
                *this = RequestState::Initiated(inner.execute(client, cf));
            }
            Pin::new_unchecked(this)
        }
    }

    fn assert_init_pin_mut<'a>(
        self: Pin<&'a mut Self>,
    ) -> Pin<&'a mut dyn Future<Output = Result<Inner::Resp>>> {
        unsafe {
            match Pin::get_unchecked_mut(self) {
                RequestState::Initiated(future) => Pin::new_unchecked(&mut *future),
                _ => unreachable!(),
            }
        }
    }

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<Inner::Resp>> {
        self = self.assure_initialized();
        self.assert_init_pin_mut().poll(cx)
    }
}

/// An unresolved [`Client::get`](Client::get) request.
///
/// Once resolved this request will result in the fetching of the value associated with the given
/// key.
pub struct Get {
    state: RequestState<GetInner>,
}

impl Get {
    pub fn new(client: Arc<RpcClient>, key: Key) -> Self {
        Self {
            state: RequestState::new(client, GetInner { key }),
        }
    }

    /// Set the (optional) [`ColumnFamily`](ColumnFamily).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.state.cf(cf);
        self
    }
}

impl Future for Get {
    type Output = Result<Option<Value>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).state).poll(cx) }
    }
}

struct GetInner {
    key: Key,
}

impl RequestInner for GetInner {
    type Resp = Option<Value>;
    existential type F: Future<Output = Result<Option<Value>>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F {
        client.raw_get(self.key, cf)
    }
}

/// An unresolved [`Client::batch_get`](Client::batch_get) request.
///
/// Once resolved this request will result in the fetching of the values associated with the given
/// keys.
pub struct BatchGet {
    state: RequestState<BatchGetInner>,
}

impl BatchGet {
    pub fn new(client: Arc<RpcClient>, keys: Vec<Key>) -> Self {
        Self {
            state: RequestState::new(client, BatchGetInner { keys }),
        }
    }

    /// Set the (optional) [`ColumnFamily`](ColumnFamily).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.state.cf(cf);
        self
    }
}

impl Future for BatchGet {
    type Output = Result<Vec<KvPair>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).state).poll(cx) }
    }
}

struct BatchGetInner {
    keys: Vec<Key>,
}

impl RequestInner for BatchGetInner {
    type Resp = Vec<KvPair>;
    existential type F: Future<Output = Result<Vec<KvPair>>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F {
        client.raw_batch_get(self.keys, cf)
    }
}

/// An unresolved [`Client::put`](Client::put) request.
///
/// Once resolved this request will result in the putting of the value associated with the given
/// key.
pub struct Put {
    state: RequestState<PutInner>,
}

impl Put {
    pub fn new(client: Arc<RpcClient>, key: Key, value: Value) -> Self {
        Self {
            state: RequestState::new(client, PutInner { key, value }),
        }
    }

    /// Set the (optional) [`ColumnFamily`](ColumnFamily).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.state.cf(cf);
        self
    }
}

impl Future for Put {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).state).poll(cx) }
    }
}

struct PutInner {
    key: Key,
    value: Value,
}

impl RequestInner for PutInner {
    type Resp = ();
    existential type F: Future<Output = Result<()>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F {
        let (key, value) = (self.key, self.value);
        client.raw_put(key, value, cf)
    }
}

/// An unresolved [`Client::batch_put`](Client::batch_put) request.
///
/// Once resolved this request will result in the setting of the value associated with the given key.
pub struct BatchPut {
    state: RequestState<BatchPutInner>,
}

impl BatchPut {
    pub fn new(client: Arc<RpcClient>, pairs: Vec<KvPair>) -> Self {
        Self {
            state: RequestState::new(client, BatchPutInner { pairs }),
        }
    }

    /// Set the (optional) [`ColumnFamily`](ColumnFamily).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.state.cf(cf);
        self
    }
}

impl Future for BatchPut {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).state).poll(cx) }
    }
}

struct BatchPutInner {
    pairs: Vec<KvPair>,
}

impl RequestInner for BatchPutInner {
    type Resp = ();
    existential type F: Future<Output = Result<()>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F {
        client.raw_batch_put(self.pairs, cf)
    }
}

/// An unresolved [`Client::delete`](Client::delete) request.
///
/// Once resolved this request will result in the deletion of the given key.
pub struct Delete {
    state: RequestState<DeleteInner>,
}

impl Delete {
    pub fn new(client: Arc<RpcClient>, key: Key) -> Self {
        Self {
            state: RequestState::new(client, DeleteInner { key }),
        }
    }

    /// Set the (optional) [`ColumnFamily`](ColumnFamily).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.state.cf(cf);
        self
    }
}

impl Future for Delete {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).state).poll(cx) }
    }
}

struct DeleteInner {
    key: Key,
}

impl RequestInner for DeleteInner {
    type Resp = ();
    existential type F: Future<Output = Result<()>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F {
        client.raw_delete(self.key, cf)
    }
}

/// An unresolved [`Client::batch_delete`](Client::batch_delete) request.
///
/// Once resolved this request will result in the deletion of the given keys.
pub struct BatchDelete {
    state: RequestState<BatchDeleteInner>,
}

impl BatchDelete {
    pub fn new(client: Arc<RpcClient>, keys: Vec<Key>) -> Self {
        Self {
            state: RequestState::new(client, BatchDeleteInner { keys }),
        }
    }

    /// Set the (optional) [`ColumnFamily`](ColumnFamily).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.state.cf(cf);
        self
    }
}

impl Future for BatchDelete {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).state).poll(cx) }
    }
}

struct BatchDeleteInner {
    keys: Vec<Key>,
}

impl RequestInner for BatchDeleteInner {
    type Resp = ();
    existential type F: Future<Output = Result<()>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F {
        client.raw_batch_delete(self.keys, cf)
    }
}

/// An unresolved [`Client::scan`](Client::scan) request.
///
/// Once resolved this request will result in a scanner over the given range.
pub struct Scan {
    state: RequestState<ScanInner>,
}

impl Scan {
    pub fn new(client: Arc<RpcClient>, range: BoundRange, limit: u32) -> Self {
        Scan {
            state: RequestState::new(client, ScanInner::new(range, limit)),
        }
    }

    /// Set the (optional) [`ColumnFamily`](ColumnFamily).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.state.cf(cf);
        self
    }

    pub fn key_only(mut self) -> Self {
        if let Some(x) = self.state.inner_mut() {
            x.key_only = true;
        };
        self
    }
}

impl Future for Scan {
    type Output = Result<Vec<KvPair>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).state).poll(cx) }
    }
}

struct ScanInner {
    range: BoundRange,
    limit: u32,
    key_only: bool,
}

impl ScanInner {
    fn new(range: BoundRange, limit: u32) -> Self {
        ScanInner {
            range,
            limit,
            key_only: false,
        }
    }
}

impl RequestInner for ScanInner {
    type Resp = Vec<KvPair>;
    existential type F: Future<Output = Result<Vec<KvPair>>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F {
        if self.limit > MAX_RAW_KV_SCAN_LIMIT {
            Either::Right(future::err(Error::max_scan_limit_exceeded(
                self.limit,
                MAX_RAW_KV_SCAN_LIMIT,
            )))
        } else {
            Either::Left(client.raw_scan(self.range, self.limit, self.key_only, cf))
        }
    }
}

/// An unresolved [`Client::batch_scan`](Client::batch_scan) request.
///
/// Once resolved this request will result in a scanner over the given ranges.
pub struct BatchScan {
    state: RequestState<BatchScanInner>,
}

impl BatchScan {
    pub fn new(client: Arc<RpcClient>, ranges: Vec<BoundRange>, each_limit: u32) -> Self {
        BatchScan {
            state: RequestState::new(client, BatchScanInner::new(ranges, each_limit)),
        }
    }

    /// Set the (optional) [`ColumnFamily`](ColumnFamily).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.state.cf(cf);
        self
    }

    pub fn key_only(mut self) -> Self {
        if let Some(x) = self.state.inner_mut() {
            x.key_only = true;
        };
        self
    }
}

impl Future for BatchScan {
    type Output = Result<Vec<KvPair>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).state).poll(cx) }
    }
}

struct BatchScanInner {
    ranges: Vec<BoundRange>,
    each_limit: u32,
    key_only: bool,
}

impl BatchScanInner {
    fn new(ranges: Vec<BoundRange>, each_limit: u32) -> Self {
        BatchScanInner {
            ranges,
            each_limit,
            key_only: false,
        }
    }
}

impl RequestInner for BatchScanInner {
    type Resp = Vec<KvPair>;
    existential type F: Future<Output = Result<Vec<KvPair>>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F {
        if self.each_limit > MAX_RAW_KV_SCAN_LIMIT {
            Either::Right(future::err(Error::max_scan_limit_exceeded(
                self.each_limit,
                MAX_RAW_KV_SCAN_LIMIT,
            )))
        } else {
            Either::Left(client.raw_batch_scan(self.ranges, self.each_limit, self.key_only, cf))
        }
    }
}

/// An unresolved [`Client::delete_range`](Client::delete_range) request.
///
/// Once resolved this request will result in the deletion of the values in the given
/// range.
pub struct DeleteRange {
    state: RequestState<DeleteRangeInner>,
}

impl DeleteRange {
    pub fn new(client: Arc<RpcClient>, range: BoundRange) -> Self {
        Self {
            state: RequestState::new(client, DeleteRangeInner { range }),
        }
    }

    /// Set the (optional) [`ColumnFamily`](ColumnFamily).
    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.state.cf(cf);
        self
    }
}

impl Future for DeleteRange {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).state).poll(cx) }
    }
}

pub struct DeleteRangeInner {
    range: BoundRange,
}

impl RequestInner for DeleteRangeInner {
    type Resp = ();
    existential type F: Future<Output = Result<()>>;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> Self::F {
        client.raw_delete_range(self.range, cf)
    }
}
