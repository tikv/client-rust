// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::result;
use thiserror::Error;

/// An error originating from the TiKV client or dependencies.
#[derive(Debug, Error)]
#[allow(clippy::large_enum_variant)]
pub enum Error {
    /// Feature is not implemented.
    #[error("Unimplemented feature")]
    Unimplemented,
    /// Duplicate key insertion happens.
    #[error("Duplicate key insertion")]
    DuplicateKeyInsertion,
    /// Failed to resolve a lock
    #[error("Failed to resolve lock")]
    ResolveLockError,
    /// Will raise this error when using a pessimistic txn only operation on an optimistic txn
    #[error("Invalid operation for this type of transaction")]
    InvalidTransactionType,
    /// It's not allowed to perform operations in a transaction after it has been committed or rolled back.
    #[error("Cannot read or write data after any attempt to commit or roll back the transaction")]
    OperationAfterCommitError,
    /// We tried to use 1pc for a transaction, but it didn't work. Probably should have used 2pc.
    #[error("1PC transaction could not be committed.")]
    OnePcFailure,
    /// An operation requires a primary key, but the transaction was empty.
    #[error("transaction has no primary key")]
    NoPrimaryKey,
    #[error(
        "The operation does is not supported in raw-atomic mode, please consider using an atomic client"
    )]
    UnsupportedInAtomicMode,
    #[error("The operation is only supported in raw-atomic mode, please consider using a non-atomic raw client")]
    UnsupportedInNonAtomicMode,
    /// Wraps a `std::io::Error`.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Wraps a `grpcio::Error`.
    #[error("gRPC error: {0}")]
    Grpc(#[from] grpcio::Error),
    /// Represents that a futures oneshot channel was cancelled.
    #[error("A futures oneshot channel was canceled. {0}")]
    Canceled(#[from] futures::channel::oneshot::Canceled),
    /// Errors caused by changes of region information
    #[error("Region error: {0:?}")]
    RegionError(tikv_client_proto::errorpb::Error),
    /// Whether the transaction is committed or not is undetermined
    #[error("Whether the transaction is committed or not is undetermined")]
    UndeterminedError(Box<Error>),
    /// Wraps `tikv_client_proto::kvrpcpb::KeyError`
    #[error("{0:?}")]
    KeyError(tikv_client_proto::kvrpcpb::KeyError),
    /// Multiple errors
    #[error("Multiple errors: {0:?}")]
    MultipleErrors(Vec<Error>),
    /// Invalid ColumnFamily
    #[error("Unsupported column family {}", _0)]
    ColumnFamilyError(String),
    /// No region is found for the given key.
    #[error("Region is not found for key: {:?}", key)]
    RegionForKeyNotFound { key: Vec<u8> },
    /// No region is found for the given id.
    #[error("Region {} is not found", region_id)]
    RegionNotFound { region_id: u64 },
    /// No leader is found for the given id.
    #[error("Leader of region {} is not found", region_id)]
    LeaderNotFound { region_id: u64 },
    /// Scan limit exceeds the maximum
    #[error("Limit {} exceeds max scan limit {}", limit, max_limit)]
    MaxScanLimitExceeded { limit: u32, max_limit: u32 },
    /// A string error returned by TiKV server
    #[error("Kv error. {}", message)]
    KvError { message: String },
    #[error("{}", message)]
    InternalError { message: String },
    #[error("{0}")]
    StringError(String),
}

impl From<tikv_client_proto::errorpb::Error> for Error {
    fn from(e: tikv_client_proto::errorpb::Error) -> Error {
        Error::RegionError(e)
    }
}

impl From<tikv_client_proto::kvrpcpb::KeyError> for Error {
    fn from(e: tikv_client_proto::kvrpcpb::KeyError) -> Error {
        Error::KeyError(e)
    }
}

/// A result holding an [`Error`](enum@Error).
pub type Result<T> = result::Result<T, Error>;

#[macro_export]
macro_rules! internal_err {
    ($e:expr) => ({
        $crate::Error::InternalError {
            message: format!("[{}:{}]: {}", file!(), line!(),  $e)
        }
    });
    ($f:tt, $($arg:expr),+) => ({
        internal_err!(format!($f, $($arg),+))
    });
}
