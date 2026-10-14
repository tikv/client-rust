// Copyright (c) 2026 PingCAP, Inc.

use super::*;
use crate::client::{build_config, parse_pd_endpoints};
use crate::raw::{
    build_write_batch_runs, slice_mutations_to_owned, slice_mutations_with_ttl_to_owned,
};

extern "C" fn noop_completion(
    _request: *mut tikv_async_request_t,
    _status: tikv_status_t,
    _userdata: *mut c_void,
) {
}

fn synthetic_request(kind: RequestKind, output: RequestOutput) -> *mut tikv_async_request_t {
    Box::into_raw(Box::new(RequestState {
        kind,
        completion: Mutex::new(CompletionState {
            complete: true,
            callback_running: false,
            callback_done: true,
            completion_status: Some(tikv_status_t::TIKV_STATUS_OK),
            error_message: None,
            output: Some(output),
            output_taken: false,
            cancel_requested: false,
        }),
        ready: Condvar::new(),
        client: Weak::new(),
        callback: Some(noop_completion),
        userdata: ptr::null_mut(),
        retain_count: AtomicUsize::new(1),
    })) as *mut tikv_async_request_t
}

#[test]
fn parse_pd_endpoints_csv() {
    let input = CString::new(" 127.0.0.1:2379 , 127.0.0.1:2380 ").unwrap();
    let endpoints = parse_pd_endpoints(input.as_ptr()).unwrap();
    assert_eq!(endpoints, vec!["127.0.0.1:2379", "127.0.0.1:2380"]);
}

#[test]
fn ffi_client_grpc_decoding_limit_is_configurable() {
    let default_limit = Config::default().grpc_max_decoding_message_size;
    assert_eq!(default_limit, 4 * 1024 * 1024);
    assert_eq!(
        build_config(0, false, 0).grpc_max_decoding_message_size,
        default_limit
    );
    assert_eq!(
        build_config(0, false, 32 * 1024 * 1024).grpc_max_decoding_message_size,
        32 * 1024 * 1024
    );
    assert_eq!(
        build_config(0, false, usize::MAX).grpc_max_decoding_message_size,
        usize::MAX
    );
}

#[test]
fn metrics_snapshot_collects_supported_counter_and_histogram() {
    let counter = prometheus::IntCounterVec::new(
        prometheus::Opts::new("tikv_request_total", "test tikv request total"),
        &["type"],
    )
    .expect("counter vec");
    prometheus::default_registry()
        .register(Box::new(counter.clone()))
        .expect("register counter");
    counter.with_label_values(&["raw_get"]).inc_by(3);

    let histogram = prometheus::HistogramVec::new(
        prometheus::HistogramOpts::new(
            "tikv_request_duration_seconds",
            "test tikv request duration",
        ),
        &["type"],
    )
    .expect("histogram vec");
    prometheus::default_registry()
        .register(Box::new(histogram.clone()))
        .expect("register histogram");
    histogram.with_label_values(&["raw_get"]).observe(0.01);

    let mut error = ptr::null_mut();
    let mut snapshot = ptr::null_mut();
    let status = tikv_metrics_snapshot_collect(&mut snapshot, &mut error);
    assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
    assert!(error.is_null());
    assert!(!snapshot.is_null());

    let len = tikv_metrics_snapshot_len(snapshot);
    assert!(len > 0);

    let mut raw_get_total = None;
    let mut raw_get_duration_count = None;
    let mut saw_inf_bucket = false;

    for index in 0..len {
        let mut sample = tikv_metric_sample_t {
            name: ptr::null(),
            type_label: ptr::null(),
            kind: tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_UNKNOWN,
            value: 0.0,
            le: 0.0,
        };
        let status = tikv_metrics_snapshot_get(snapshot, index, &mut sample, &mut error);
        assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
        assert!(error.is_null());

        let name = unsafe { CStr::from_ptr(sample.name) }.to_str().unwrap();
        let type_label = if sample.type_label.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(sample.type_label) }
                    .to_str()
                    .unwrap(),
            )
        };

        if name == "tikv_request_total"
            && sample.kind == tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_COUNTER
            && type_label == Some("raw_get")
        {
            raw_get_total = Some(sample.value);
        }

        if name == "tikv_request_duration_seconds"
            && sample.kind == tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT
            && type_label == Some("raw_get")
        {
            raw_get_duration_count = Some(sample.value);
        }

        if name == "tikv_request_duration_seconds"
            && sample.kind == tikv_metric_sample_kind_t::TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET
            && type_label == Some("raw_get")
            && sample.le.is_infinite()
        {
            saw_inf_bucket = true;
        }
    }

    assert_eq!(raw_get_total, Some(3.0));
    assert_eq!(raw_get_duration_count, Some(1.0));
    assert!(saw_inf_bucket);

    tikv_metrics_snapshot_destroy(snapshot);
}

