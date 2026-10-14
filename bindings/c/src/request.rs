// Copyright (c) 2026 PingCAP, Inc.

use super::*;

fn ensure_accepting(core: &AsyncClientCore) -> FfiResult<()> {
    if !core.lifecycle.accepting.load(Ordering::Acquire) {
        return Err(FfiError::rejected(
            "client is shutting down and rejects new requests",
        ));
    }
    Ok(())
}

fn ensure_request_kind(request: &RequestState, expected: RequestKind) -> FfiResult<()> {
    if request.kind != expected {
        return Err(FfiError::invalid_argument(format!(
            "request does not carry a {} result",
            expected.name()
        )));
    }
    Ok(())
}

fn make_request_state(
    kind: RequestKind,
    core: &Arc<AsyncClientCore>,
    callback: tikv_async_completion_cb,
    userdata: *mut c_void,
) -> *mut RequestState {
    inflight_add(core);
    let request = Box::new(RequestState {
        kind,
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
        client: Arc::downgrade(core),
        callback: Some(callback),
        userdata,
        retain_count: AtomicUsize::new(1),
    });
    let request_ptr = Box::into_raw(request);
    let mut requests = core
        .lifecycle
        .requests
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    requests.push(request_ptr as usize);
    request_ptr
}

fn remove_request_from_client(core: &AsyncClientCore, request_ptr: *mut RequestState) {
    let mut requests = core
        .lifecycle
        .requests
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    requests.retain(|entry| *entry != request_ptr as usize);
}

fn finalize_request(
    request_ptr: *mut RequestState,
    status: tikv_status_t,
    output: Option<RequestOutput>,
    error_message: Option<String>,
) {
    let (callback, userdata) = {
        let request = unsafe { &*request_ptr };
        let mut completion = lock_completion(request);
        completion.complete = true;
        completion.callback_running = true;
        completion.completion_status = Some(status);
        completion.output = output;
        completion.error_message =
            error_message.and_then(|message| CString::new(message.replace('\0', " ")).ok());
        request.ready.notify_all();
        (request.callback, request.userdata)
    };

    retain_request(request_ptr);
    if let Some(callback) = callback {
        callback(request_ptr as *mut tikv_async_request_t, status, userdata);
    }

    let request = unsafe { &*request_ptr };
    {
        let mut completion = lock_completion(request);
        completion.callback_running = false;
        completion.callback_done = true;
        request.ready.notify_all();
    }

    if let Some(core) = request.client.upgrade() {
        remove_request_from_client(&core, request_ptr);
        inflight_done(&core);
    }

    release_request(request_ptr);
    release_request(request_ptr);
}

fn spawn_request<F>(core: Arc<AsyncClientCore>, request_ptr: *mut RequestState, future: F)
where
    F: std::future::Future<Output = FfiResult<RequestOutput>> + Send + 'static,
{
    let handle: Handle = core.runtime.handle().clone();
    let keepalive = Arc::clone(&core);
    let request_addr = request_ptr as usize;

    handle.spawn(async move {
        let result = AssertUnwindSafe(future).catch_unwind().await;
        let request_ptr = request_addr as *mut RequestState;
        let cancelled = {
            let request = unsafe { &*request_ptr };
            let completion = lock_completion(request);
            completion.cancel_requested
                || keepalive.lifecycle.cancel_inflight.load(Ordering::Acquire)
        };

        let (status, output, error_message) = match result {
            Ok(Ok(_)) if cancelled => (
                tikv_status_t::TIKV_STATUS_CANCELLED,
                None,
                Some("request cancelled".to_owned()),
            ),
            Ok(Ok(output)) => (tikv_status_t::TIKV_STATUS_OK, Some(output), None),
            Ok(Err(error)) => (error.status, None, Some(error.message)),
            Err(payload) => (
                tikv_status_t::TIKV_STATUS_PANIC,
                None,
                Some(panic_message(payload)),
            ),
        };

        finalize_request(request_ptr, status, output, error_message);
        drop(keepalive);
    });
}

