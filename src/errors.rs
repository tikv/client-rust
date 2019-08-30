// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use failure::{Backtrace, Context, Fail};
use grpcio;
use std::fmt::{self, Display};
use std::result;

#[derive(Debug)]
pub struct Error {
    inner: Box<Context<ErrorKind>>,
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
    Canceled(#[fail(cause)] futures::channel::oneshot::Canceled),
    /// Feature is not implemented.
    #[fail(display = "Unimplemented feature")]
    Unimplemented,
    /// No region is found for the given key.
    #[fail(display = "Region is not found for key: {:?}", key)]
    RegionForKeyNotFound { key: Vec<u8> },
    /// Errors caused by changed region information
    #[fail(display = "Region error: {:?}", _0)]
    RegionError(kvproto::errorpb::Error),
    /// No region is found for the given id.
    #[fail(display = "Region {} is not found", region_id)]
    RegionNotFound { region_id: u64 },
    /// No region is found for the given id.
    #[fail(display = "Leader of region {} is not found", region_id)]
    LeaderNotFound { region_id: u64 },
    /// Whether the transaction is committed or not is undetermined
    #[fail(display = "Whether the transaction is committed or not is undetermined")]
    UndeterminedError(#[fail(cause)] Error),
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
    #[fail(display = "{}", message)]
    InternalError { message: String },
    /// Multiple errors
    #[fail(display = "Multiple errors: {:?}", _0)]
    MultipleErrors(Vec<Error>),
}

impl Fail for Error {
    fn cause(&self) -> Option<&dyn Fail> {
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

    pub(crate) fn region_error(error: kvproto::errorpb::Error) -> Self {
        Error::from(ErrorKind::RegionError(error))
    }

    pub(crate) fn region_not_found(region_id: u64) -> Self {
        Error::from(ErrorKind::RegionNotFound { region_id })
    }

    pub(crate) fn leader_not_found(region_id: u64) -> Self {
        Error::from(ErrorKind::LeaderNotFound { region_id })
    }

    pub(crate) fn invalid_key_range() -> Self {
        Error::from(ErrorKind::InvalidKeyRange)
    }

    pub(crate) fn max_scan_limit_exceeded(limit: u32, max_limit: u32) -> Self {
        Error::from(ErrorKind::MaxScanLimitExceeded { limit, max_limit })
    }

    pub(crate) fn kv_error(message: String) -> Self {
        Error::from(ErrorKind::KvError { message })
    }

    pub(crate) fn internal_error(message: impl Into<String>) -> Self {
        Error::from(ErrorKind::InternalError {
            message: message.into(),
        })
    }

    pub(crate) fn multiple_errors(errors: Vec<Error>) -> Self {
        Error::from(ErrorKind::MultipleErrors(errors))
    }

    pub(crate) fn undetermined_error(error: Error) -> Self {
        Error::from(ErrorKind::UndeterminedError(error))
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Box::new(Context::new(kind)),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error {
            inner: Box::new(inner),
        }
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

impl From<futures::channel::oneshot::Canceled> for Error {
    fn from(err: futures::channel::oneshot::Canceled) -> Self {
        Error::from(ErrorKind::Canceled(err))
    }
}

impl From<kvproto::kvrpcpb::KeyError> for Error {
    fn from(err: kvproto::kvrpcpb::KeyError) -> Self {
        Error::from(ErrorKind::KeyError(err))
    }
}

/// A result holding an [`Error`](Error).
pub type Result<T> = result::Result<T, Error>;
