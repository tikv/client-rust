// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

use super::*;
use crate::request::submit_common;
use std::ops::Bound;

fn slices_to_keys(raw: tikv_slices_t, name: &str) -> FfiResult<Vec<Key>> {
    let slices = slices_to_vec(raw.ptr, raw.len, name)?;
    slices
        .iter()
        .enumerate()
        .map(|(index, item)| slice_to_key(*item, &format!("{name}[{index}]")))
        .collect()
}

pub(crate) fn slice_mutations_to_owned(
    raw: tikv_async_raw_mutations_t,
) -> FfiResult<Vec<OwnedMutation>> {
    let mutations = slices_to_vec(raw.ptr, raw.len, "mutations")?;
    mutations
        .iter()
        .enumerate()
        .map(|(index, mutation)| {
            let op = MutationOp::from_ffi(mutation.op, &format!("mutations[{index}]"))?;
            let key = slice_to_key(mutation.key, &format!("mutations[{index}].key"))?;
            match op {
                MutationOp::Put => {
                    let value = slice_to_vec(mutation.value, &format!("mutations[{index}].value"))?;
                    Ok(OwnedMutation::Put(KvPair::new(key, value)))
                }
                MutationOp::Delete => Ok(OwnedMutation::Delete(key)),
            }
        })
        .collect()
}

/// Splits a mixed mutation list into ordered PUT and DELETE runs.
///
/// Each run contains one operation kind and is submitted with the matching
/// `RawClient` batch method. PUT/DELETE transitions preserve the input order.
/// A repeated key ends the current PUT run so later PUTs for the same key are
/// submitted separately and retain their input-order semantics. This function
/// only builds the runs; it does not submit any requests.
pub(crate) fn build_write_batch_runs(mutations: Vec<OwnedMutation>) -> Vec<WriteBatchRun> {
    let mut runs = Vec::new();
    let mut put_pairs = Vec::new();
    let mut put_keys = HashSet::new();
    let mut delete_keys = Vec::new();

    for mutation in mutations {
        match mutation {
            OwnedMutation::Put(pair) => {
                if !delete_keys.is_empty() {
                    runs.push(WriteBatchRun::Delete(mem::take(&mut delete_keys)));
                }

                let key = pair.key().clone();
                if put_keys.contains(&key) && !put_pairs.is_empty() {
                    runs.push(WriteBatchRun::Put(mem::take(&mut put_pairs)));
                    put_keys.clear();
                }
                put_keys.insert(key);
                put_pairs.push(pair);
            }
            OwnedMutation::Delete(key) => {
                if !put_pairs.is_empty() {
                    runs.push(WriteBatchRun::Put(mem::take(&mut put_pairs)));
                    put_keys.clear();
                }
                delete_keys.push(key);
            }
        }
    }

    if !put_pairs.is_empty() {
        runs.push(WriteBatchRun::Put(put_pairs));
    }
    if !delete_keys.is_empty() {
        runs.push(WriteBatchRun::Delete(delete_keys));
    }

    runs
}

fn slice_to_range(start_key: tikv_slice_t, end_key: tikv_slice_t) -> FfiResult<BoundRange> {
    let start = slice_to_key(start_key, "start_key")?;
    let end = if slice_is_empty(end_key) {
        Bound::Unbounded
    } else {
        Bound::Excluded(slice_to_key(end_key, "end_key")?)
    };
    // Tuple conversion strips trailing NULs and turns b"\0" into an inclusive
    // empty upper bound, which keyspace encoding interprets as unbounded.
    Ok(BoundRange::new(Bound::Included(start), end))
}

async fn submit_put_batch(client: &RawClient, puts: Vec<KvPair>) -> FfiResult<()> {
    if puts.is_empty() {
        return Ok(());
    }

    client
        .batch_put(puts)
        .await
        .map_err(|error| FfiError::error(error.to_string()))?;
    Ok(())
}

async fn submit_delete_batch(client: &RawClient, keys: Vec<Key>) -> FfiResult<()> {
    if keys.is_empty() {
        return Ok(());
    }

    client
        .batch_delete(keys)
        .await
        .map_err(|error| FfiError::error(error.to_string()))?;
    Ok(())
}

async fn execute_write_batch(client: RawClient, runs: Vec<WriteBatchRun>) -> FfiResult<()> {
    for run in runs {
        match run {
            WriteBatchRun::Put(pairs) => submit_put_batch(&client, pairs).await?,
            WriteBatchRun::Delete(keys) => submit_delete_batch(&client, keys).await?,
        }
    }
    Ok(())
}

/// # Safety
/// The client, input buffers, output slots, callback, and userdata must satisfy
/// the [submission safety contract](crate).
#[no_mangle]
pub unsafe extern "C" fn tikv_async_raw_put_submit(
    client: *const tikv_async_client_t,
    key: tikv_slice_t,
    value: tikv_slice_t,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    metrics::binding_metrics().submit(out_error, || {
        let key = slice_to_key(key, "key")?;
        let value = slice_to_vec(value, "value")?;
        submit_common(
            client,
            RequestKind::Put,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    client
                        .put(key, value)
                        .await
                        .map_err(|error| FfiError::error(error.to_string()))?;
                    Ok(RequestOutput::Unit)
                }))
            },
        )
    })
}

