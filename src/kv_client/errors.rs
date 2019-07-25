// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::Error;
use kvproto::{errorpb, kvrpcpb};

pub trait HasRegionError {
    fn region_error(&mut self) -> Option<Error>;
}

pub trait HasError: HasRegionError {
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