pub(crate) fn submit_common<F>(
    client: *const tikv_async_client_t,
    kind: RequestKind,
    completion: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    out_request: *mut *mut tikv_async_request_t,
    build_future: F,
) -> FfiResult<()>
where
    F: FnOnce(RawClient) -> FfiResult<PinBoxRequestFuture>,
{
    if out_request.is_null() {
        return Err(FfiError::invalid_argument("out_request must not be null"));
    }

    let completion = completion
        .ok_or_else(|| FfiError::invalid_argument("completion callback must not be null"))?;
    let handle = client_ref(client)?;
    ensure_accepting(&handle.core)?;

    let request_ptr = make_request_state(kind, &handle.core, completion, userdata);
    let future = match build_future(handle.core.client.clone()) {
        Ok(future) => future,
        Err(error) => {
            remove_request_from_client(&handle.core, request_ptr);
            inflight_done(&handle.core);
            release_request(request_ptr);
            return Err(error);
        }
    };

    if !handle.core.lifecycle.accepting.load(Ordering::Acquire) {
        remove_request_from_client(&handle.core, request_ptr);
        inflight_done(&handle.core);
        release_request(request_ptr);
        return Err(FfiError::rejected(
            "client is shutting down and rejects new requests",
        ));
    }

    retain_request(request_ptr);
    unsafe {
        *out_request = request_ptr as *mut tikv_async_request_t;
    }
    spawn_request(Arc::clone(&handle.core), request_ptr, future);

    if handle
        .core
        .lifecycle
        .cancel_inflight
        .load(Ordering::Acquire)
    {
        let request = unsafe { &*request_ptr };
        let mut completion = lock_completion(request);
        completion.cancel_requested = true;
    }

    Ok(())
}

fn take_request_output<'a>(
    request: &'a RequestState,
    expected: RequestKind,
) -> FfiResult<std::sync::MutexGuard<'a, CompletionState>> {
    ensure_request_kind(request, expected)?;
    let completion = lock_completion(request);
    if !completion.complete {
        return Err(FfiError::invalid_argument("request is still pending"));
    }
    if completion.completion_status != Some(tikv_status_t::TIKV_STATUS_OK) {
        return Err(FfiError::invalid_argument(
            "request did not complete successfully",
        ));
    }
    if completion.output_taken {
        return Err(FfiError::invalid_argument("request result already taken"));
    }
    Ok(completion)
}

