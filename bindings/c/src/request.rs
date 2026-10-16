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
    lifecycle: &Arc<ClientLifecycle>,
    callback: tikv_async_completion_cb,
    userdata: *mut c_void,
) -> *mut RequestState {
    inflight_add(lifecycle);
    let request = Box::new(RequestState {
        kind,
        completion: Mutex::new(CompletionState {
            complete: false,
            completion_status: None,
            error_message: None,
            output: None,
            output_taken: false,
        }),
        lifecycle: Arc::clone(lifecycle),
        callback: Some(callback),
        userdata,
        retain_count: AtomicUsize::new(1),
    });
    Box::into_raw(request)
}

fn finalize_request(
    request_ptr: *mut RequestState,
    status: tikv_status_t,
    output: Option<RequestOutput>,
    error_message: Option<String>,
) {
    let (callback, userdata, lifecycle) = {
        let request = unsafe { &*request_ptr };
        let mut completion = lock_completion(request);
        completion.complete = true;
        completion.completion_status = Some(status);
        completion.output = output;
        completion.error_message =
            error_message.and_then(|message| CString::new(message.replace('\0', " ")).ok());
        (
            request.callback,
            request.userdata,
            Arc::clone(&request.lifecycle),
        )
    };

    if let Some(callback) = callback {
        callback(request_ptr as *mut tikv_async_request_t, status, userdata);
    }

    // The worker reference keeps the state alive if the callback destroys it.
    release_request(request_ptr);
    inflight_done(&lifecycle);
}

