// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::Error;
use std::fmt::Display;
use tikv_client_proto::kvrpcpb;

// Those that can have a single region error
pub trait HasRegionError {
    fn region_error(&mut self) -> Option<tikv_client_proto::errorpb::Error>;
}

// Those that can have multiple region errors
pub trait HasRegionErrors {
    fn region_errors(&mut self) -> Option<Vec<tikv_client_proto::errorpb::Error>>;
}

pub trait HasKeyErrors {
    fn key_errors(&mut self) -> Option<Vec<Error>>;
}

impl<T: HasRegionError> HasRegionErrors for T {
    fn region_errors(&mut self) -> Option<Vec<tikv_client_proto::errorpb::Error>> {
        self.region_error().map(|e| vec![e])
    }
}

macro_rules! has_region_error {
    ($type:ty) => {
        impl HasRegionError for $type {
            fn region_error(&mut self) -> Option<tikv_client_proto::errorpb::Error> {
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
has_region_error!(kvrpcpb::TxnHeartBeatResponse);
has_region_error!(kvrpcpb::CheckTxnStatusResponse);
has_region_error!(kvrpcpb::CheckSecondaryLocksResponse);
has_region_error!(kvrpcpb::DeleteRangeResponse);
has_region_error!(kvrpcpb::GcResponse);
has_region_error!(kvrpcpb::RawGetResponse);
has_region_error!(kvrpcpb::RawBatchGetResponse);
has_region_error!(kvrpcpb::RawPutResponse);
has_region_error!(kvrpcpb::RawBatchPutResponse);
has_region_error!(kvrpcpb::RawDeleteResponse);
has_region_error!(kvrpcpb::RawBatchDeleteResponse);
has_region_error!(kvrpcpb::RawDeleteRangeResponse);
has_region_error!(kvrpcpb::RawScanResponse);
has_region_error!(kvrpcpb::RawBatchScanResponse);
has_region_error!(kvrpcpb::RawCasResponse);

macro_rules! has_key_error {
    ($type:ty) => {
        impl HasKeyErrors for $type {
            fn key_errors(&mut self) -> Option<Vec<Error>> {
                if self.has_error() {
                    Some(vec![self.take_error().into()])
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
has_key_error!(kvrpcpb::TxnHeartBeatResponse);
has_key_error!(kvrpcpb::CheckTxnStatusResponse);
has_key_error!(kvrpcpb::CheckSecondaryLocksResponse);

macro_rules! has_str_error {
    ($type:ty) => {
        impl HasKeyErrors for $type {
            fn key_errors(&mut self) -> Option<Vec<Error>> {
                if self.get_error().is_empty() {
                    None
                } else {
                    Some(vec![Error::KvError {
                        message: self.take_error(),
                    }])
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
has_str_error!(kvrpcpb::RawCasResponse);
has_str_error!(kvrpcpb::ImportResponse);
has_str_error!(kvrpcpb::DeleteRangeResponse);

impl HasKeyErrors for kvrpcpb::ScanResponse {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        extract_errors(self.pairs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasKeyErrors for kvrpcpb::BatchGetResponse {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        extract_errors(self.pairs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasKeyErrors for kvrpcpb::RawBatchGetResponse {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        extract_errors(self.pairs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasKeyErrors for kvrpcpb::RawScanResponse {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        extract_errors(self.kvs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasKeyErrors for kvrpcpb::RawBatchScanResponse {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        extract_errors(self.kvs.iter_mut().map(|pair| pair.error.take()))
    }
}

impl HasKeyErrors for kvrpcpb::PrewriteResponse {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        extract_errors(self.take_errors().into_iter().map(Some))
    }
}

impl HasKeyErrors for kvrpcpb::PessimisticLockResponse {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        extract_errors(self.take_errors().into_iter().map(Some))
    }
}

impl HasKeyErrors for kvrpcpb::PessimisticRollbackResponse {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        extract_errors(self.take_errors().into_iter().map(Some))
    }
}

impl<T: HasKeyErrors, E: Display> HasKeyErrors for Result<T, E> {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        match self {
            Ok(x) => x.key_errors(),
            Err(e) => Some(vec![Error::StringError(e.to_string())]),
        }
    }
}

impl<T: HasKeyErrors> HasKeyErrors for Vec<T> {
    fn key_errors(&mut self) -> Option<Vec<Error>> {
        for t in self {
            if let Some(e) = t.key_errors() {
                return Some(e);
            }
        }

        None
    }
}

impl<T: HasRegionError, E> HasRegionError for Result<T, E> {
    fn region_error(&mut self) -> Option<tikv_client_proto::errorpb::Error> {
        self.as_mut().ok().and_then(|t| t.region_error())
    }
}

impl<T: HasRegionError> HasRegionErrors for Vec<T> {
    fn region_errors(&mut self) -> Option<Vec<tikv_client_proto::errorpb::Error>> {
        let errors: Vec<_> = self.iter_mut().filter_map(|x| x.region_error()).collect();
        if errors.is_empty() {
            None
        } else {
            Some(errors)
        }
    }
}

fn extract_errors(
    error_iter: impl Iterator<Item = Option<kvrpcpb::KeyError>>,
) -> Option<Vec<Error>> {
    let errors: Vec<Error> = error_iter.flatten().map(Into::into).collect();
    if errors.is_empty() {
        None
    } else {
        Some(errors)
    }
}

#[cfg(test)]
mod test {
    use super::HasKeyErrors;
    use tikv_client_common::{internal_err, Error};
    use tikv_client_proto::kvrpcpb;
    #[test]
    fn result_haslocks() {
        let mut resp: Result<_, Error> = Ok(kvrpcpb::CommitResponse {
            region_error: None,
            error: None,
            commit_version: 0,
        });
        assert!(resp.key_errors().is_none());

        let mut resp: Result<_, Error> = Ok(kvrpcpb::CommitResponse {
            region_error: None,
            error: Some(kvrpcpb::KeyError {
                locked: None,
                retryable: String::new(),
                abort: String::new(),
                conflict: None,
                already_exist: None,
                deadlock: None,
                commit_ts_expired: None,
                txn_not_found: None,
                commit_ts_too_large: None,
            }),
            commit_version: 0,
        });
        assert!(resp.key_errors().is_some());

        let mut resp: Result<kvrpcpb::CommitResponse, _> = Err(internal_err!("some error"));
        assert!(resp.key_errors().is_some());
    }
}