/// # Safety
/// The client, input buffers, output slots, callback, and userdata must satisfy
/// the [submission safety contract](crate).
#[no_mangle]
pub unsafe extern "C" fn tikv_async_raw_get_submit(
    client: *const tikv_async_client_t,
    key: tikv_slice_t,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    metrics::binding_metrics().submit(out_error, || {
        let key = slice_to_key(key, "key")?;
        submit_common(
            client,
            RequestKind::Get,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    let value = client
                        .get(key)
                        .await
                        .map_err(|error| FfiError::error(error.to_string()))?;
                    Ok(RequestOutput::Get(value))
                }))
            },
        )
    })
}

/// # Safety
/// The client, input buffers, output slots, callback, and userdata must satisfy
/// the [submission safety contract](crate).
#[no_mangle]
pub unsafe extern "C" fn tikv_async_raw_delete_submit(
    client: *const tikv_async_client_t,
    key: tikv_slice_t,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    metrics::binding_metrics().submit(out_error, || {
        let key = slice_to_key(key, "key")?;
        submit_common(
            client,
            RequestKind::Delete,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    client
                        .delete(key)
                        .await
                        .map_err(|error| FfiError::error(error.to_string()))?;
                    Ok(RequestOutput::Unit)
                }))
            },
        )
    })
}

/// # Safety
/// The client, input buffers, output slots, callback, and userdata must satisfy
/// the [submission safety contract](crate).
#[no_mangle]
pub unsafe extern "C" fn tikv_async_raw_delete_range_submit(
    client: *const tikv_async_client_t,
    start_key: tikv_slice_t,
    end_key: tikv_slice_t,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    metrics::binding_metrics().submit(out_error, || {
        let range = slice_to_range(start_key, end_key)?;
        submit_common(
            client,
            RequestKind::DeleteRange,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    client
                        .delete_range(range)
                        .await
                        .map_err(|error| FfiError::error(error.to_string()))?;
                    Ok(RequestOutput::Unit)
                }))
            },
        )
    })
}

/// # Safety
/// The client, input buffers, output slots, callback, and userdata must satisfy
/// the [submission safety contract](crate).
#[no_mangle]
pub unsafe extern "C" fn tikv_async_raw_scan_submit(
    client: *const tikv_async_client_t,
    start_key: tikv_slice_t,
    end_key: tikv_slice_t,
    limit: u32,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    metrics::binding_metrics().submit(out_error, || {
        let range = slice_to_range(start_key, end_key)?;
        submit_common(
            client,
            RequestKind::Scan,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    let pairs = client
                        .scan(range, limit)
                        .await
                        .map_err(|error| FfiError::error(error.to_string()))?;
                    Ok(RequestOutput::Pairs(pairs))
                }))
            },
        )
    })
}

/// # Safety
/// The client, input buffers, output slots, callback, and userdata must satisfy
/// the [submission safety contract](crate).
#[no_mangle]
pub unsafe extern "C" fn tikv_async_raw_scan_reverse_submit(
    client: *const tikv_async_client_t,
    start_key: tikv_slice_t,
    end_key: tikv_slice_t,
    limit: u32,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    metrics::binding_metrics().submit(out_error, || {
        let range = slice_to_range(start_key, end_key)?;
        submit_common(
            client,
            RequestKind::ScanReverse,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    let pairs = client
                        .scan_reverse(range, limit)
                        .await
                        .map_err(|error| FfiError::error(error.to_string()))?;
                    Ok(RequestOutput::Pairs(pairs))
                }))
            },
        )
    })
}

/// # Safety
/// The client, input buffers, output slots, callback, and userdata must satisfy
/// the [submission safety contract](crate).
#[no_mangle]
pub unsafe extern "C" fn tikv_async_raw_batch_write_submit(
    client: *const tikv_async_client_t,
    mutations: tikv_async_raw_mutations_t,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    metrics::binding_metrics().submit(out_error, || {
        let runs = build_write_batch_runs(slice_mutations_to_owned(mutations)?);
        submit_common(
            client,
            RequestKind::BatchWrite,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    execute_write_batch(client, runs).await?;
                    Ok(RequestOutput::Unit)
                }))
            },
        )
    })
}

/// # Safety
/// The client, input buffers, output slots, callback, and userdata must satisfy
/// the [submission safety contract](crate).
#[no_mangle]
pub unsafe extern "C" fn tikv_async_raw_batch_get_submit(
    client: *const tikv_async_client_t,
    keys: tikv_slices_t,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    metrics::binding_metrics().submit(out_error, || {
        let keys = slices_to_keys(keys, "keys")?;
        submit_common(
            client,
            RequestKind::BatchGet,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    if keys.is_empty() {
                        return Ok(RequestOutput::Pairs(Vec::new()));
                    }
                    let pairs = client
                        .batch_get(keys)
                        .await
                        .map_err(|error| FfiError::error(error.to_string()))?;
                    Ok(RequestOutput::Pairs(pairs))
                }))
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::RangeBounds;

    #[test]
    fn range_preserves_binary_exclusive_endpoints() {
        let as_slice = |bytes: &[u8]| tikv_slice_t {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        };
        let empty = as_slice(b"");
        for end in [b"\0".as_slice(), b"a\0".as_slice()] {
            let range = slice_to_range(empty, as_slice(end)).unwrap();
            assert!(range.contains(&Key::EMPTY));
            assert!(!range.contains(&Key::from(end.to_vec())));
            assert!(!range.contains(&Key::from(b"z".to_vec())));
        }
        let unbounded = slice_to_range(empty, empty).unwrap();
        assert!(unbounded.contains(&Key::from(b"z".to_vec())));
    }
}
