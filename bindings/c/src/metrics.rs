// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

use super::*;

struct MetricsSnapshot {
    samples: Vec<tikv_metric_sample_t>,
    // The samples point into these CString buffers, which remain valid until
    // the snapshot is destroyed even if this Vec reallocates.
    _strings: Vec<CString>,
}

fn ffi_cstring(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).expect("sanitized string")
}

fn metric_type_label(metric: &prometheus::proto::Metric) -> Option<&str> {
    match metric.get_label() {
        [label] if label.get_name() == "type" => Some(label.get_value()),
        _ => None,
    }
}

impl MetricsSnapshot {
    fn push(
        &mut self,
        name: &str,
        kind: tikv_metric_sample_kind_t,
        type_label: &str,
        value: f64,
        le: f64,
    ) {
        let name = ffi_cstring(name);
        let type_label = ffi_cstring(type_label);
        self.samples.push(tikv_metric_sample_t {
            name: name.as_ptr(),
            type_label: type_label.as_ptr(),
            kind,
            value,
            le,
        });
        self._strings.push(name);
        self._strings.push(type_label);
    }
}

fn collect_metrics_snapshot(registry: &prometheus::Registry) -> MetricsSnapshot {
    let mut snapshot = MetricsSnapshot {
        samples: Vec::new(),
        _strings: Vec::new(),
    };

    for family in registry.gather() {
        let name = family.get_name();
        // These request families have a single `type` label in client-rust.
        // Restrict both names and types so the C representation is lossless.
        if !matches!(
            (name, family.get_field_type()),
            (
                "tikv_request_total"
                    | "tikv_failed_request_total"
                    | "pd_request_total"
                    | "pd_failed_request_total",
                MetricType::COUNTER
            ) | (
                "tikv_request_duration_seconds"
                    | "tikv_failed_request_duration_seconds"
                    | "pd_request_duration_seconds"
                    | "pd_failed_request_duration_seconds",
                MetricType::HISTOGRAM
            )
        ) {
            continue;
        }

        match family.get_field_type() {
            MetricType::COUNTER => {
                for metric in family.get_metric() {
                    let Some(type_label) = metric_type_label(metric) else {
                        continue;
                    };
                    snapshot.push(
                        name,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_COUNTER,
                        type_label,
                        metric.get_counter().get_value(),
                        0.0,
                    );
                }
            }
            MetricType::HISTOGRAM => {
                for metric in family.get_metric() {
                    let Some(type_label) = metric_type_label(metric) else {
                        continue;
                    };
                    let histogram = metric.get_histogram();
                    let mut inf_seen = false;
                    for bucket in histogram.get_bucket() {
                        let upper_bound = bucket.get_upper_bound();
                        inf_seen |= upper_bound.is_sign_positive() && upper_bound.is_infinite();
                        snapshot.push(
                            name,
                            tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET,
                            type_label,
                            bucket.get_cumulative_count() as f64,
                            upper_bound,
                        );
                    }
                    if !inf_seen {
                        snapshot.push(
                            name,
                            tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET,
                            type_label,
                            histogram.get_sample_count() as f64,
                            f64::INFINITY,
                        );
                    }
                    snapshot.push(
                        name,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_SUM,
                        type_label,
                        histogram.get_sample_sum(),
                        0.0,
                    );
                    snapshot.push(
                        name,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT,
                        type_label,
                        histogram.get_sample_count() as f64,
                        0.0,
                    );
                }
            }
            _ => {}
        }
    }

    snapshot
}

