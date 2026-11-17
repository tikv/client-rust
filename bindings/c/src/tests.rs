// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

use super::*;
use crate::raw::{build_write_batch_runs, slice_mutations_to_owned};

fn synthetic_request(kind: RequestKind, output: RequestOutput) -> *mut tikv_async_request_t {
    Arc::into_raw(Arc::new(RequestState {
        kind,
        completion: RefCell::new(CompletionState {
            complete: true,
            completion_status: Some(tikv_status_t::TIKV_STATUS_OK),
            error_message: None,
            output: Some(output),
            output_taken: false,
        }),
        callback: None,
        userdata: ptr::null_mut(),
    })) as *mut tikv_async_request_t
}

#[test]
fn write_batch_preserves_phase_order_and_splits_repeated_put_keys() {
    let runs = build_write_batch_runs(vec![
        OwnedMutation::Put(KvPair::new(b"a".to_vec(), b"v1".to_vec())),
        OwnedMutation::Put(KvPair::new(b"b".to_vec(), b"v2".to_vec())),
        OwnedMutation::Put(KvPair::new(b"a".to_vec(), b"v3".to_vec())),
        OwnedMutation::Delete(Key::from(b"c".to_vec())),
        OwnedMutation::Delete(Key::from(b"d".to_vec())),
        OwnedMutation::Put(KvPair::new(b"a".to_vec(), b"v4".to_vec())),
        OwnedMutation::Delete(Key::from(b"a".to_vec())),
    ]);

    assert_eq!(
        runs,
        vec![
            WriteBatchRun::Put(vec![
                KvPair::new(b"a".to_vec(), b"v1".to_vec()),
                KvPair::new(b"b".to_vec(), b"v2".to_vec()),
            ]),
            WriteBatchRun::Put(vec![KvPair::new(b"a".to_vec(), b"v3".to_vec())]),
            WriteBatchRun::Delete(vec![Key::from(b"c".to_vec()), Key::from(b"d".to_vec())]),
            WriteBatchRun::Put(vec![KvPair::new(b"a".to_vec(), b"v4".to_vec())]),
            WriteBatchRun::Delete(vec![Key::from(b"a".to_vec())]),
        ]
    );
}

#[test]
fn ffi_mutations_copy_inputs_and_ignore_delete_value() {
    // Async submission must own its inputs, and DELETE must not dereference
    // the unused value slice even if that slice is invalid.
    let mut key = b"put-key".to_vec();
    let mut value = b"put-value".to_vec();
    let mutations = [
        tikv_async_raw_mutation_t {
            op: TIKV_ASYNC_RAW_MUTATION_OP_PUT,
            key: tikv_slice_t {
                ptr: key.as_ptr(),
                len: key.len(),
            },
            value: tikv_slice_t {
                ptr: value.as_ptr(),
                len: value.len(),
            },
        },
        tikv_async_raw_mutation_t {
            op: TIKV_ASYNC_RAW_MUTATION_OP_DELETE,
            key: tikv_slice_t {
                ptr: key.as_ptr(),
                len: key.len(),
            },
            value: tikv_slice_t {
                ptr: ptr::null(),
                len: usize::MAX,
            },
        },
    ];
    let owned = slice_mutations_to_owned(tikv_async_raw_mutations_t {
        ptr: mutations.as_ptr(),
        len: mutations.len(),
    })
    .unwrap();
    key.fill(0);
    value.fill(0);
    assert_eq!(
        owned,
        vec![
            OwnedMutation::Put(KvPair::new(b"put-key".to_vec(), b"put-value".to_vec())),
            OwnedMutation::Delete(Key::from(b"put-key".to_vec())),
        ]
    );
}

#[test]
fn request_batch_get_result_transfers_pairs_once() {
    // Batch results use a separate accessor and nested C allocations; verify
    // their contents, single transfer, and matching destruction helpers.
    let request = synthetic_request(
        RequestKind::BatchGet,
        RequestOutput::Pairs(vec![KvPair::new(b"k1".to_vec(), b"v1".to_vec())]),
    );

    let mut error = ptr::null_mut();
    let mut out_pairs = empty_kv_pairs();
    // SAFETY: the synthetic request is complete and exclusively accessed here;
    // output slots are disjoint, writable locals.
    let status =
        unsafe { tikv_async_request_take_batch_get_result(request, &mut out_pairs, &mut error) };
    assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
    assert!(error.is_null());
    assert_eq!(out_pairs.len, 1);
    let key = unsafe { slice::from_raw_parts((*out_pairs.ptr).key.ptr, (*out_pairs.ptr).key.len) };
    let value =
        unsafe { slice::from_raw_parts((*out_pairs.ptr).value.ptr, (*out_pairs.ptr).value.len) };
    assert_eq!(key, b"k1");
    assert_eq!(value, b"v1");
    // SAFETY: all borrowed pair slices are no longer used; free the array once.
    unsafe { tikv_kv_pairs_destroy(out_pairs) };

    // SAFETY: the request remains live and exclusively accessed on this thread.
    let status =
        unsafe { tikv_async_request_take_batch_get_result(request, &mut out_pairs, &mut error) };
    assert_eq!(status, tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT);
    assert!(!error.is_null());
    // SAFETY: both handles are library-owned and released exactly once.
    unsafe {
        tikv_error_destroy(error);
        tikv_async_request_destroy(request);
    }
}