fn spawn_request<F>(
    handle: &Handle,
    request_ptr: *mut RequestState,
    future: F,
) -> tokio::task::JoinHandle<()>
where
    F: std::future::Future<Output = FfiResult<RequestOutput>> + Send + 'static,
{
    let request_addr = request_ptr as usize;

    handle.spawn(async move {
        let result = AssertUnwindSafe(future).catch_unwind().await;
        let request_ptr = request_addr as *mut RequestState;
        let (status, output, error_message) = match result {
            Ok(Ok(output)) => (tikv_status_t::TIKV_STATUS_OK, Some(output), None),
            Ok(Err(error)) => (error.status, None, Some(error.message)),
            Err(payload) => (
                tikv_status_t::TIKV_STATUS_PANIC,
                None,
                Some(panic_message(payload)),
            ),
        };

        finalize_request(request_ptr, status, output, error_message);
    })
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

    let future = build_future(handle.core.client.clone())?;
    let request_ptr = make_request_state(kind, &handle.core.lifecycle, completion, userdata);

    if !handle.core.lifecycle.accepting.load(Ordering::Acquire) {
        inflight_done(&handle.core.lifecycle);
        release_request(request_ptr);
        return Err(FfiError::rejected(
            "client is shutting down and rejects new requests",
        ));
    }

    retain_request(request_ptr);
    unsafe {
        *out_request = request_ptr as *mut tikv_async_request_t;
    }
    spawn_request(handle.core.runtime.handle(), request_ptr, future);

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn lifecycle() -> Arc<ClientLifecycle> {
        Arc::new(ClientLifecycle {
            accepting: AtomicBool::new(true),
            drain: Mutex::new(DrainState { inflight: 0 }),
            drain_ready: Condvar::new(),
        })
    }

    struct CompletionReport {
        status: tikv_status_t,
        status_access: tikv_status_t,
        queried_status: tikv_status_t,
        message: Option<String>,
    }

    extern "C" fn destroy_in_callback(
        request: *mut tikv_async_request_t,
        status: tikv_status_t,
        userdata: *mut c_void,
    ) {
        let sender = unsafe { &*(userdata as *const mpsc::Sender<CompletionReport>) };
        let mut queried_status = tikv_status_t::TIKV_STATUS_ERROR;
        let status_access =
            tikv_async_request_completion_status(request, &mut queried_status, ptr::null_mut());
        let message = tikv_async_request_error_message(request);
        let message = if message.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(message) }
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        tikv_async_request_destroy(request);
        let _ = sender.send(CompletionReport {
            status,
            status_access,
            queried_status,
            message,
        });
    }

    #[test]
    fn accepted_requests_complete_once_and_allow_destruction_in_callback() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let lifecycle = lifecycle();
        let (sender, receiver) = mpsc::channel::<CompletionReport>();
        let outcomes = [
            (
                boxed_future(async { Ok(RequestOutput::Unit) }),
                tikv_status_t::TIKV_STATUS_OK,
                None,
            ),
            (
                boxed_future(async { Err(FfiError::error("operation failed")) }),
                tikv_status_t::TIKV_STATUS_ERROR,
                Some("operation failed"),
            ),
            (
                boxed_future(async { panic!("operation panicked") }),
                tikv_status_t::TIKV_STATUS_PANIC,
                Some("panic across FFI boundary: operation panicked"),
            ),
        ];
        for (future, expected_status, expected_message) in outcomes {
            let request = make_request_state(
                RequestKind::Put,
                &lifecycle,
                destroy_in_callback,
                &sender as *const _ as *mut c_void,
            );
            retain_request(request);
            let task = spawn_request(runtime.handle(), request, future);
            let report = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
            wait_drain(&lifecycle);
            runtime.block_on(task).unwrap();
            assert_eq!(report.status, expected_status);
            assert_eq!(report.status_access, tikv_status_t::TIKV_STATUS_OK);
            assert_eq!(report.queried_status, expected_status);
            assert_eq!(report.message.as_deref(), expected_message);
            assert!(matches!(
                receiver.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            assert_eq!(Arc::strong_count(&lifecycle), 1);
        }
    }

    struct BlockingCallback {
        entered: mpsc::Sender<tikv_status_t>,
        release: Mutex<mpsc::Receiver<()>>,
    }

    extern "C" fn blocking_callback(
        request: *mut tikv_async_request_t,
        status: tikv_status_t,
        userdata: *mut c_void,
    ) {
        let callback = unsafe { &*(userdata as *const BlockingCallback) };
        let _ = callback.entered.send(status);
        let _ = callback.release.lock().unwrap().recv();
        tikv_async_request_destroy(request);
    }

    #[test]
    fn shutdown_drain_waits_for_pending_operation_and_callback() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let lifecycle = lifecycle();
        let (entered_sender, entered_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let callback = BlockingCallback {
            entered: entered_sender,
            release: Mutex::new(release_receiver),
        };
        let (finish_sender, finish_receiver) = futures::channel::oneshot::channel();
        let request = make_request_state(
            RequestKind::Put,
            &lifecycle,
            blocking_callback,
            &callback as *const _ as *mut c_void,
        );
        retain_request(request);
        let task = spawn_request(runtime.handle(), request, async move {
            finish_receiver.await.unwrap();
            Ok(RequestOutput::Unit)
        });
        lifecycle.accepting.store(false, Ordering::Release);
        let (drain_started_sender, drain_started_receiver) = mpsc::channel();
        let (drained_sender, drained_receiver) = mpsc::channel();
        let drain_lifecycle = Arc::clone(&lifecycle);
        let drain_thread = std::thread::spawn(move || {
            let _ = drain_started_sender.send(());
            wait_drain(&drain_lifecycle);
            let _ = drained_sender.send(());
        });
        drain_started_receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        let before_completion = drained_receiver.recv_timeout(Duration::from_millis(50));
        finish_sender.send(()).unwrap();
        let callback_status = entered_receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        let during_callback = drained_receiver.recv_timeout(Duration::from_millis(50));
        release_sender.send(()).unwrap();
        drained_receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        drain_thread.join().unwrap();
        runtime.block_on(task).unwrap();
        assert_eq!(before_completion, Err(mpsc::RecvTimeoutError::Timeout));
        assert_eq!(during_callback, Err(mpsc::RecvTimeoutError::Timeout));
        assert_eq!(callback_status, tikv_status_t::TIKV_STATUS_OK);
        assert_eq!(Arc::strong_count(&lifecycle), 1);
    }
}
