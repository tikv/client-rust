// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use failure::{Backtrace, Context, Fail};
use grpcio;
use std::fmt::{self, Display};
use std::result;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

/// An error originating from the TiKV client or dependencies.
#[derive(Debug, Fail)]
#[allow(clippy::large_enum_variant)]
pub enum ErrorKind {
    /// Wraps a `std::io::Error`.
    #[fail(display = "IO error: {}", _0)]
    Io(#[fail(cause)] std::io::Error),
    /// Wraps a `grpcio::Error`.
    #[fail(display = "gRPC error: {}", _0)]
    Grpc(#[fail(cause)] grpcio::Error),
    /// Represents that a futures oneshot channel was cancelled.
    #[fail(display = "A futures oneshot channel was canceled. {}", _0)]
    Canceled(#[fail(cause)] futures::sync::oneshot::Canceled),
    /// Feature is not implemented.
    #[fail(display = "Unimplemented feature")]
    Unimplemented,
    // No region is found for the given key.
    #[fail(display = "Region is not found for key: {:?}", key)]
    RegionForKeyNotFound { key: Vec<u8> },
    /// The peer is not the leader for the region.
    #[fail(display = "Peer is not leader for region {}. {}", region_id, message)]
    NotLeader { region_id: u64, message: String },
    /// Stale epoch
    #[fail(display = "Stale epoch. {}", message)]
    StaleEpoch { message: String },
    /// No region is found for the given id.
    #[fail(display = "Region {} is not found. {}", region_id, message)]
    RegionNotFound { region_id: u64, message: String },
    /// Invalid key range to scan. Only left bounded intervals are supported.
    #[fail(display = "Only left bounded intervals are supported")]
    InvalidKeyRange,
    /// Cannot set an empty value
    #[fail(display = "Cannot set an empty value")]
    EmptyValue,
    /// Scan limit exceeds the maximum
    #[fail(display = "Limit {} exceeds max scan limit {}", limit, max_limit)]
    MaxScanLimitExceeded { limit: u32, max_limit: u32 },
    /// Wraps `kvproto::kvrpcpb::KeyError`
    #[fail(display = "{:?}", _0)]
    KeyError(kvproto::kvrpcpb::KeyError),
    /// A string error returned by TiKV server
    #[fail(display = "Kv error. {}", message)]
    KvError { message: String },
    /// Reconstructed `kvproto::errorpb::KeyNotInRegion`
    #[fail(
        display = "Key {:?} is not in region {}: [{:?}, {:?})",
        key, region_id, start_key, end_key
    )]
    KeyNotInRegion {
        key: Vec<u8>,
        region_id: u64,
        start_key: Vec<u8>,
        end_key: Vec<u8>,
    },
    /// Reconstructed `kvproto::errorpb::ServerIsBusy`
    #[fail(display = "Server is busy: {}. Backoff {} ms", reason, backoff_ms)]
    ServerIsBusy { reason: String, backoff_ms: u64 },
    /// Represents `kvproto::errorpb::StaleCommand` with additional error message
    #[fail(display = "Stale command. {}", message)]
    StaleCommand { message: String },
    /// Represents `kvproto::errorpb::StoreNotMatch` with additional error message
    #[fail(
        display = "Requesting store {} when actual store is {}. {}",
        request_store_id, actual_store_id, message
    )]
    StoreNotMatch {
        request_store_id: u64,
        actual_store_id: u64,
        message: String,
    },
    /// Represents `kvproto::errorpb::RaftEntryTooLarge` with additional error message
    #[fail(
        display = "{} bytes raft entry of region {} is too large. {}",
        entry_size, region_id, message
    )]
    RaftEntryTooLarge {
        region_id: u64,
        entry_size: u64,
        message: String,
    },
    #[fail(display = "{}", message)]
    InternalError { message: String },
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }

    pub(crate) fn unimplemented() -> Self {
        Error::from(ErrorKind::Unimplemented)
    }

    pub(crate) fn region_for_key_not_found(key: Vec<u8>) -> Self {
        Error::from(ErrorKind::RegionForKeyNotFound { key })
    }

    pub(crate) fn not_leader(region_id: u64, message: Option<String>) -> Self {
        Error::from(ErrorKind::NotLeader {
            region_id,
            message: message.unwrap_or_default(),
        })
    }

    pub(crate) fn stale_epoch(message: Option<String>) -> Self {
        Error::from(ErrorKind::StaleEpoch {
            message: message.unwrap_or_default(),
        })
    }

    pub(crate) fn region_not_found(region_id: u64, message: Option<String>) -> Self {
        Error::from(ErrorKind::RegionNotFound {
            region_id,
            message: message.unwrap_or_default(),
        })
    }

    pub(crate) fn invalid_key_range() -> Self {
        Error::from(ErrorKind::InvalidKeyRange)
    }

    pub(crate) fn empty_value() -> Self {
        Error::from(ErrorKind::EmptyValue)
    }

    pub(crate) fn max_scan_limit_exceeded(limit: u32, max_limit: u32) -> Self {
        Error::from(ErrorKind::MaxScanLimitExceeded { limit, max_limit })
    }

    pub(crate) fn kv_error(message: String) -> Self {
        Error::from(ErrorKind::KvError { message })
    }

    pub(crate) fn key_not_in_region(mut e: kvproto::errorpb::KeyNotInRegion) -> Self {
        Error::from(ErrorKind::KeyNotInRegion {
            key: e.take_key(),
            region_id: e.get_region_id(),
            start_key: e.take_start_key(),
            end_key: e.take_end_key(),
        })
    }

    pub(crate) fn server_is_busy(mut e: kvproto::errorpb::ServerIsBusy) -> Self {
        Error::from(ErrorKind::ServerIsBusy {
            reason: e.take_reason(),
            backoff_ms: e.get_backoff_ms(),
        })
    }

    pub(crate) fn stale_command(message: String) -> Self {
        Error::from(ErrorKind::StaleCommand { message })
    }

    pub(crate) fn store_not_match(e: kvproto::errorpb::StoreNotMatch, message: String) -> Self {
        Error::from(ErrorKind::StoreNotMatch {
            request_store_id: e.get_request_store_id(),
            actual_store_id: e.get_actual_store_id(),
            message,
        })
    }

    pub(crate) fn raft_entry_too_large(
        e: kvproto::errorpb::RaftEntryTooLarge,
        message: String,
    ) -> Self {
        Error::from(ErrorKind::RaftEntryTooLarge {
            region_id: e.get_region_id(),
            entry_size: e.get_entry_size(),
            message,
        })
    }

    pub(crate) fn internal_error(message: String) -> Self {
        Error::from(ErrorKind::InternalError { message })
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::from(ErrorKind::Io(err))
    }
}

impl From<grpcio::Error> for Error {
    fn from(err: grpcio::Error) -> Self {
        Error::from(ErrorKind::Grpc(err))
    }
}

impl From<futures::sync::oneshot::Canceled> for Error {
    fn from(err: futures::sync::oneshot::Canceled) -> Self {
        Error::from(ErrorKind::Canceled(err))
    }
}

impl From<kvproto::kvrpcpb::KeyError> for Error {
    fn from(err: kvproto::kvrpcpb::KeyError) -> Self {
        Error::from(ErrorKind::KeyError(err))
    }
}

/// A result holding an [`Error`](enum.Error.html).
pub type Result<T> = result::Result<T, Error>;
