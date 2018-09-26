// Copyright 2016 PingCAP, Inc.
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

use std::error;
use std::result;

quick_error!{
    #[derive(Debug)]
    pub enum Error {
        Io(err: ::std::io::Error) {
            from()
            cause(err)
            description(err.description())
        }
        Grpc(err: ::grpc::Error) {
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
        RegionNotFound(key: Vec<u8>) {
            description("region is not found")
            display("region is not found for key {:?}", key)
        }
    }
}

pub type Result<T> = result::Result<T, Error>;
