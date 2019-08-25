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
    fn from(e: errorpb::Error) -> Error {
        Error::region_error(e)
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
