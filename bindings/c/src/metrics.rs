// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

use super::*;
use prometheus::{IntCounter, IntCounterVec, IntGauge, Opts, Registry};
use std::sync::LazyLock;

pub(crate) struct BindingMetrics {
    accepted: IntCounter,
    rejected: IntCounter,
    completed: [IntCounter; 3],
    inflight: IntGauge,
}

pub(crate) fn binding_metrics() -> &'static Arc<BindingMetrics> {
    static METRICS: LazyLock<Arc<BindingMetrics>> = LazyLock::new(|| {
        Arc::new(BindingMetrics::new(prometheus::default_registry()).expect("register FFI metrics"))
    });
    &METRICS
}

impl BindingMetrics {
    pub(crate) fn new(registry: &Registry) -> prometheus::Result<Self> {
        let submissions = IntCounterVec::new(
            Opts::new("tikv_ffi_submissions_total", "C request submissions"),
            &["type"],
        )?;
        let completions = IntCounterVec::new(
            Opts::new(
                "tikv_ffi_completions_total",
                "C callbacks that have returned",
            ),
            &["type"],
        )?;
        let inflight = IntGauge::new(
            "tikv_ffi_requests_inflight",
            "Accepted C requests whose callbacks have not returned",
        )?;
        let metrics = Self {
            accepted: submissions.with_label_values(&["accepted"]),
            rejected: submissions.with_label_values(&["rejected"]),
            completed: ["ok", "error", "panic"]
                .map(|label| completions.with_label_values(&[label])),
            inflight: inflight.clone(),
        };
        registry.register(Box::new(submissions))?;
        registry.register(Box::new(completions))?;
        registry.register(Box::new(inflight))?;
        Ok(metrics)
    }

    // Wrap the whole submit call: input validation can fail before submit_common.
    pub(crate) fn submit<F>(&self, out_error: *mut *mut tikv_error_t, f: F) -> tikv_status_t
    where
        F: FnOnce() -> FfiResult<()>,
    {
        match run_ffi(out_error, f) {
            Ok(()) => tikv_status_t::TIKV_STATUS_OK,
            Err(status) => {
                self.rejected.inc();
                status
            }
        }
    }

    pub(crate) fn accept(&self) {
        self.accepted.inc();
        self.inflight.inc();
    }

    pub(crate) fn complete(&self, status: tikv_status_t) {
        let index = match status {
            tikv_status_t::TIKV_STATUS_OK => 0,
            tikv_status_t::TIKV_STATUS_PANIC => 2,
            _ => 1,
        };
        self.completed[index].inc();
    }

    pub(crate) fn release(&self) {
        self.inflight.dec();
    }

    #[cfg(test)]
    pub(crate) fn counts(&self) -> (u64, u64, [u64; 3], i64) {
        (
            self.accepted.get(),
            self.rejected.get(),
            self.completed.each_ref().map(IntCounter::get),
            self.inflight.get(),
        )
    }

    #[cfg(test)]
    pub(crate) fn assert_counts(
        &self,
        accepted: u64,
        rejected: u64,
        completed: [u64; 3],
        inflight: i64,
    ) {
        assert_eq!(self.counts(), (accepted, rejected, completed, inflight));
    }
}

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
        type_label: Option<&str>,
        value: f64,
        le: f64,
    ) {
        let name = ffi_cstring(name);
        let type_label = type_label.map(ffi_cstring);
        self.samples.push(tikv_metric_sample_t {
            name: name.as_ptr(),
            type_label: type_label
                .as_ref()
                .map_or(ptr::null(), |label| label.as_ptr()),
            kind,
            value,
            le,
        });
        self._strings.push(name);
        self._strings.extend(type_label);
    }
}

