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
    #[derive(Debug)]
    pub enum Error {
        Io(err: ::std::io::Error) {
            from()
            cause(err)
            description(err.description())
        }
        Grpc(err: grpcio::Error) {
            from()
            cause(err)
            description(err.description())
        }
        Canceled(err: ::futures::sync::oneshot::Canceled) {
            from()
            cause(err)
            description(err.description())
        }
        Other(err: Box<error::Error + Sync + Send>) {
            from()
            cause(err.as_ref())
            description(err.description())
            display("unknown error {:?}", err)
        }
        RegionForKeyNotFound(key: Vec<u8>) {
            description("region is not found")
            display("region is not found for key {:?}", key)
        }
        RegionNotFound(id: u64) {
            description("region is not found")
            display("region {:?} is not found", id)
        }
        NotLeader(region_id: u64) {
            description("peer is not leader")
            display("peer is not leader for region {:?}.", region_id)
        }
        StoreNotMatch {
            description("store not match")
            display("store not match")
        }
        KeyNotInRegion(key: Vec<u8>, region_id: u64, start_key: Vec<u8>, end_key: Vec<u8>) {
            description("region is not found")
            display("key {:?} is not in region {:?}: [{:?}, {:?})", key, region_id, start_key, end_key)
        }
        StaleEpoch {
            description("stale epoch")
            display("stale epoch")
        }
        ServerIsBusy(reason: String) {
            description("server is busy")
            display("server is busy: {:?}", reason)
        }
        RaftEntryTooLarge(region_id: u64, entry_size: u64) {
            description("raft entry too large")
            display("{:?} bytes raft entry of region {:?} is too large", entry_size, region_id)
        }
    }
}

pub type Result<T> = result::Result<T, Error>;
