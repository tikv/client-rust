// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

// FIXME: Remove this when txn is done.
#![allow(dead_code)]

use derive_new::new;
use futures::compat::Compat01As03;
use futures::future::BoxFuture;
use futures::prelude::*;
use grpcio::CallOption;
use kvproto::kvrpcpb;
use kvproto::tikvpb::TikvClient;
use std::{sync::Arc, time::Duration};

use crate::{
    kv_client::HasError, pd::Region, raw::RawRequest, stats::tikv_stats, transaction::TxnInfo,
    ErrorKind, Key, Result,
};

/// This client handles requests for a single TiKV node. It converts the data
/// types and abstractions of the client program into the grpc data types.
#[derive(new, Clone)]
pub struct KvRpcClient {
    rpc_client: Arc<TikvClient>,
}

impl super::KvClient for KvRpcClient {
    fn dispatch<T: RawRequest>(
        &self,
        request: &T::RpcRequest,
        opt: CallOption,
    ) -> BoxFuture<'static, Result<T::RpcResponse>> {
        map_errors_and_trace(T::REQUEST_NAME, T::RPC_FN(&self.rpc_client, request, opt)).boxed()
    }
}

pub struct TransactionRegionClient {
    region: Region,
    timeout: Duration,
    client: Arc<TikvClient>,
}

// FIXME use `request` method instead.
macro_rules! txn_request {
    ($region:expr, $type:ty) => {{
        let mut req = <$type>::default();
        // FIXME don't unwrap
        req.set_context($region.context().unwrap());
        req
    }};
}

impl From<TxnInfo> for kvrpcpb::TxnInfo {
    fn from(txn_info: TxnInfo) -> kvrpcpb::TxnInfo {
        let mut pb = kvrpcpb::TxnInfo::default();
        pb.set_txn(txn_info.txn);
        pb.set_status(txn_info.status);
        pb
    }
}