#[no_mangle]
pub extern "C" fn tikv_async_request_cancel(
    request: *mut tikv_async_request_t,
    out_cancel_requested: *mut bool,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_cancel_requested.is_null() {
        clear_error_slot(out_error);
        set_error_slot(
            out_error,
            "out_cancel_requested must not be null".to_owned(),
        );
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        *out_cancel_requested = false;
    }

    match run_ffi(out_error, || {
        let request = request_ref(request)?;
        let mut completion = lock_completion(request);
        if completion.complete || completion.cancel_requested {
            return Ok(());
        }
        completion.cancel_requested = true;
        unsafe {
            *out_cancel_requested = true;
        }
        Ok(())
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_request_is_complete(
    request: *const tikv_async_request_t,
    out_complete: *mut bool,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_complete.is_null() {
        clear_error_slot(out_error);
        set_error_slot(out_error, "out_complete must not be null".to_owned());
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        *out_complete = false;
    }

    match run_ffi(out_error, || {
        let request = request_ref(request)?;
        let completion = lock_completion(request);
        unsafe {
            *out_complete = completion.complete;
        }
        Ok(())
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_request_completion_status(
    request: *const tikv_async_request_t,
    out_status: *mut tikv_status_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_status.is_null() {
        clear_error_slot(out_error);
        set_error_slot(out_error, "out_status must not be null".to_owned());
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        *out_status = tikv_status_t::TIKV_STATUS_ERROR;
    }

    match run_ffi(out_error, || {
        let request = request_ref(request)?;
        let completion = lock_completion(request);
        if !completion.complete {
            return Err(FfiError::invalid_argument("request is still pending"));
        }
        unsafe {
            *out_status = completion.completion_status.expect("completed request");
        }
        Ok(())
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_request_error_message(
    request: *const tikv_async_request_t,
) -> *const c_char {
    let Ok(request) = request_ref(request) else {
        return ptr::null();
    };
    let completion = lock_completion(request);
    completion
        .error_message
        .as_ref()
        .map(|message| message.as_ptr())
        .unwrap_or(ptr::null())
}

#[no_mangle]
pub extern "C" fn tikv_async_request_take_get_result(
    request: *const tikv_async_request_t,
    out_value: *mut tikv_owned_bytes_t,
    out_found: *mut bool,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_value.is_null() || out_found.is_null() {
        clear_error_slot(out_error);
        set_error_slot(
            out_error,
            "out_value and out_found must not be null".to_owned(),
        );
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        *out_value = empty_owned_bytes();
        *out_found = false;
    }

    match run_ffi(out_error, || {
        let request = request_ref(request)?;
        let mut completion = take_request_output(request, RequestKind::Get)?;
        match completion.output.as_mut().expect("successful request") {
            RequestOutput::Get(value) => {
                let value = value.take();
                unsafe {
                    match value {
                        Some(value) => {
                            *out_value = bytes_to_owned(value);
                            *out_found = true;
                        }
                        None => {
                            *out_value = empty_owned_bytes();
                            *out_found = false;
                        }
                    }
                }
                completion.output_taken = true;
                Ok(())
            }
            _ => Err(FfiError::error("internal get result mismatch")),
        }
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_request_take_get_key_ttl_result(
    request: *const tikv_async_request_t,
    out_ttl_secs: *mut u64,
    out_found: *mut bool,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_ttl_secs.is_null() || out_found.is_null() {
        clear_error_slot(out_error);
        set_error_slot(
            out_error,
            "out_ttl_secs and out_found must not be null".to_owned(),
        );
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        *out_ttl_secs = 0;
        *out_found = false;
    }

    match run_ffi(out_error, || {
        let request = request_ref(request)?;
        let mut completion = take_request_output(request, RequestKind::GetKeyTtl)?;
        match completion.output.as_mut().expect("successful request") {
            RequestOutput::KeyTtl(ttl) => {
                let ttl = ttl.take();
                unsafe {
                    match ttl {
                        Some(ttl) => {
                            *out_ttl_secs = ttl;
                            *out_found = true;
                        }
                        None => {
                            *out_ttl_secs = 0;
                            *out_found = false;
                        }
                    }
                }
                completion.output_taken = true;
                Ok(())
            }
            _ => Err(FfiError::error("internal get_key_ttl result mismatch")),
        }
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_request_take_scan_result(
    request: *const tikv_async_request_t,
    out_pairs: *mut tikv_kv_pairs_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_pairs.is_null() {
        clear_error_slot(out_error);
        set_error_slot(out_error, "out_pairs must not be null".to_owned());
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        *out_pairs = empty_kv_pairs();
    }

    match run_ffi(out_error, || {
        let request = request_ref(request)?;
        let expected = match request.kind {
            RequestKind::Scan => RequestKind::Scan,
            RequestKind::ScanReverse => RequestKind::ScanReverse,
            _ => {
                return Err(FfiError::invalid_argument(
                    "request does not carry a scan result",
                ))
            }
        };
        let mut completion = take_request_output(request, expected)?;
        match completion.output.take().expect("successful request") {
            RequestOutput::Pairs(pairs) => {
                unsafe {
                    *out_pairs = pairs_to_owned(pairs);
                }
                completion.output_taken = true;
                Ok(())
            }
            _ => Err(FfiError::error("internal scan result mismatch")),
        }
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_request_take_batch_get_result(
    request: *const tikv_async_request_t,
    out_pairs: *mut tikv_kv_pairs_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_pairs.is_null() {
        clear_error_slot(out_error);
        set_error_slot(out_error, "out_pairs must not be null".to_owned());
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        *out_pairs = empty_kv_pairs();
    }

    match run_ffi(out_error, || {
        let request = request_ref(request)?;
        let mut completion = take_request_output(request, RequestKind::BatchGet)?;
        match completion.output.take().expect("successful request") {
            RequestOutput::Pairs(pairs) => {
                unsafe {
                    *out_pairs = pairs_to_owned(pairs);
                }
                completion.output_taken = true;
                Ok(())
            }
            _ => Err(FfiError::error("internal batch_get result mismatch")),
        }
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_request_destroy(request: *mut tikv_async_request_t) {
    if request.is_null() {
        return;
    }
    release_request(request_ptr(request));
}
