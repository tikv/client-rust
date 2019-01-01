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

use grpcio;
use quick_error::quick_error;
use std::{error, result};

quick_error! {
    /// An error originating from the TiKV client or dependencies.
    ///
    /// This client currently uses [`quick_error`](https://docs.rs/quick-error/1.2.2/quick_error/)
    /// for errors. *This may change in future versions.*
    #[derive(Debug)]
    pub enum Error {
        /// Wraps a `std::io::Error`.
        Io(err: ::std::io::Error) {
            from()
            cause(err)
            description(err.description())
        }
        /// Wraps a `grpcio::Error`.
        Grpc(err: grpcio::Error) {
            from()
            cause(err)
            description(err.description())
        }
        /// Represents that a futures oneshot channel was cancelled.
        Canceled(err: ::futures::sync::oneshot::Canceled) {
            from()
            cause(err)
            description(err.description())
        }
        /// An unknown error.
        ///
        /// Generally, this is not an expected error. Please report it if encountered.
        Other(err: Box<error::Error + Sync + Send>) {
            from()
            cause(err.as_ref())
            description(err.description())
            display("unknown error {:?}", err)
        }
        /// A region was not found for the given key.
        RegionForKeyNotFound(key: Vec<u8>) {
            description("region is not found")
            display("region is not found for key {:?}", key)
        }
        /// A region was not found.
        RegionNotFound(region_id: u64, message: Option<String>) {
            description("region is not found")
            display("region {:?} is not found. {}", region_id, message.as_ref().unwrap_or(&"".to_owned()))
        }
        /// The peer is not a leader of the given region.
        NotLeader(region_id: u64, message: Option<String>) {
            description("peer is not leader")
            display("peer is not leader for region {}. {}", region_id, message.as_ref().unwrap_or(&"".to_owned()))
        }
        /// The store does not match.
        StoreNotMatch(request_store_id: u64, actual_store_id: u64, message: String) {
            description("store not match")
            display("requesting store '{}' when actual store is '{}'. {}", request_store_id, actual_store_id, message)
        }
        /// The given key is not within the given region.
        KeyNotInRegion(key: Vec<u8>, region_id: u64, start_key: Vec<u8>, end_key: Vec<u8>) {
            description("region is not found")
            display("key {:?} is not in region {:?}: [{:?}, {:?})", key, region_id, start_key, end_key)
        }
        /// A stale epoch.
        StaleEpoch(message: Option<String>) {
            description("stale epoch")
            display("{}", message.as_ref().unwrap_or(&"".to_owned()))
        }
        StaleCommand(message: String) {
            description("stale command")
            display("{}", message)
        }
        /// The server is too busy.
        ServerIsBusy(reason: String, backoff: u64) {
            description("server is busy")
            display("server is busy: {:?}. Backoff {} ms", reason, backoff)
        }
        /// The given raft entry is too large for the region.
        RaftEntryTooLarge(region_id: u64, entry_size: u64, message: String) {
            description("raft entry too large")
            display("{:?} bytes raft entry of region {:?} is too large. {}", entry_size, region_id, message)
        }
        KeyError(message: String) {
            description("key error")
            display("{}", message)
        }
        KVError(message: String) {
            description("kv error")
            display("{}", message)
        }
        InternalError(message: String) {
           description("internal error")
           display("{}", message)
        }
        InvalidKeyRange {
            description("invalid key range")
            display("Only left closed intervals are supported")
        }
        Unimplemented {
            description("unimplemented feature")
            display("Unimplemented feature")
        }
        EmptyValue {
            description("can not set empty value")
            display("Can not set empty value")
        }
        NoSuchKey {
            description("key does not exist")
            display("Key doest not exist")
        }
        InvalidOverlappingRanges {
            description("ranges can not be overlapping")
            display("Ranges can not be overlapping")
        }
    }
}

/// A result holding an [`Error`](enum.Error.html).
pub type Result<T> = result::Result<T, Error>;