impl TransactionRegionClient {
    pub fn kv_get(
        &self,
        version: u64,
        key: Key,
    ) -> impl Future<Output = Result<kvrpcpb::GetResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::GetRequest);
        req.set_key(key.into());
        req.set_version(version);

        map_errors_and_trace(
            "kv_get",
            self.client
                .clone()
                .kv_get_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_scan(
        &self,
        version: u64,
        start_key: Key,
        end_key: Key,
        limit: u32,
        key_only: bool,
    ) -> impl Future<Output = Result<kvrpcpb::ScanResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::ScanRequest);
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());
        req.set_version(version);
        req.set_limit(limit);
        req.set_key_only(key_only);

        map_errors_and_trace(
            "kv_scan",
            self.client
                .clone()
                .kv_scan_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_prewrite(
        &self,
        mutations: impl Iterator<Item = kvrpcpb::Mutation>,
        primary_lock: Key,
        start_version: u64,
        lock_ttl: u64,
        skip_constraint_check: bool,
    ) -> impl Future<Output = Result<kvrpcpb::PrewriteResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::PrewriteRequest);
        req.set_mutations(mutations.collect());
        req.set_primary_lock(primary_lock.into());
        req.set_start_version(start_version);
        req.set_lock_ttl(lock_ttl);
        req.set_skip_constraint_check(skip_constraint_check);

        map_errors_and_trace(
            "kv_prewrite",
            self.client
                .clone()
                .kv_prewrite_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_commit(
        &self,
        keys: impl Iterator<Item = Key>,
        start_version: u64,
        commit_version: u64,
    ) -> impl Future<Output = Result<kvrpcpb::CommitResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::CommitRequest);
        req.set_keys(keys.map(|x| x.into()).collect());
        req.set_start_version(start_version);
        req.set_commit_version(commit_version);

        map_errors_and_trace(
            "kv_commit",
            self.client
                .clone()
                .kv_commit_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_import(
        &self,
        mutations: impl Iterator<Item = kvrpcpb::Mutation>,
        commit_version: u64,
    ) -> impl Future<Output = Result<kvrpcpb::ImportResponse>> {
        let mut req = kvrpcpb::ImportRequest::default();
        req.set_mutations(mutations.collect());
        req.set_commit_version(commit_version);

        map_errors_and_trace(
            "kv_import",
            self.client
                .clone()
                .kv_import_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_cleanup(
        &self,
        key: Key,
        start_version: u64,
    ) -> impl Future<Output = Result<kvrpcpb::CleanupResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::CleanupRequest);
        req.set_key(key.into());
        req.set_start_version(start_version);

        map_errors_and_trace(
            "kv_cleanup",
            self.client
                .clone()
                .kv_cleanup_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_batch_get(
        &self,
        keys: impl Iterator<Item = Key>,
        version: u64,
    ) -> impl Future<Output = Result<kvrpcpb::BatchGetResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::BatchGetRequest);
        req.set_keys(keys.map(|x| x.into()).collect());
        req.set_version(version);

        map_errors_and_trace(
            "kv_batch_get",
            self.client
                .clone()
                .kv_batch_get_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_batch_rollback(
        &self,
        keys: impl Iterator<Item = Key>,
        start_version: u64,
    ) -> impl Future<Output = Result<kvrpcpb::BatchRollbackResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::BatchRollbackRequest);
        req.set_keys(keys.map(|x| x.into()).collect());
        req.set_start_version(start_version);

        map_errors_and_trace(
            "kv_batch_rollback",
            self.client
                .clone()
                .kv_batch_rollback_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_scan_lock(
        &self,
        start_key: Key,
        max_version: u64,
        limit: u32,
    ) -> impl Future<Output = Result<kvrpcpb::ScanLockResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::ScanLockRequest);
        req.set_start_key(start_key.into());
        req.set_max_version(max_version);
        req.set_limit(limit);

        map_errors_and_trace(
            "kv_scan_lock",
            self.client
                .clone()
                .kv_scan_lock_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_resolve_lock(
        &self,
        txn_infos: impl Iterator<Item = TxnInfo>,
        start_version: u64,
        commit_version: u64,
    ) -> impl Future<Output = Result<kvrpcpb::ResolveLockResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::ResolveLockRequest);
        req.set_start_version(start_version);
        req.set_commit_version(commit_version);
        req.set_txn_infos(txn_infos.map(Into::into).collect());

        map_errors_and_trace(
            "kv_resolve_lock",
            self.client
                .clone()
                .kv_resolve_lock_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_gc(&self, safe_point: u64) -> impl Future<Output = Result<kvrpcpb::GcResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::GcRequest);
        req.set_safe_point(safe_point);

        map_errors_and_trace(
            "kv_gc",
            self.client
                .clone()
                .kv_gc_async_opt(&req, self.call_options()),
        )
    }

    pub fn kv_delete_range(
        &self,
        start_key: Key,
        end_key: Key,
    ) -> impl Future<Output = Result<kvrpcpb::DeleteRangeResponse>> {
        let mut req = txn_request!(self.region, kvrpcpb::DeleteRangeRequest);
        req.set_start_key(start_key.into());
        req.set_end_key(end_key.into());

        map_errors_and_trace(
            "kv_delete_range",
            self.client
                .clone()
                .kv_delete_range_async_opt(&req, self.call_options()),
        )
    }

    fn call_options(&self) -> CallOption {
        CallOption::default().timeout(self.timeout)
    }
}

fn map_errors_and_trace<Resp, RpcFuture>(
    request_name: &'static str,
    fut: ::grpcio::Result<RpcFuture>,
) -> impl Future<Output = Result<Resp>>
where
    Compat01As03<RpcFuture>: Future<Output = std::result::Result<Resp, ::grpcio::Error>>,
    Resp: HasError + Sized + Clone + Send + 'static,
{
    let context = tikv_stats(request_name);

    // FIXME should handle the error, not unwrap.
    Compat01As03::new(fut.unwrap())
        .map(|r| match r {
            Err(e) => Err(ErrorKind::Grpc(e).into()),
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
        .map(move |r| context.done(r))
}