fn collect_metrics_snapshot(registry: &prometheus::Registry) -> MetricsSnapshot {
    let mut snapshot = MetricsSnapshot {
        samples: Vec::new(),
        _strings: Vec::new(),
    };

    for family in registry.gather() {
        let name = family.get_name();
        // Counters and histograms have a single `type` label; the FFI gauge
        // has none. Restrict names, types, and labels to a lossless C view.
        if !matches!(
            (name, family.get_field_type()),
            (
                "tikv_request_total"
                    | "tikv_failed_request_total"
                    | "pd_request_total"
                    | "pd_failed_request_total"
                    | "tikv_ffi_submissions_total"
                    | "tikv_ffi_completions_total",
                MetricType::COUNTER
            ) | (
                "tikv_request_duration_seconds"
                    | "tikv_failed_request_duration_seconds"
                    | "pd_request_duration_seconds"
                    | "pd_failed_request_duration_seconds",
                MetricType::HISTOGRAM
            ) | ("tikv_ffi_requests_inflight", MetricType::GAUGE)
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
                        Some(type_label),
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
                            Some(type_label),
                            bucket.get_cumulative_count() as f64,
                            upper_bound,
                        );
                    }
                    if !inf_seen {
                        snapshot.push(
                            name,
                            tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET,
                            Some(type_label),
                            histogram.get_sample_count() as f64,
                            f64::INFINITY,
                        );
                    }
                    snapshot.push(
                        name,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_SUM,
                        Some(type_label),
                        histogram.get_sample_sum(),
                        0.0,
                    );
                    snapshot.push(
                        name,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT,
                        Some(type_label),
                        histogram.get_sample_count() as f64,
                        0.0,
                    );
                }
            }
            MetricType::GAUGE => {
                for metric in family.get_metric() {
                    if metric.get_label().is_empty() {
                        snapshot.push(
                            name,
                            tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_GAUGE,
                            None,
                            metric.get_gauge().get_value(),
                            0.0,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    snapshot
}

/// # Safety
/// Non-null output slots must be aligned, writable, mutually disjoint, and
/// exclusively accessible for this call. Returned views and their strings
/// must not be modified or used after the snapshot is destroyed.
#[no_mangle]
pub unsafe extern "C" fn tikv_metrics_snapshot_collect(
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

/// # Safety
/// A non-null `snapshot` must be a live snapshot from this library, destroyed
/// exactly once after all accesses to its sample array and strings have ended.
#[no_mangle]
pub unsafe extern "C" fn tikv_metrics_snapshot_destroy(snapshot: *mut tikv_metrics_snapshot_t) {
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
    fn snapshot_exports_only_supported_client_and_binding_families() {
        // A known observation checks cumulative buckets, sum, count, and labels
        // for every supported family while unrelated registry data is excluded.
        let registry = prometheus::Registry::new();
        let binding = BindingMetrics::new(&registry).unwrap();
        binding.accept();
        binding.accept();
        binding.complete(tikv_status_t::TIKV_STATUS_ERROR);
        binding.release();
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
        let mut histogram_buckets = 0;
        let mut histogram_sums = 0;
        let mut histogram_counts = 0;
        let mut binding_samples = 0;
        for sample in &snapshot.samples {
            let name = unsafe { CStr::from_ptr(sample.name) }.to_str().unwrap();
            names.insert(name);
            if name.starts_with("tikv_ffi_") {
                // Export real binding collectors, including zero-valued series
                // and an unlabeled gauge while one accepted request is active.
                binding_samples += 1;
                if name == "tikv_ffi_requests_inflight" {
                    assert_eq!(
                        sample.kind,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_GAUGE
                    );
                    assert!(sample.type_label.is_null());
                    assert_eq!(sample.value, 1.0);
                } else {
                    assert_eq!(
                        sample.kind,
                        tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_COUNTER
                    );
                    let label = unsafe { CStr::from_ptr(sample.type_label) }
                        .to_str()
                        .unwrap();
                    let expected = match (name, label) {
                        ("tikv_ffi_submissions_total", "accepted") => 2.0,
                        ("tikv_ffi_completions_total", "error") => 1.0,
                        ("tikv_ffi_submissions_total", "rejected")
                        | ("tikv_ffi_completions_total", "ok" | "panic") => 0.0,
                        _ => panic!("unexpected binding metric: {name} {label}"),
                    };
                    assert_eq!(sample.value, expected);
                }
                continue;
            }
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
                    histogram_buckets += 1;
                    assert!(sample.le == 0.005 || sample.le == 0.01 || sample.le == f64::INFINITY);
                    assert_eq!(sample.value, if sample.le < 0.01 { 0.0 } else { 1.0 });
                }
                tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_SUM => {
                    histogram_sums += 1;
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
        // Require all two finite buckets, the +Inf bucket, and the sum; checking
        // values only would also pass if a sample kind disappeared entirely.
        assert_eq!(histogram_buckets, histogram_names.len() * 3);
        assert_eq!(histogram_sums, histogram_names.len());
        assert_eq!(histogram_counts, histogram_names.len());
        assert_eq!(binding_samples, 6);
        assert_eq!(
            names,
            counter_names
                .into_iter()
                .chain(histogram_names)
                .chain([
                    "tikv_ffi_submissions_total",
                    "tikv_ffi_completions_total",
                    "tikv_ffi_requests_inflight",
                ])
                .collect()
        );
    }

    #[test]
    fn snapshot_skips_incompatible_types_and_labels() {
        // Incompatible series must not be exported with lost labels or a
        // misleading sample kind, even when their family name is supported.
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

        let gauge = prometheus::IntGaugeVec::new(
            prometheus::Opts::new("tikv_ffi_requests_inflight", "unexpected label"),
            &["type"],
        )
        .unwrap();
        gauge.with_label_values(&["put"]).set(1);
        registry.register(Box::new(gauge)).unwrap();

        assert!(collect_metrics_snapshot(&registry).samples.is_empty());
    }
}
