// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::ops::{Deref, DerefMut};

use lazy_static::*;
use prometheus::*;

use crate::rpc::context::RequestContext;

pub struct PdRequestContext<Executor> {
    target: RequestContext<Executor>,
}

impl<Executor> Deref for PdRequestContext<Executor> {
    type Target = RequestContext<Executor>;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

impl<Executor> DerefMut for PdRequestContext<Executor> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.target
    }
}

pub fn request_context<Executor>(
    cmd: &'static str,
    executor: Executor,
) -> PdRequestContext<Executor> {
    PdRequestContext {
        target: RequestContext::new(
            cmd,
            &PD_REQUEST_DURATION_HISTOGRAM_VEC,
            &PD_REQUEST_COUNTER_VEC,
            &PD_FAILED_REQUEST_DURATION_HISTOGRAM_VEC,
            &PD_FAILED_REQUEST_COUNTER_VEC,
            executor,
        ),
    }
}

pub fn observe_tso_batch(batch_size: usize) -> u32 {
    PD_TSO_BATCH_SIZE_HISTOGRAM.observe(batch_size as f64);
    batch_size as u32
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
