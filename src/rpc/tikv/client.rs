// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

// TODO: Remove this when txn is done.
#![allow(dead_code)]

use std::{fmt, sync::Arc, time::Duration};

use futures::Future;
use grpcio::{CallOption, Environment};
use kvproto::{errorpb, kvrpcpb, tikvpb::TikvClient};

use crate::{
    rpc::{
        client::{RawContext, TxnContext},
        security::SecurityManager,
        tikv::context::{request_context, RequestContext},
    },
    transaction::{Mutation, TxnInfo},
    Error, ErrorKind, Key, KvPair, Result, Value,
};

trait HasRegionError {
    fn region_error(&mut self) -> Option<Error>;
}

trait HasError {
    fn error(&mut self) -> Option<Error>;
}

impl From<errorpb::Error> for Error {
    fn from(mut e: errorpb::Error) -> Error {
        let message = e.take_message();
        if e.has_not_leader() {
            let e = e.get_not_leader();
            let message = format!("{}. Leader: {:?}", message, e.get_leader());
            Error::not_leader(e.get_region_id(), Some(message))
        } else if e.has_region_not_found() {
            Error::region_not_found(e.get_region_not_found().get_region_id(), Some(message))
        } else if e.has_key_not_in_region() {
            let e = e.take_key_not_in_region();
            Error::key_not_in_region(e)
        } else if e.has_epoch_not_match() {
            Error::stale_epoch(Some(format!(
                "{}. New epoch: {:?}",
                message,
                e.get_epoch_not_match().get_current_regions()
            )))
        } else if e.has_server_is_busy() {
            Error::server_is_busy(e.take_server_is_busy())
        } else if e.has_stale_command() {
            Error::stale_command(message)
        } else if e.has_store_not_match() {
            Error::store_not_match(e.take_store_not_match(), message)
        } else if e.has_raft_entry_too_large() {
            Error::raft_entry_too_large(e.take_raft_entry_too_large(), message)
        } else {
            Error::internal_error(message)
        }
    }
}

macro_rules! has_region_error {
    ($type:ty) => {
        impl HasRegionError for $type {
            fn region_error(&mut self) -> Option<Error> {
                if self.has_region_error() {
                    Some(self.take_region_error().into())
                } else {
                    None
                }
            }
        }
    };
}

has_region_error!(kvrpcpb::GetResponse);
has_region_error!(kvrpcpb::ScanResponse);
has_region_error!(kvrpcpb::PrewriteResponse);
has_region_error!(kvrpcpb::CommitResponse);
has_region_error!(kvrpcpb::ImportResponse);
has_region_error!(kvrpcpb::BatchRollbackResponse);
has_region_error!(kvrpcpb::CleanupResponse);
has_region_error!(kvrpcpb::BatchGetResponse);
has_region_error!(kvrpcpb::ScanLockResponse);
has_region_error!(kvrpcpb::ResolveLockResponse);
has_region_error!(kvrpcpb::GcResponse);
has_region_error!(kvrpcpb::RawGetResponse);
has_region_error!(kvrpcpb::RawBatchGetResponse);
has_region_error!(kvrpcpb::RawPutResponse);
has_region_error!(kvrpcpb::RawBatchPutResponse);
has_region_error!(kvrpcpb::RawDeleteResponse);
has_region_error!(kvrpcpb::RawBatchDeleteResponse);
has_region_error!(kvrpcpb::DeleteRangeResponse);
has_region_error!(kvrpcpb::RawDeleteRangeResponse);
has_region_error!(kvrpcpb::RawScanResponse);
has_region_error!(kvrpcpb::RawBatchScanResponse);

macro_rules! has_key_error {
    ($type:ty) => {
        impl HasError for $type {
            fn error(&mut self) -> Option<Error> {
                if self.has_error() {
                    Some(self.take_error().into())
                } else {
                    None
                }
            }
        }
    };
}

has_key_error!(kvrpcpb::GetResponse);
has_key_error!(kvrpcpb::CommitResponse);
has_key_error!(kvrpcpb::BatchRollbackResponse);
has_key_error!(kvrpcpb::CleanupResponse);
has_key_error!(kvrpcpb::ScanLockResponse);
has_key_error!(kvrpcpb::ResolveLockResponse);
has_key_error!(kvrpcpb::GcResponse);

