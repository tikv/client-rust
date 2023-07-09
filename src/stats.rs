// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::Duration;
use std::time::Instant;

use prometheus::register_histogram;
use prometheus::register_histogram_vec;
use prometheus::register_int_counter_vec;
use prometheus::Histogram;
use prometheus::HistogramVec;
use prometheus::IntCounterVec;

use crate::Result;

pub struct RequestStats {
    start: Instant,
    cmd: &'static str,
    duration: &'static HistogramVec,
    failed_duration: &'static HistogramVec,
    failed_counter: &'static IntCounterVec,
}

impl RequestStats {
    pub fn new(
        cmd: &'static str,
        duration: &'static HistogramVec,
        counter: &'static IntCounterVec,
        failed_duration: &'static HistogramVec,
        failed_counter: &'static IntCounterVec,
    ) -> Self {
        counter.with_label_values(&[cmd]).inc();
        RequestStats {
            start: Instant::now(),
            cmd,
            duration,
            failed_duration,
            failed_counter,
        }
    }

    pub fn done<R>(&self, r: Result<R>) -> Result<R> {
        if r.is_ok() {
            self.duration
                .with_label_values(&[self.cmd])
                .observe(duration_to_sec(self.start.elapsed()));
        } else {
            self.failed_duration
                .with_label_values(&[self.cmd])
                .observe(duration_to_sec(self.start.elapsed()));
            self.failed_counter.with_label_values(&[self.cmd]).inc();
        }
        r
    }
}

pub fn tikv_stats(cmd: &'static str) -> RequestStats {
    RequestStats::new(
        cmd,
        &TIKV_REQUEST_DURATION_HISTOGRAM_VEC,
        &TIKV_REQUEST_COUNTER_VEC,
        &TIKV_FAILED_REQUEST_DURATION_HISTOGRAM_VEC,
        &TIKV_FAILED_REQUEST_COUNTER_VEC,
    )
}

pub fn pd_stats(cmd: &'static str) -> RequestStats {
    RequestStats::new(
        cmd,
        &PD_REQUEST_DURATION_HISTOGRAM_VEC,
        &PD_REQUEST_COUNTER_VEC,
        &PD_FAILED_REQUEST_DURATION_HISTOGRAM_VEC,
        &PD_FAILED_REQUEST_COUNTER_VEC,
    )
}

#[allow(dead_code)]
pub fn observe_tso_batch(batch_size: usize) {
    PD_TSO_BATCH_SIZE_HISTOGRAM.observe(batch_size as f64);
}

lazy_static::lazy_static! {
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

/// Convert Duration to seconds.
#[inline]
fn duration_to_sec(d: Duration) -> f64 {
    let nanos = f64::from(d.subsec_nanos());
    // In most cases, we can't have so large Duration, so here just panic if overflow now.
    d.as_secs() as f64 + (nanos / 1_000_000_000.0)
}
