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

use std::{error, result};

use grpcio;
use quick_error::quick_error;

quick_error! {
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
        RegionNotFound(region_id: u64, message: Option<String>) {
            description("region is not found")
            display("region {:?} is not found. {}", region_id, message.as_ref().unwrap_or(&"".to_owned()))
        }
        NotLeader(region_id: u64, message: Option<String>) {
            description("peer is not leader")
            display("peer is not leader for region {}. {}", region_id, message.as_ref().unwrap_or(&"".to_owned()))
        }
        StoreNotMatch(request_store_id: u64, actual_store_id: u64, message: String) {
            description("store not match")
            display("requesting store '{}' when actual store is '{}'. {}", request_store_id, actual_store_id, message)
        }
        KeyNotInRegion(key: Vec<u8>, region_id: u64, start_key: Vec<u8>, end_key: Vec<u8>) {
            description("region is not found")
            display("key {:?} is not in region {:?}: [{:?}, {:?})", key, region_id, start_key, end_key)
        }
        StaleEpoch(message: Option<String>) {
            description("stale epoch")
            display("{}", message.as_ref().unwrap_or(&"".to_owned()))
        }
        StaleCommand(message: String) {
            description("stale command")
            display("{}", message)
        }
        ServerIsBusy(reason: String, backoff: u64) {
            description("server is busy")
            display("server is busy: {:?}. Backoff {} ms", reason, backoff)
        }
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

pub type Result<T> = result::Result<T, Error>;