#[test]
fn write_batch_runs_preserve_put_delete_phase_order() {
    let runs = build_write_batch_runs(vec![
        OwnedMutation::Put(KvPair::new(b"a".to_vec(), b"v1".to_vec()), 10),
        OwnedMutation::Put(KvPair::new(b"b".to_vec(), b"v2".to_vec()), 0),
        OwnedMutation::Delete(Key::from(b"c".to_vec())),
        OwnedMutation::Delete(Key::from(b"d".to_vec())),
        OwnedMutation::Put(KvPair::new(b"e".to_vec(), b"v3".to_vec()), 30),
    ]);

    assert_eq!(
        runs,
        vec![
            WriteBatchRun::Put(vec![
                (KvPair::new(b"a".to_vec(), b"v1".to_vec()), 10),
                (KvPair::new(b"b".to_vec(), b"v2".to_vec()), 0),
            ]),
            WriteBatchRun::Delete(vec![Key::from(b"c".to_vec()), Key::from(b"d".to_vec())]),
            WriteBatchRun::Put(vec![(KvPair::new(b"e".to_vec(), b"v3".to_vec()), 30,)]),
        ]
    );
}

#[test]
fn write_batch_splits_repeated_put_keys() {
    let runs = build_write_batch_runs(vec![
        OwnedMutation::Put(KvPair::new(b"a".to_vec(), b"v1".to_vec()), 10),
        OwnedMutation::Put(KvPair::new(b"b".to_vec(), b"v2".to_vec()), 20),
        OwnedMutation::Put(KvPair::new(b"a".to_vec(), b"v3".to_vec()), 30),
    ]);

    assert_eq!(
        runs,
        vec![
            WriteBatchRun::Put(vec![
                (KvPair::new(b"a".to_vec(), b"v1".to_vec()), 10),
                (KvPair::new(b"b".to_vec(), b"v2".to_vec()), 20),
            ]),
            WriteBatchRun::Put(vec![(KvPair::new(b"a".to_vec(), b"v3".to_vec()), 30,)]),
        ]
    );
}

#[test]
fn ffi_mutations_copy_put_ttl_and_ignore_delete_ttl() {
    let put_key = b"put-key";
    let put_value = b"put-value";
    let delete_key = b"delete-key";
    let mutations = [
        tikv_async_raw_mutation_with_ttl_t {
            op: TIKV_ASYNC_RAW_MUTATION_OP_PUT,
            key: tikv_slice_t {
                ptr: put_key.as_ptr(),
                len: put_key.len(),
            },
            value: tikv_slice_t {
                ptr: put_value.as_ptr(),
                len: put_value.len(),
            },
            ttl_secs: 42,
        },
        tikv_async_raw_mutation_with_ttl_t {
            op: TIKV_ASYNC_RAW_MUTATION_OP_DELETE,
            key: tikv_slice_t {
                ptr: delete_key.as_ptr(),
                len: delete_key.len(),
            },
            value: tikv_slice_t {
                ptr: ptr::null(),
                len: 0,
            },
            ttl_secs: u64::MAX,
        },
    ];

    let owned = slice_mutations_with_ttl_to_owned(tikv_async_raw_mutations_with_ttl_t {
        ptr: mutations.as_ptr(),
        len: mutations.len(),
    })
    .unwrap();

    assert_eq!(
        owned,
        vec![
            OwnedMutation::Put(KvPair::new(put_key.to_vec(), put_value.to_vec()), 42,),
            OwnedMutation::Delete(Key::from(delete_key.to_vec())),
        ]
    );
}

#[test]
fn legacy_ffi_mutations_default_put_ttl_to_zero() {
    let key = b"legacy-key";
    let value = b"legacy-value";
    let mutations = [tikv_async_raw_mutation_t {
        op: TIKV_ASYNC_RAW_MUTATION_OP_PUT,
        key: tikv_slice_t {
            ptr: key.as_ptr(),
            len: key.len(),
        },
        value: tikv_slice_t {
            ptr: value.as_ptr(),
            len: value.len(),
        },
    }];

    let owned = slice_mutations_to_owned(tikv_async_raw_mutations_t {
        ptr: mutations.as_ptr(),
        len: mutations.len(),
    })
    .unwrap();

    assert_eq!(
        owned,
        vec![OwnedMutation::Put(
            KvPair::new(key.to_vec(), value.to_vec()),
            0,
        )]
    );
}

