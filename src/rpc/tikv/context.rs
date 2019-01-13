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

use lazy_static::*;
use prometheus::*;

pub use crate::rpc::context::RequestContext;

pub fn request_context<Executor>(
    cmd: &'static str,
    executor: Executor,
) -> RequestContext<Executor> {
    RequestContext::new(
        cmd,
        &TIKV_REQUEST_DURATION_HISTOGRAM_VEC,
        &TIKV_REQUEST_COUNTER_VEC,
        &TIKV_FAILED_REQUEST_DURATION_HISTOGRAM_VEC,
        &TIKV_FAILED_REQUEST_COUNTER_VEC,
        executor,
    )
}

lazy_static! {
    static ref TIKV_REQUEST_DURATION_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "tikv_request_duration_seconds",
        "Bucketed histogram of TiKV requests duration",
        &["type"]
    )
    .unwrap();
    static ref TIKV_REQUEST_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "tikv_request_total",
        "Total number of requests sent to TiKV",
        &["type"]
    )
    .unwrap();
    static ref TIKV_FAILED_REQUEST_DURATION_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "tikv_failed_request_duration_seconds",
        "Bucketed histogram of failed TiKV requests duration",
        &["type"]
    )
    .unwrap();
    static ref TIKV_FAILED_REQUEST_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "tikv_failed_request_total",
        "Total number of failed requests sent to TiKV",
        &["type"]
    )
    .unwrap();
}
