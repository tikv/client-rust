// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::Error;
use tikv_client_proto::kvrpcpb;

pub trait HasRegionError {
    fn region_error(&mut self) -> Option<Error>;
}

pub trait HasError: HasRegionError {
    fn error(&mut self) -> Option<Error>;
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
has_region_error!(kvrpcpb::PessimisticLockResponse);
has_region_error!(kvrpcpb::ImportResponse);
has_region_error!(kvrpcpb::BatchRollbackResponse);
has_region_error!(kvrpcpb::PessimisticRollbackResponse);
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
                    Some(Error::KvError {
                        message: self.take_error(),
                    })
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

impl HasError for kvrpcpb::ScanResponse {
    fn error(&mut self) -> Option<Error> {
        extract_errors(self.pairs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasError for kvrpcpb::BatchGetResponse {
    fn error(&mut self) -> Option<Error> {
        extract_errors(self.pairs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasError for kvrpcpb::RawBatchGetResponse {
    fn error(&mut self) -> Option<Error> {
        extract_errors(self.pairs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasError for kvrpcpb::RawScanResponse {
    fn error(&mut self) -> Option<Error> {
        extract_errors(self.kvs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasError for kvrpcpb::RawBatchScanResponse {
    fn error(&mut self) -> Option<Error> {
        extract_errors(self.kvs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasError for kvrpcpb::PrewriteResponse {
    fn error(&mut self) -> Option<Error> {
        extract_errors(self.take_errors().into_iter().map(Some))
    }
}

impl HasError for kvrpcpb::PessimisticLockResponse {
    fn error(&mut self) -> Option<Error> {
        extract_errors(self.take_errors().into_iter().map(Some))
    }
}

impl HasError for kvrpcpb::PessimisticRollbackResponse {
    fn error(&mut self) -> Option<Error> {
        extract_errors(self.take_errors().into_iter().map(Some))
    }
}

fn extract_errors(error_iter: impl Iterator<Item = Option<kvrpcpb::KeyError>>) -> Option<Error> {
    let errors: Vec<Error> = error_iter.flatten().map(Into::into).collect();
    if errors.is_empty() {
        None
    } else if errors.len() == 1 {
        Some(errors.into_iter().next().unwrap())
    } else {
        Some(Error::MultipleErrors(errors))
    }
}
