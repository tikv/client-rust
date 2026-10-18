// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

use super::*;

struct MetricsSnapshot {
    samples: Vec<MetricSample>,
}

struct MetricSample {
    ffi: tikv_metric_sample_t,
    _name: CString,
    _type_label: CString,
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

fn new_metric_sample(
    name: &str,
    kind: tikv_metric_sample_kind_t,
    type_label: &str,
    value: f64,
    le: f64,
) -> MetricSample {
    let name = ffi_cstring(name);
    let type_label = ffi_cstring(type_label);
    let ffi = tikv_metric_sample_t {
        name: name.as_ptr(),
        type_label: type_label.as_ptr(),
        kind,
        value,
        le,
    };
    MetricSample {
        ffi,
        _name: name,
        _type_label: type_label,
    }
}

fn collect_metrics_snapshot(registry: &prometheus::Registry) -> MetricsSnapshot {
    let mut samples = Vec::new();

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
                    samples.push(new_metric_sample(
                        name,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_COUNTER,
                        type_label,
                        metric.get_counter().get_value(),
                        0.0,
                    ));
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
                        samples.push(new_metric_sample(
                            name,
                            tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET,
                            type_label,
                            bucket.get_cumulative_count() as f64,
                            upper_bound,
                        ));
                    }
                    if !inf_seen {
                        samples.push(new_metric_sample(
                            name,
                            tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET,
                            type_label,
                            histogram.get_sample_count() as f64,
                            f64::INFINITY,
                        ));
                    }
                    samples.push(new_metric_sample(
                        name,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_SUM,
                        type_label,
                        histogram.get_sample_sum(),
                        0.0,
                    ));
                    samples.push(new_metric_sample(
                        name,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT,
                        type_label,
                        histogram.get_sample_count() as f64,
                        0.0,
                    ));
                }
            }
            _ => {}
        }
    }

    MetricsSnapshot { samples }
}

fn metrics_snapshot_ref<'a>(raw: *const tikv_metrics_snapshot_t) -> FfiResult<&'a MetricsSnapshot> {
    if raw.is_null() {
        return Err(FfiError::invalid_argument("snapshot must not be null"));
    }
    Ok(unsafe { &*(raw as *const MetricsSnapshot) })
}

#[no_mangle]
pub extern "C" fn tikv_metrics_snapshot_collect(
    out_snapshot: *mut *mut tikv_metrics_snapshot_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_snapshot.is_null() {
        clear_error_slot(out_error);
        set_error_slot(out_error, "out_snapshot must not be null".to_owned());
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        *out_snapshot = ptr::null_mut();
    }

    match run_ffi(out_error, || {
        let snapshot = Box::new(collect_metrics_snapshot(prometheus::default_registry()));
        unsafe {
            *out_snapshot = Box::into_raw(snapshot) as *mut tikv_metrics_snapshot_t;
        }
        Ok(())
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_metrics_snapshot_len(snapshot: *const tikv_metrics_snapshot_t) -> usize {
    if snapshot.is_null() {
        return 0;
    }
    unsafe { (*(snapshot as *const MetricsSnapshot)).samples.len() }
}

#[no_mangle]
pub extern "C" fn tikv_metrics_snapshot_get(
    snapshot: *const tikv_metrics_snapshot_t,
    index: usize,
    out_sample: *mut tikv_metric_sample_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_sample.is_null() {
        clear_error_slot(out_error);
        set_error_slot(out_error, "out_sample must not be null".to_owned());
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    match run_ffi(out_error, || {
        let snapshot = metrics_snapshot_ref(snapshot)?;
        let sample = snapshot.samples.get(index).ok_or_else(|| {
            FfiError::invalid_argument(format!("sample index {index} out of range"))
        })?;
        unsafe {
            *out_sample = sample.ffi;
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
            let sample = &sample.ffi;
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
