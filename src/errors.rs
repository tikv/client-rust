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

quick_error!{
    /// An error originating from the TiKV client or dependencies.
    ///
    /// This client currently uses [`quick_error`](https://docs.rs/quick-error/1.2.2/quick_error/)
    /// for errors. *This may change in future versions.*
    #[derive(Debug)]
    pub enum Error {
        /// Wraps a a `std::io::Error`.
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
        RegionNotFound(id: u64) {
            description("region is not found")
            display("region {:?} is not found", id)
        }
        /// The peer is not a leader of the given region.
        NotLeader(region_id: u64) {
            description("peer is not leader")
            display("peer is not leader for region {:?}.", region_id)
        }
        /// The store does not match.
        StoreNotMatch {
            description("store not match")
            display("store not match")
        }
        /// The given key is not eithin the given region.
        KeyNotInRegion(key: Vec<u8>, region_id: u64, start_key: Vec<u8>, end_key: Vec<u8>) {
            description("region is not found")
            display("key {:?} is not in region {:?}: [{:?}, {:?})", key, region_id, start_key, end_key)
        }
        /// A stale epoch.
        StaleEpoch {
            description("stale epoch")
            display("stale epoch")
        }
        /// The server is too busy.
        ServerIsBusy(reason: String) {
            description("server is busy")
            display("server is busy: {:?}", reason)
        }
        /// The given raft entry is too large for the region.
        RaftEntryTooLarge(region_id: u64, entry_size: u64) {
            description("raft entry too large")
            display("{:?} bytes raft entry of region {:?} is too large", entry_size, region_id)
        }
    }
}

/// A result holding an [`Error`](enum.Error.html).
pub type Result<T> = result::Result<T, Error>;
