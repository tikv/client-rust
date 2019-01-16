// Copyright 2018 The TiKV Project Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.
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
    /// No such key
    #[fail(display = "Key does not exist")]
    NoSuchKey,
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
    /// Reconstructed `kvproto::errorpb::StaleCommand`
    #[fail(display = "Stale command. {}", message)]
    StaleCommand { message: String },
    /// Reconstructed `kvproto::errorpb::StoreNotMatch`
    #[fail(
        display = "Requesting store {} when actual store is {}. {}",
        request_store_id, actual_store_id, message
    )]
    StoreNotMatch {
        request_store_id: u64,
        actual_store_id: u64,
        message: String,
    },
    /// Reconstructed `kvproto::errorpb::RaftEntryTooLarge`
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

/// A result holding an [`Error`](enum.Error.html).
pub type Result<T> = result::Result<T, Error>;
