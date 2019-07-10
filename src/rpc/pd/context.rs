// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use prometheus::{Histogram, HistogramVec, IntCounterVec};

use crate::rpc::context::RequestContext;

pub fn request_context(cmd: &'static str) -> RequestContext {
    RequestContext::new(
        cmd,
        &PD_REQUEST_DURATION_HISTOGRAM_VEC,
        &PD_REQUEST_COUNTER_VEC,
        &PD_FAILED_REQUEST_DURATION_HISTOGRAM_VEC,
        &PD_FAILED_REQUEST_COUNTER_VEC,
    )
}

pub fn observe_tso_batch(batch_size: usize) {
    PD_TSO_BATCH_SIZE_HISTOGRAM.observe(batch_size as f64);
}

lazy_static! {
    static ref PD_REQUEST_DURATION_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "pd_request_duration_seconds",
        "Bucketed histogram of PD requests duration",
        &["type"]
    )
    .unwrap();
    static ref PD_REQUEST_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "pd_request_total",
        "Total number of requests sent to PD",
        &["type"]
    )
    .unwrap();
    static ref PD_FAILED_REQUEST_DURATION_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "pd_failed_request_duration_seconds",
        "Bucketed histogram of failed PD requests duration",
        &["type"]
    )
    .unwrap();
    static ref PD_FAILED_REQUEST_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "pd_failed_request_total",
        "Total number of failed requests sent to PD",
        &["type"]
    )
    .unwrap();
    static ref PD_TSO_BATCH_SIZE_HISTOGRAM: Histogram = register_histogram!(
        "pd_tso_batch_size",
        "Bucketed histogram of TSO request batch size"
    )
    .unwrap();
}
