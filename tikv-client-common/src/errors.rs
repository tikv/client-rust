// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use failure::{Backtrace, Context, Fail};
use std::{
    fmt::{self, Display},
    result,
};
use tikv_client_proto::errorpb;

/// The error type used in tikv-client.
#[derive(Debug)]
pub struct Error {
    inner: Box<Context<ErrorKind>>,
}

/// An error originating from the TiKV client or dependencies.
#[derive(Debug, Fail)]
#[allow(clippy::large_enum_variant)]
pub enum ErrorKind {
    /// Feature is not implemented.
    #[fail(display = "Unimplemented feature")]
    Unimplemented,
    /// Failed to resolve a lock
    #[fail(display = "Failed to resolve lock")]
    ResolveLockError,
    /// Will raise this error when using a pessimistic txn only operation on an optimistic txn
    #[fail(display = "Invalid operation for this type of transaction")]
    InvalidTransactionType,
    /// Invalid key range to scan. Only left bounded intervals are supported.
    #[fail(display = "Only left bounded intervals are supported")]
    InvalidKeyRange,
    /// Wraps a `std::io::Error`.
    #[fail(display = "IO error: {}", _0)]
    Io(#[fail(cause)] std::io::Error),
    /// Wraps a `grpcio::Error`.
    #[fail(display = "gRPC error: {}", _0)]
    Grpc(#[fail(cause)] grpcio::Error),
    /// Represents that a futures oneshot channel was cancelled.
    #[fail(display = "A futures oneshot channel was canceled. {}", _0)]
    Canceled(#[fail(cause)] futures::channel::oneshot::Canceled),
    /// Errors caused by changes of region information
    #[fail(display = "Region error: {:?}", _0)]
    RegionError(tikv_client_proto::errorpb::Error),
    /// Whether the transaction is committed or not is undetermined
    #[fail(display = "Whether the transaction is committed or not is undetermined")]
    UndeterminedError(#[fail(cause)] Error),
    /// Wraps `tikv_client_proto::kvrpcpb::KeyError`
    #[fail(display = "{:?}", _0)]
    KeyError(tikv_client_proto::kvrpcpb::KeyError),
    /// Multiple errors
    #[fail(display = "Multiple errors: {:?}", _0)]
    MultipleErrors(Vec<Error>),
    /// Invalid ColumnFamily
    #[fail(display = "Unsupported column family {}", _0)]
    ColumnFamilyError(String),
    /// No region is found for the given key.
    #[fail(display = "Region is not found for key: {:?}", key)]
    RegionForKeyNotFound { key: Vec<u8> },
    /// No region is found for the given id.
    #[fail(display = "Region {} is not found", region_id)]
    RegionNotFound { region_id: u64 },
    /// No leader is found for the given id.
    #[fail(display = "Leader of region {} is not found", region_id)]
    LeaderNotFound { region_id: u64 },
    /// Scan limit exceeds the maximum
    #[fail(display = "Limit {} exceeds max scan limit {}", limit, max_limit)]
    MaxScanLimitExceeded { limit: u32, max_limit: u32 },
    /// A string error returned by TiKV server
    #[fail(display = "Kv error. {}", message)]
    KvError { message: String },
    #[fail(display = "{}", message)]
    InternalError { message: String },
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

    #[allow(dead_code)]
    pub fn unimplemented() -> Self {
        Error::from(ErrorKind::Unimplemented)
    }

    pub fn region_for_key_not_found(key: Vec<u8>) -> Self {
        Error::from(ErrorKind::RegionForKeyNotFound { key })
    }

    pub fn region_error(error: tikv_client_proto::errorpb::Error) -> Self {
        Error::from(ErrorKind::RegionError(error))
    }

    pub fn region_not_found(region_id: u64) -> Self {
        Error::from(ErrorKind::RegionNotFound { region_id })
    }

    pub fn leader_not_found(region_id: u64) -> Self {
        Error::from(ErrorKind::LeaderNotFound { region_id })
    }

    pub fn invalid_key_range() -> Self {
        Error::from(ErrorKind::InvalidKeyRange)
    }

    pub fn max_scan_limit_exceeded(limit: u32, max_limit: u32) -> Self {
        Error::from(ErrorKind::MaxScanLimitExceeded { limit, max_limit })
    }

    pub fn kv_error(message: String) -> Self {
        Error::from(ErrorKind::KvError { message })
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Error::from(ErrorKind::InternalError {
            message: message.into(),
        })
    }

    pub fn multiple_errors(errors: Vec<Error>) -> Self {
        Error::from(ErrorKind::MultipleErrors(errors))
    }

    pub fn undetermined_error(error: Error) -> Self {
        Error::from(ErrorKind::UndeterminedError(error))
    }
}

impl From<errorpb::Error> for Error {
    fn from(e: errorpb::Error) -> Error {
        Error::region_error(e)
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

impl From<tikv_client_proto::kvrpcpb::KeyError> for Error {
    fn from(err: tikv_client_proto::kvrpcpb::KeyError) -> Self {
        Error::from(ErrorKind::KeyError(err))
    }
}

/// The result type used in tikv-client. 
/// It holds an error type that contains varisous kinds of error that can happen in the crate.
pub type Result<T> = result::Result<T, Error>;

#[macro_export]
macro_rules! internal_err {
    ($e:expr) => ({
        let kind = $crate::Error::internal_error(
            format!("[{}:{}]: {}", file!(), line!(),  $e)
        );
        $crate::Error::from(kind)
    });
    ($f:tt, $($arg:expr),+) => ({
        internal_err!(format!($f, $($arg),+))
    });
}
