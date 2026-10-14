// Copyright (c) 2026 PingCAP, Inc.

use super::*;

struct MetricsSnapshot {
    samples: Vec<MetricSample>,
}

struct MetricSample {
    ffi: tikv_metric_sample_t,
    _name: CString,
    _type_label: Option<CString>,
}

fn ffi_cstring(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).expect("sanitized string")
}

fn metric_type_label(metric: &prometheus::proto::Metric) -> Option<&str> {
    metric
        .get_label()
        .iter()
        .find(|label| label.get_name() == "type")
        .map(|label| label.get_value())
}

fn new_metric_sample(
    name: &str,
    kind: tikv_metric_sample_kind_t,
    type_label: Option<&str>,
    value: f64,
    le: f64,
) -> MetricSample {
    let name = ffi_cstring(name);
    let type_label = type_label.map(ffi_cstring);
    let ffi = tikv_metric_sample_t {
        name: name.as_ptr(),
        type_label: type_label
            .as_ref()
            .map_or(ptr::null(), |label| label.as_ptr()),
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

fn collect_metrics_snapshot() -> MetricsSnapshot {
    let mut samples = Vec::new();

    for family in prometheus::gather() {
        let name = family.get_name();

        match family.get_field_type() {
            MetricType::COUNTER => {
                for metric in family.get_metric() {
                    let type_label = metric_type_label(metric);
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
                    let type_label = metric_type_label(metric);
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
        let snapshot = Box::new(collect_metrics_snapshot());
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