#[no_mangle]
pub extern "C" fn tikv_metrics_snapshot_collect(
    out_snapshot: *mut *mut tikv_metrics_snapshot_t,
    out_samples: *mut tikv_metric_samples_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_snapshot.is_null() {
        unsafe { *out_snapshot = ptr::null_mut() };
    }
    if !out_samples.is_null() {
        unsafe {
            *out_samples = tikv_metric_samples_t {
                ptr: ptr::null(),
                len: 0,
            }
        };
    }
    if out_snapshot.is_null() || out_samples.is_null() {
        clear_error_slot(out_error);
        let name = if out_snapshot.is_null() {
            "out_snapshot"
        } else {
            "out_samples"
        };
        set_error_slot(out_error, format!("{name} must not be null"));
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    match run_ffi(out_error, || {
        let snapshot = Box::new(collect_metrics_snapshot(prometheus::default_registry()));
        unsafe {
            *out_samples = tikv_metric_samples_t {
                ptr: if snapshot.samples.is_empty() {
                    ptr::null()
                } else {
                    snapshot.samples.as_ptr()
                },
                len: snapshot.samples.len(),
            };
            *out_snapshot = Box::into_raw(snapshot) as *mut tikv_metrics_snapshot_t;
        }
        Ok(())
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_metrics_snapshot_destroy(snapshot: *mut tikv_metrics_snapshot_t) {
    if snapshot.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(snapshot as *mut MetricsSnapshot));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_exports_only_client_request_families() {
        let registry = prometheus::Registry::new();
        let counter_names = [
            "tikv_request_total",
            "tikv_failed_request_total",
            "pd_request_total",
            "pd_failed_request_total",
        ];
        let histogram_names = [
            "tikv_request_duration_seconds",
            "tikv_failed_request_duration_seconds",
            "pd_request_duration_seconds",
            "pd_failed_request_duration_seconds",
        ];
        for name in counter_names {
            let counter =
                prometheus::IntCounterVec::new(prometheus::Opts::new(name, name), &["type"])
                    .unwrap();
            counter.with_label_values(&["raw_get"]).inc_by(3);
            registry.register(Box::new(counter)).unwrap();
        }
        for name in histogram_names {
            let histogram = prometheus::HistogramVec::new(
                prometheus::HistogramOpts::new(name, name).buckets(vec![0.005, 0.01]),
                &["type"],
            )
            .unwrap();
            histogram.with_label_values(&["raw_get"]).observe(0.01);
            registry.register(Box::new(histogram)).unwrap();
        }
        let unrelated_counter = prometheus::IntCounterVec::new(
            prometheus::Opts::new("other_requests_total", "other requests"),
            &["type", "store"],
        )
        .unwrap();
        for store in ["1", "2"] {
            unrelated_counter
                .with_label_values(&["raw_get", store])
                .inc();
        }
        registry.register(Box::new(unrelated_counter)).unwrap();
        for name in ["other_duration_seconds", "pd_tso_batch_size"] {
            let histogram =
                prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(name, name))
                    .unwrap();
            histogram.observe(1.0);
            registry.register(Box::new(histogram)).unwrap();
        }

        let snapshot = collect_metrics_snapshot(&registry);
        let mut names = HashSet::new();
        let mut counters = 0;
        let mut histogram_counts = 0;
        for sample in &snapshot.samples {
            names.insert(unsafe { CStr::from_ptr(sample.name) }.to_str().unwrap());
            assert_eq!(
                unsafe { CStr::from_ptr(sample.type_label) }.to_bytes(),
                b"raw_get"
            );
            match sample.kind {
                tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_COUNTER => {
                    counters += 1;
                    assert_eq!(sample.value, 3.0);
                }
                tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET => {
                    assert!(sample.le == 0.005 || sample.le == 0.01 || sample.le == f64::INFINITY);
                    assert_eq!(sample.value, if sample.le < 0.01 { 0.0 } else { 1.0 });
                }
                tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_SUM => {
                    assert_eq!(sample.value, 0.01);
                }
                tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT => {
                    histogram_counts += 1;
                    assert_eq!(sample.value, 1.0);
                }
                kind => panic!("unexpected sample kind: {kind:?}"),
            }
        }
        assert_eq!(counters, counter_names.len());
        assert_eq!(histogram_counts, histogram_names.len());
        assert_eq!(
            names,
            counter_names.into_iter().chain(histogram_names).collect()
        );
    }

    #[test]
    fn snapshot_skips_incompatible_types_and_labels() {
        let registry = prometheus::Registry::new();
        let counter = prometheus::IntCounterVec::new(
            prometheus::Opts::new("tikv_request_total", "extra labels"),
            &["type", "store"],
        )
        .unwrap();
        for store in ["1", "2"] {
            counter.with_label_values(&["raw_get", store]).inc();
        }
        registry.register(Box::new(counter)).unwrap();
        let histogram = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new("tikv_request_duration_seconds", "extra labels"),
            &["type", "store"],
        )
        .unwrap();
        for store in ["1", "2"] {
            histogram
                .with_label_values(&["raw_get", store])
                .observe(0.01);
        }
        registry.register(Box::new(histogram)).unwrap();
        let counter = prometheus::IntCounter::new("pd_request_total", "missing label").unwrap();
        counter.inc();
        registry.register(Box::new(counter)).unwrap();
        let histogram = prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
            "pd_request_duration_seconds",
            "missing label",
        ))
        .unwrap();
        histogram.observe(0.01);
        registry.register(Box::new(histogram)).unwrap();
        let gauge = prometheus::IntGaugeVec::new(
            prometheus::Opts::new("tikv_failed_request_total", "wrong type"),
            &["type"],
        )
        .unwrap();
        gauge.with_label_values(&["raw_get"]).set(1);
        registry.register(Box::new(gauge)).unwrap();
        let counter = prometheus::IntCounterVec::new(
            prometheus::Opts::new("tikv_failed_request_duration_seconds", "wrong type"),
            &["type"],
        )
        .unwrap();
        counter.with_label_values(&["raw_get"]).inc();
        registry.register(Box::new(counter)).unwrap();
        let histogram = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new("pd_failed_request_duration_seconds", "wrong label"),
            &["operation"],
        )
        .unwrap();
        histogram.with_label_values(&["raw_get"]).observe(0.01);
        registry.register(Box::new(histogram)).unwrap();

        assert!(collect_metrics_snapshot(&registry).samples.is_empty());
    }
}