macro_rules! has_str_error {
    ($type:ty) => {
        impl HasError for $type {
            fn error(&mut self) -> Option<Error> {
                if self.get_error().is_empty() {
                    None
                } else {
                    Some(Error::kv_error(self.take_error()))
                }
            }
        }
    };
}

has_str_error!(kvrpcpb::RawGetResponse);
has_str_error!(kvrpcpb::RawPutResponse);
has_str_error!(kvrpcpb::RawBatchPutResponse);
has_str_error!(kvrpcpb::RawDeleteResponse);
has_str_error!(kvrpcpb::RawBatchDeleteResponse);
has_str_error!(kvrpcpb::RawDeleteRangeResponse);
has_str_error!(kvrpcpb::ImportResponse);
has_str_error!(kvrpcpb::DeleteRangeResponse);

macro_rules! has_no_error {
    ($type:ty) => {
        impl HasError for $type {
            fn error(&mut self) -> Option<Error> {
                None
            }
        }
    };
}

has_no_error!(kvrpcpb::ScanResponse);
has_no_error!(kvrpcpb::PrewriteResponse);
has_no_error!(kvrpcpb::BatchGetResponse);
has_no_error!(kvrpcpb::RawBatchGetResponse);
has_no_error!(kvrpcpb::RawScanResponse);
has_no_error!(kvrpcpb::RawBatchScanResponse);

macro_rules! raw_request {
    ($context:expr, $type:ty) => {{
        let mut req = <$type>::default();
        let (region, cf) = $context.into_inner();
        req.set_context(region.into());
        if let Some(cf) = cf {
            req.set_cf(cf.into_inner());
        }
        req
    }};
}

macro_rules! txn_request {
    ($context:expr, $type:ty) => {{
        let mut req = <$type>::default();
        req.set_context($context.into_inner().into());
        req
    }};
}

impl From<Mutation> for kvrpcpb::Mutation {
    fn from(mutation: Mutation) -> kvrpcpb::Mutation {
        let mut pb = kvrpcpb::Mutation::default();
        match mutation {
            Mutation::Put(k, v) => {
                pb.set_op(kvrpcpb::Op::Put);
                pb.set_key(k.into_inner());
                pb.set_value(v.into_inner());
            }
            Mutation::Del(k) => {
                pb.set_op(kvrpcpb::Op::Del);
                pb.set_key(k.into_inner());
            }
            Mutation::Lock(k) => {
                pb.set_op(kvrpcpb::Op::Lock);
                pb.set_key(k.into_inner());
            }
            Mutation::Rollback(k) => {
                pb.set_op(kvrpcpb::Op::Rollback);
                pb.set_key(k.into_inner());
            }
        };
        pb
    }
}

impl From<TxnInfo> for kvrpcpb::TxnInfo {
    fn from(txn_info: TxnInfo) -> kvrpcpb::TxnInfo {
        let mut pb = kvrpcpb::TxnInfo::default();
        pb.set_txn(txn_info.txn);
        pb.set_status(txn_info.status);
        pb
    }
}

pub struct KvClient {
    client: Arc<TikvClient>,
    timeout: Duration,
    address: String,
}

impl KvClient {
    pub fn connect(
        env: Arc<Environment>,
        addr: &str,
        security_mgr: &Arc<SecurityManager>,
        timeout: Duration,
    ) -> Result<KvClient> {
        let client = Arc::new(security_mgr.connect(env, addr, TikvClient::new)?);
        Ok(KvClient {
            client,
            timeout,
            address: addr.to_owned(),
        })
    }

