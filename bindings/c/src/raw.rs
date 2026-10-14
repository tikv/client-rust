// Copyright (c) 2026 PingCAP, Inc.

use super::*;
use crate::request::submit_common;

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
                    Ok(OwnedMutation::Put(KvPair::new(key, value), 0))
                }
                MutationOp::Delete => Ok(OwnedMutation::Delete(key)),
            }
        })
        .collect()
}

pub(crate) fn slice_mutations_with_ttl_to_owned(
    raw: tikv_async_raw_mutations_with_ttl_t,
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
                    Ok(OwnedMutation::Put(
                        KvPair::new(key, value),
                        mutation.ttl_secs,
                    ))
                }
                MutationOp::Delete => Ok(OwnedMutation::Delete(key)),
            }
        })
        .collect()
}

pub(crate) fn build_write_batch_runs(mutations: Vec<OwnedMutation>) -> Vec<WriteBatchRun> {
    let mut runs = Vec::new();
    let mut put_pairs = Vec::new();
    let mut put_keys = HashSet::new();
    let mut delete_keys = Vec::new();

    for mutation in mutations {
        match mutation {
            OwnedMutation::Put(pair, ttl_secs) => {
                if !delete_keys.is_empty() {
                    runs.push(WriteBatchRun::Delete(mem::take(&mut delete_keys)));
                }

                let key = pair.key().clone();
                if put_keys.contains(&key) && !put_pairs.is_empty() {
                    runs.push(WriteBatchRun::Put(mem::take(&mut put_pairs)));
                    put_keys.clear();
                }
                put_keys.insert(key);
                put_pairs.push((pair, ttl_secs));
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
        None
    } else {
        Some(slice_to_key(end_key, "end_key")?)
    };
    Ok(BoundRange::from((start, end)))
}

async fn submit_put_batch(client: &RawClient, puts: Vec<(KvPair, u64)>) -> FfiResult<()> {
    if puts.is_empty() {
        return Ok(());
    }

    let (pairs, ttls): (Vec<_>, Vec<_>) = puts.into_iter().unzip();
    client
        .batch_put_with_ttl(pairs, ttls)
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

#[no_mangle]
pub extern "C" fn tikv_async_raw_put_submit(
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

    match run_ffi(out_error, || {
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
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_put_with_ttl_submit(
    client: *const tikv_async_client_t,
    key: tikv_slice_t,
    value: tikv_slice_t,
    ttl_secs: u64,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    match run_ffi(out_error, || {
        let key = slice_to_key(key, "key")?;
        let value = slice_to_vec(value, "value")?;
        submit_common(
            client,
            RequestKind::PutWithTtl,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    client
                        .put_with_ttl(key, value, ttl_secs)
                        .await
                        .map_err(|error| FfiError::error(error.to_string()))?;
                    Ok(RequestOutput::Unit)
                }))
            },
        )
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_get_submit(
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

    match run_ffi(out_error, || {
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
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_get_key_ttl_submit(
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

    match run_ffi(out_error, || {
        let key = slice_to_key(key, "key")?;
        submit_common(
            client,
            RequestKind::GetKeyTtl,
            completion,
            userdata,
            out_request,
            move |client| {
                Ok(boxed_future(async move {
                    let ttl = client
                        .get_key_ttl_secs(key)
                        .await
                        .map_err(|error| FfiError::error(error.to_string()))?;
                    Ok(RequestOutput::KeyTtl(ttl))
                }))
            },
        )
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_delete_submit(
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

    match run_ffi(out_error, || {
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
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_delete_range_submit(
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

    match run_ffi(out_error, || {
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
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_scan_submit(
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

    match run_ffi(out_error, || {
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
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_scan_reverse_submit(
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

    match run_ffi(out_error, || {
        if slice_is_empty(start_key) {
            return Err(FfiError::invalid_argument(
                "scan_reverse does not support empty start_key in client-rust",
            ));
        }
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
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_batch_write_submit(
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

    match run_ffi(out_error, || {
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
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_batch_write_with_ttl_submit(
    client: *const tikv_async_client_t,
    mutations: tikv_async_raw_mutations_with_ttl_t,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if !out_request.is_null() {
        unsafe { *out_request = ptr::null_mut() };
    }

    match run_ffi(out_error, || {
        let runs = build_write_batch_runs(slice_mutations_with_ttl_to_owned(mutations)?);
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
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_raw_batch_get_submit(
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

    match run_ffi(out_error, || {
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
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}
