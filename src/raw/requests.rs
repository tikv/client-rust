// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::ColumnFamily;
use crate::{rpc::RpcClient, Error, Key, BoundRange, KvPair, Result, Value};
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::{ops::Bound, pin::Pin, sync::Arc, u32};

const MAX_RAW_KV_SCAN_LIMIT: u32 = 10240;

type BoxTryFuture<Resp> = Box<dyn Future<Output = Result<Resp>> + Send>;

trait RequestInner: Sized {
    type Resp;

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> BoxTryFuture<Self::Resp>;
}

enum RequestState<Inner>
where
    Inner: RequestInner,
{
    Uninitiated(Option<(Arc<RpcClient>, Inner, Option<ColumnFamily>)>),
    Initiated(BoxTryFuture<Inner::Resp>),
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
                RequestState::Initiated(future) => Pin::new_unchecked(&mut **future),
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
    pub fn new(client: Arc<RpcClient>, inner: GetInner) -> Self {
        Self {
            state: RequestState::new(client, inner),
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

pub struct GetInner {
    key: Key,
}

impl GetInner {
    pub fn new(key: Key) -> Self {
        GetInner { key }
    }
}

impl RequestInner for GetInner {
    type Resp = Option<Value>;

    fn execute(
        self,
        client: Arc<RpcClient>,
        cf: Option<ColumnFamily>,
    ) -> BoxTryFuture<Option<Value>> {
        Box::new(client.raw_get(self.key, cf))
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
    pub fn new(client: Arc<RpcClient>, inner: BatchGetInner) -> Self {
        Self {
            state: RequestState::new(client, inner),
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

pub struct BatchGetInner {
    keys: Vec<Key>,
}

impl RequestInner for BatchGetInner {
    type Resp = Vec<KvPair>;

    fn execute(
        self,
        client: Arc<RpcClient>,
        cf: Option<ColumnFamily>,
    ) -> BoxTryFuture<Vec<KvPair>> {
        Box::new(client.raw_batch_get(self.keys, cf))
    }
}

impl BatchGetInner {
    pub fn new(keys: Vec<Key>) -> Self {
        BatchGetInner { keys }
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
    pub fn new(client: Arc<RpcClient>, inner: PutInner) -> Self {
        Self {
            state: RequestState::new(client, inner),
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

pub struct PutInner {
    key: Key,
    value: Value,
}

impl PutInner {
    pub fn new(key: Key, value: Value) -> Self {
        PutInner { key, value }
    }
}

impl RequestInner for PutInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> BoxTryFuture<()> {
        let (key, value) = (self.key, self.value);
        Box::new(client.raw_put(key, value, cf))
    }
}

/// An unresolved [`Client::batch_put`](Client::batch_put) request.
///
/// Once resolved this request will result in the setting of the value associated with the given key.
pub struct BatchPut {
    state: RequestState<BatchPutInner>,
}

impl BatchPut {
    pub fn new(client: Arc<RpcClient>, inner: BatchPutInner) -> Self {
        Self {
            state: RequestState::new(client, inner),
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

pub struct BatchPutInner {
    pairs: Vec<KvPair>,
}

impl BatchPutInner {
    pub fn new(pairs: Vec<KvPair>) -> Self {
        BatchPutInner { pairs }
    }
}

impl RequestInner for BatchPutInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> BoxTryFuture<()> {
        Box::new(client.raw_batch_put(self.pairs, cf))
    }
}

/// An unresolved [`Client::delete`](Client::delete) request.
///
/// Once resolved this request will result in the deletion of the given key.
pub struct Delete {
    state: RequestState<DeleteInner>,
}

impl Delete {
    pub fn new(client: Arc<RpcClient>, inner: DeleteInner) -> Self {
        Self {
            state: RequestState::new(client, inner),
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

pub struct DeleteInner {
    key: Key,
}

impl DeleteInner {
    pub fn new(key: Key) -> Self {
        DeleteInner { key }
    }
}

impl RequestInner for DeleteInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> BoxTryFuture<()> {
        Box::new(client.raw_delete(self.key, cf))
    }
}

/// An unresolved [`Client::batch_delete`](Client::batch_delete) request.
///
/// Once resolved this request will result in the deletion of the given keys.
pub struct BatchDelete {
    state: RequestState<BatchDeleteInner>,
}

impl BatchDelete {
    pub fn new(client: Arc<RpcClient>, inner: BatchDeleteInner) -> Self {
        Self {
            state: RequestState::new(client, inner),
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

pub struct BatchDeleteInner {
    keys: Vec<Key>,
}

impl BatchDeleteInner {
    pub fn new(keys: Vec<Key>) -> Self {
        BatchDeleteInner { keys }
    }
}

impl RequestInner for BatchDeleteInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> BoxTryFuture<()> {
        Box::new(client.raw_batch_delete(self.keys, cf))
    }
}

/// An unresolved [`Client::scan`](Client::scan) request.
///
/// Once resolved this request will result in a scanner over the given range.
pub struct Scan {
    state: RequestState<ScanInner>,
}

impl Scan {
    pub fn new(client: Arc<RpcClient>, inner: ScanInner) -> Self {
        Scan {
            state: RequestState::new(client, inner),
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

pub struct ScanInner {
    range: BoundRange,
    limit: u32,
    key_only: bool,
}

impl ScanInner {
    pub fn new(range: BoundRange, limit: u32) -> Self {
        ScanInner {
            range,
            limit,
            key_only: false,
        }
    }
}

impl RequestInner for ScanInner {
    type Resp = Vec<KvPair>;

    fn execute(
        self,
        client: Arc<RpcClient>,
        cf: Option<ColumnFamily>,
    ) -> BoxTryFuture<Vec<KvPair>> {
        if self.limit > MAX_RAW_KV_SCAN_LIMIT {
            Box::new(future::err(Error::max_scan_limit_exceeded(
                self.limit,
                MAX_RAW_KV_SCAN_LIMIT,
            )))
        } else {
            Box::new(client.raw_scan(self.range, self.limit, self.key_only, cf))
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
    pub fn new(client: Arc<RpcClient>, inner: BatchScanInner) -> Self {
        BatchScan {
            state: RequestState::new(client, inner),
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

pub struct BatchScanInner {
    ranges: Vec<BoundRange>,
    each_limit: u32,
    key_only: bool,
}

impl BatchScanInner {
    pub fn new(ranges: Vec<BoundRange>, each_limit: u32) -> Self {
        BatchScanInner {
            ranges,
            each_limit,
            key_only: false,
        }
    }
}

impl RequestInner for BatchScanInner {
    type Resp = Vec<KvPair>;

    fn execute(
        self,
        client: Arc<RpcClient>,
        cf: Option<ColumnFamily>,
    ) -> BoxTryFuture<Vec<KvPair>> {
        if self.each_limit > MAX_RAW_KV_SCAN_LIMIT {
            Box::new(future::err(Error::max_scan_limit_exceeded(
                self.each_limit,
                MAX_RAW_KV_SCAN_LIMIT,
            )))
        } else {
            Box::new(client.raw_batch_scan(self.ranges, self.each_limit, self.key_only, cf))
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
    pub fn new(client: Arc<RpcClient>, inner: DeleteRangeInner) -> Self {
        Self {
            state: RequestState::new(client, inner),
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

impl DeleteRangeInner {
    pub fn new(range: BoundRange) -> Self {
        DeleteRangeInner { range }
    }
}

impl RequestInner for DeleteRangeInner {
    type Resp = ();

    fn execute(self, client: Arc<RpcClient>, cf: Option<ColumnFamily>) -> BoxTryFuture<()> {
        Box::new(client.raw_delete_range(self.range, cf))
    }
}