    pub fn kv_get(
        &self,
        context: TxnContext,
        version: u64,
        key: Key,
    ) -> impl Future<Item = kvrpcpb::GetResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::GetRequest);
        req.set_key(key.into_inner());
        req.set_version(version);

        self.execute(request_context(
            "kv_get",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_get_async_opt(&req, opt),
        ))
    }

    pub fn kv_scan(
        &self,
        context: TxnContext,
        version: u64,
        start_key: Key,
        end_key: Key,
        limit: u32,
        key_only: bool,
    ) -> impl Future<Item = kvrpcpb::ScanResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::ScanRequest);
        req.set_start_key(start_key.into_inner());
        req.set_end_key(end_key.into_inner());
        req.set_version(version);
        req.set_limit(limit);
        req.set_key_only(key_only);

        self.execute(request_context(
            "kv_scan",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_scan_async_opt(&req, opt),
        ))
    }

    pub fn kv_prewrite(
        &self,
        context: TxnContext,
        mutations: impl Iterator<Item = Mutation>,
        primary_lock: Key,
        start_version: u64,
        lock_ttl: u64,
        skip_constraint_check: bool,
    ) -> impl Future<Item = kvrpcpb::PrewriteResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::PrewriteRequest);
        req.set_mutations(mutations.map(Into::into).collect());
        req.set_primary_lock(primary_lock.into_inner());
        req.set_start_version(start_version);
        req.set_lock_ttl(lock_ttl);
        req.set_skip_constraint_check(skip_constraint_check);

        self.execute(request_context(
            "kv_prewrite",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_prewrite_async_opt(&req, opt),
        ))
    }

    pub fn kv_commit(
        &self,
        context: TxnContext,
        keys: impl Iterator<Item = Key>,
        start_version: u64,
        commit_version: u64,
    ) -> impl Future<Item = kvrpcpb::CommitResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::CommitRequest);
        req.set_keys(keys.map(|x| x.into_inner()).collect());
        req.set_start_version(start_version);
        req.set_commit_version(commit_version);

        self.execute(request_context(
            "kv_commit",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_commit_async_opt(&req, opt),
        ))
    }

    pub fn kv_import(
        &self,
        mutations: impl Iterator<Item = Mutation>,
        commit_version: u64,
    ) -> impl Future<Item = kvrpcpb::ImportResponse, Error = Error> {
        let mut req = kvrpcpb::ImportRequest::default();
        req.set_mutations(mutations.map(Into::into).collect());
        req.set_commit_version(commit_version);

        self.execute(request_context(
            "kv_import",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_import_async_opt(&req, opt),
        ))
    }

    pub fn kv_cleanup(
        &self,
        context: TxnContext,
        key: Key,
        start_version: u64,
    ) -> impl Future<Item = kvrpcpb::CleanupResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::CleanupRequest);
        req.set_key(key.into_inner());
        req.set_start_version(start_version);

        self.execute(request_context(
            "kv_cleanup",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_cleanup_async_opt(&req, opt),
        ))
    }

    pub fn kv_batch_get(
        &self,
        context: TxnContext,
        keys: impl Iterator<Item = Key>,
        version: u64,
    ) -> impl Future<Item = kvrpcpb::BatchGetResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::BatchGetRequest);
        req.set_keys(keys.map(|x| x.into_inner()).collect());
        req.set_version(version);

        self.execute(request_context(
            "kv_batch_get",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_batch_get_async_opt(&req, opt),
        ))
    }

    pub fn kv_batch_rollback(
        &self,
        context: TxnContext,
        keys: impl Iterator<Item = Key>,
        start_version: u64,
    ) -> impl Future<Item = kvrpcpb::BatchRollbackResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::BatchRollbackRequest);
        req.set_keys(keys.map(|x| x.into_inner()).collect());
        req.set_start_version(start_version);

        self.execute(request_context(
            "kv_batch_rollback",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_batch_rollback_async_opt(&req, opt),
        ))
    }

    pub fn kv_scan_lock(
        &self,
        context: TxnContext,
        start_key: Key,
        max_version: u64,
        limit: u32,
    ) -> impl Future<Item = kvrpcpb::ScanLockResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::ScanLockRequest);
        req.set_start_key(start_key.into_inner());
        req.set_max_version(max_version);
        req.set_limit(limit);

        self.execute(request_context(
            "kv_scan_lock",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_scan_lock_async_opt(&req, opt),
        ))
    }

    pub fn kv_resolve_lock(
        &self,
        context: TxnContext,
        txn_infos: impl Iterator<Item = TxnInfo>,
        start_version: u64,
        commit_version: u64,
    ) -> impl Future<Item = kvrpcpb::ResolveLockResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::ResolveLockRequest);
        req.set_start_version(start_version);
        req.set_commit_version(commit_version);
        req.set_txn_infos(txn_infos.map(Into::into).collect());

        self.execute(request_context(
            "kv_resolve_lock",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_resolve_lock_async_opt(&req, opt),
        ))
    }

    pub fn kv_gc(
        &self,
        context: TxnContext,
        safe_point: u64,
    ) -> impl Future<Item = kvrpcpb::GcResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::GcRequest);
        req.set_safe_point(safe_point);

        self.execute(request_context(
            "kv_gc",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_gc_async_opt(&req, opt),
        ))
    }

    pub fn kv_delete_range(
        &self,
        context: TxnContext,
        start_key: Key,
        end_key: Key,
    ) -> impl Future<Item = kvrpcpb::DeleteRangeResponse, Error = Error> {
        let mut req = txn_request!(context, kvrpcpb::DeleteRangeRequest);
        req.set_start_key(start_key.into_inner());
        req.set_end_key(end_key.into_inner());

        self.execute(request_context(
            "kv_delete_range",
            move |cli: Arc<TikvClient>, opt: _| cli.kv_delete_range_async_opt(&req, opt),
        ))
    }

    pub fn raw_get(
        &self,
        context: RawContext,
        key: Key,
    ) -> impl Future<Item = Value, Error = Error> {
        let mut req = raw_request!(context, kvrpcpb::RawGetRequest);
        req.set_key(key.into_inner());

        self.execute(request_context(
            "raw_get",
            move |cli: Arc<TikvClient>, opt: _| cli.raw_get_async_opt(&req, opt),
        ))
        .map(|mut resp| resp.take_value().into())
    }

    pub fn raw_batch_get(
        &self,
        context: RawContext,
        keys: impl Iterator<Item = Key>,
    ) -> impl Future<Item = Vec<KvPair>, Error = Error> {
        let mut req = raw_request!(context, kvrpcpb::RawBatchGetRequest);
        req.set_keys(keys.map(|x| x.into_inner()).collect());

        self.execute(request_context(
            "raw_batch_get",
            move |cli: Arc<TikvClient>, opt: _| cli.raw_batch_get_async_opt(&req, opt),
        ))
        .map(|mut resp| Self::convert_from_grpc_pairs(resp.take_pairs()))
    }

    pub fn raw_put(
        &self,
        context: RawContext,
        key: Key,
        value: Value,
    ) -> impl Future<Item = (), Error = Error> {
        let mut req = raw_request!(context, kvrpcpb::RawPutRequest);
        req.set_key(key.into_inner());
        req.set_value(value.into_inner());

        self.execute(request_context(
            "raw_put",
            move |cli: Arc<TikvClient>, opt: _| cli.raw_put_async_opt(&req, opt),
        ))
        .map(|_| ())
    }

    pub fn raw_batch_put(
        &self,
        context: RawContext,
        pairs: Vec<KvPair>,
    ) -> impl Future<Item = (), Error = Error> {
        let mut req = raw_request!(context, kvrpcpb::RawBatchPutRequest);
        req.set_pairs(Self::convert_to_grpc_pairs(pairs));

        self.execute(request_context(
            "raw_batch_put",
            move |cli: Arc<TikvClient>, opt: _| cli.raw_batch_put_async_opt(&req, opt),
        ))
        .map(|_| ())
    }

    pub fn raw_delete(
        &self,
        context: RawContext,
        key: Key,
    ) -> impl Future<Item = (), Error = Error> {
        let mut req = raw_request!(context, kvrpcpb::RawDeleteRequest);
        req.set_key(key.into_inner());

        self.execute(request_context(
            "raw_delete",
            move |cli: Arc<TikvClient>, opt: _| cli.raw_delete_async_opt(&req, opt),
        ))
        .map(|_| ())
    }

    pub fn raw_batch_delete(
        &self,
        context: RawContext,
        keys: Vec<Key>,
    ) -> impl Future<Item = (), Error = Error> {
        let mut req = raw_request!(context, kvrpcpb::RawBatchDeleteRequest);
        req.set_keys(keys.into_iter().map(|x| x.into_inner()).collect());

        self.execute(request_context(
            "raw_batch_delete",
            move |cli: Arc<TikvClient>, opt: _| cli.raw_batch_delete_async_opt(&req, opt),
        ))
        .map(|_| ())
    }

    pub fn raw_scan(
        &self,
        context: RawContext,
        start_key: Option<Key>,
        end_key: Option<Key>,
        limit: u32,
        key_only: bool,
    ) -> impl Future<Item = Vec<KvPair>, Error = Error> {
        let mut req = raw_request!(context, kvrpcpb::RawScanRequest);
        start_key
            .map(|k| req.set_start_key(k.into_inner()))
            .unwrap();
        end_key.map(|k| req.set_end_key(k.into_inner())).unwrap();
        req.set_limit(limit);
        req.set_key_only(key_only);

        self.execute(request_context(
            "raw_scan",
            move |cli: Arc<TikvClient>, opt: _| cli.raw_scan_async_opt(&req, opt),
        ))
        .map(|mut resp| Self::convert_from_grpc_pairs(resp.take_kvs()))
    }

    pub fn raw_batch_scan(
        &self,
        context: RawContext,
        ranges: impl Iterator<Item = (Option<Key>, Option<Key>)>,
        each_limit: u32,
        key_only: bool,
    ) -> impl Future<Item = Vec<KvPair>, Error = Error> {
        let mut req = raw_request!(context, kvrpcpb::RawBatchScanRequest);
        req.set_ranges(Self::convert_to_grpc_ranges(ranges));
        req.set_each_limit(each_limit);
        req.set_key_only(key_only);

        self.execute(request_context(
            "raw_batch_scan",
            move |cli: Arc<TikvClient>, opt: _| cli.raw_batch_scan_async_opt(&req, opt),
        ))
        .map(|mut resp| Self::convert_from_grpc_pairs(resp.take_kvs()))
    }

    pub fn raw_delete_range(
        &self,
        context: RawContext,
        start_key: Key,
        end_key: Key,
    ) -> impl Future<Item = (), Error = Error> {
        let mut req = raw_request!(context, kvrpcpb::RawDeleteRangeRequest);
        req.set_start_key(start_key.into_inner());
        req.set_end_key(end_key.into_inner());

        self.execute(request_context(
            "raw_delete_range",
            move |cli: Arc<TikvClient>, opt: _| cli.raw_delete_range_async_opt(&req, opt),
        ))
        .map(|_| ())
    }

    fn execute<Executor, Resp, RpcFuture>(
        &self,
        mut context: RequestContext<Executor>,
    ) -> impl Future<Item = Resp, Error = Error>
    where
        Executor: FnOnce(Arc<TikvClient>, CallOption) -> ::grpcio::Result<RpcFuture>,
        RpcFuture: Future<Item = Resp, Error = ::grpcio::Error>,
        Resp: HasRegionError + HasError + Sized + Clone,
    {
        let executor = context.executor();
        executor(
            Arc::clone(&self.client),
            CallOption::default().timeout(self.timeout),
        )
        .unwrap()
        .then(|r| match r {
            Err(e) => Err(ErrorKind::Grpc(e))?,
            Ok(mut r) => {
                if let Some(e) = r.region_error() {
                    Err(e)
                } else if let Some(e) = r.error() {
                    Err(e)
                } else {
                    Ok(r)
                }
            }
        })
        .then(move |r| context.done(r))
    }

    #[inline]
    fn convert_to_grpc_pair(pair: KvPair) -> kvrpcpb::KvPair {
        let mut result = kvrpcpb::KvPair::default();
        let (key, value) = pair.into_inner();
        result.set_key(key.into_inner());
        result.set_value(value.into_inner());
        result
    }

    #[inline]
    fn convert_to_grpc_pairs(pairs: Vec<KvPair>) -> Vec<kvrpcpb::KvPair> {
        pairs.into_iter().map(Self::convert_to_grpc_pair).collect()
    }

    #[inline]
    fn convert_from_grpc_pair(mut pair: kvrpcpb::KvPair) -> KvPair {
        KvPair::new(Key::from(pair.take_key()), Value::from(pair.take_value()))
    }

    #[inline]
    fn convert_from_grpc_pairs(pairs: Vec<kvrpcpb::KvPair>) -> Vec<KvPair> {
        pairs
            .into_iter()
            .map(Self::convert_from_grpc_pair)
            .collect()
    }

    #[inline]
    fn convert_to_grpc_range(range: (Option<Key>, Option<Key>)) -> kvrpcpb::KeyRange {
        let (start, end) = range;
        let mut range = kvrpcpb::KeyRange::default();
        start.map(|k| range.set_start_key(k.into_inner())).unwrap();
        end.map(|k| range.set_end_key(k.into_inner())).unwrap();
        range
    }

    #[inline]
    fn convert_to_grpc_ranges(
        ranges: impl Iterator<Item = (Option<Key>, Option<Key>)>,
    ) -> Vec<kvrpcpb::KeyRange> {
        ranges.map(Self::convert_to_grpc_range).collect()
    }
}

impl fmt::Debug for KvClient {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("KvClient")
            .field("address", &self.address)
            .field("timeout", &self.timeout)
            .finish()
    }
}