#[test]
fn request_get_result_transfers_owned_bytes_once() {
    let request = synthetic_request(
        RequestKind::Get,
        RequestOutput::Get(Some(b"value".to_vec())),
    );

    let mut error = ptr::null_mut();
    let mut out_value = empty_owned_bytes();
    let mut out_found = false;
    let status =
        tikv_async_request_take_get_result(request, &mut out_value, &mut out_found, &mut error);
    assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
    assert!(error.is_null());
    assert!(out_found);
    let bytes = unsafe { slice::from_raw_parts(out_value.ptr, out_value.len) };
    assert_eq!(bytes, b"value");
    tikv_owned_bytes_destroy(out_value);

    let status =
        tikv_async_request_take_get_result(request, &mut out_value, &mut out_found, &mut error);
    assert_eq!(status, tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT);
    assert!(!error.is_null());
    tikv_error_destroy(error);
    tikv_async_request_destroy(request);
}

#[test]
fn request_get_key_ttl_result_distinguishes_found_states() {
    for (ttl, expected_found, expected_ttl) in
        [(Some(42), true, 42), (Some(0), true, 0), (None, false, 0)]
    {
        let request = synthetic_request(RequestKind::GetKeyTtl, RequestOutput::KeyTtl(ttl));

        let mut error = ptr::null_mut();
        let mut out_ttl_secs = u64::MAX;
        let mut out_found = false;
        let status = tikv_async_request_take_get_key_ttl_result(
            request,
            &mut out_ttl_secs,
            &mut out_found,
            &mut error,
        );
        assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
        assert!(error.is_null());
        assert_eq!(out_found, expected_found);
        assert_eq!(out_ttl_secs, expected_ttl);

        let status = tikv_async_request_take_get_key_ttl_result(
            request,
            &mut out_ttl_secs,
            &mut out_found,
            &mut error,
        );
        assert_eq!(status, tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT);
        assert!(!error.is_null());
        tikv_error_destroy(error);
        tikv_async_request_destroy(request);
    }
}

#[test]
fn request_get_key_ttl_result_rejects_wrong_request_kind() {
    let request = synthetic_request(RequestKind::Get, RequestOutput::Get(None));
    let mut error = ptr::null_mut();
    let mut out_ttl_secs = u64::MAX;
    let mut out_found = true;

    let status = tikv_async_request_take_get_key_ttl_result(
        request,
        &mut out_ttl_secs,
        &mut out_found,
        &mut error,
    );

    assert_eq!(status, tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT);
    assert!(!error.is_null());
    assert_eq!(out_ttl_secs, 0);
    assert!(!out_found);
    tikv_error_destroy(error);
    tikv_async_request_destroy(request);
}

#[test]
fn request_batch_get_result_transfers_pairs_once() {
    let request = synthetic_request(
        RequestKind::BatchGet,
        RequestOutput::Pairs(vec![KvPair::new(b"k1".to_vec(), b"v1".to_vec())]),
    );

    let mut error = ptr::null_mut();
    let mut out_pairs = empty_kv_pairs();
    let status = tikv_async_request_take_batch_get_result(request, &mut out_pairs, &mut error);
    assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
    assert!(error.is_null());
    assert_eq!(out_pairs.len, 1);
    let key = unsafe { slice::from_raw_parts((*out_pairs.ptr).key.ptr, (*out_pairs.ptr).key.len) };
    let value =
        unsafe { slice::from_raw_parts((*out_pairs.ptr).value.ptr, (*out_pairs.ptr).value.len) };
    assert_eq!(key, b"k1");
    assert_eq!(value, b"v1");
    tikv_kv_pairs_destroy(out_pairs);

    let status = tikv_async_request_take_batch_get_result(request, &mut out_pairs, &mut error);
    assert_eq!(status, tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT);
    assert!(!error.is_null());
    tikv_error_destroy(error);
    tikv_async_request_destroy(request);
}

#[test]
fn request_cancel_is_idempotent() {
    let request = Box::into_raw(Box::new(RequestState {
        kind: RequestKind::Put,
        completion: Mutex::new(CompletionState {
            complete: false,
            callback_running: false,
            callback_done: false,
            completion_status: None,
            error_message: None,
            output: None,
            output_taken: false,
            cancel_requested: false,
        }),
        ready: Condvar::new(),
        client: Weak::new(),
        callback: Some(noop_completion),
        userdata: ptr::null_mut(),
        retain_count: AtomicUsize::new(1),
    })) as *mut tikv_async_request_t;

    let mut error = ptr::null_mut();
    let mut cancel_requested = false;
    let status = tikv_async_request_cancel(request, &mut cancel_requested, &mut error);
    assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
    assert!(error.is_null());
    assert!(cancel_requested);

    let status = tikv_async_request_cancel(request, &mut cancel_requested, &mut error);
    assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
    assert!(error.is_null());
    assert!(!cancel_requested);

    tikv_async_request_destroy(request);
}
