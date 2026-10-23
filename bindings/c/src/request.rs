// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

use super::*;

fn ensure_accepting(core: &AsyncClientCore) -> FfiResult<()> {
    if !core.lifecycle.accepting.load(Ordering::Acquire) {
        return Err(FfiError::rejected(
            "client is shutting down and rejects new requests",
        ));
    }
    Ok(())
}

fn make_request_state(
    kind: RequestKind,
    lifecycle: &Arc<ClientLifecycle>,
    callback: tikv_async_completion_cb,
    userdata: *mut c_void,
) -> (Arc<RequestState>, InflightGuard) {
    let inflight = InflightGuard::new(lifecycle);
    let request = Arc::new(RequestState {
        kind,
        completion: RefCell::new(CompletionState {
            complete: false,
            completion_status: None,
            error_message: None,
            output: None,
            output_taken: false,
        }),
        callback: Some(callback),
        userdata,
    });
    (request, inflight)
}

fn finalize_request(
    request: Arc<RequestState>,
    status: tikv_status_t,
    output: Option<RequestOutput>,
    error_message: Option<String>,
) {
    let (callback, userdata) = {
        let mut completion = request.completion.borrow_mut();
        completion.complete = true;
        completion.completion_status = Some(status);
        completion.output = output;
        completion.error_message =
            error_message.and_then(|message| CString::new(message.replace('\0', " ")).ok());
        (request.callback, request.userdata)
    };

    if let Some(callback) = callback {
        callback(
            Arc::as_ptr(&request) as *mut tikv_async_request_t,
            status,
            userdata,
        );
    }
}

fn spawn_request<F>(
    handle: &Handle,
    request: Arc<RequestState>,
    inflight: InflightGuard,
    future: F,
) -> tokio::task::JoinHandle<()>
where
    F: std::future::Future<Output = FfiResult<RequestOutput>> + Send + 'static,
{
    handle.spawn(async move {
        // Keep the inflight registration until finalize_request returns, so
        // client drain waits for the callback but not for a retained C handle.
        let _inflight = inflight;
        let result = AssertUnwindSafe(future).catch_unwind().await;
        let (status, output, error_message) = match result {
            Ok(Ok(output)) => (tikv_status_t::TIKV_STATUS_OK, Some(output), None),
            Ok(Err(error)) => (error.status, None, Some(error.message)),
            Err(payload) => (
                tikv_status_t::TIKV_STATUS_PANIC,
                None,
                Some(panic_message(payload)),
            ),
        };

        finalize_request(request, status, output, error_message);
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
    let (request, inflight) =
        make_request_state(kind, &handle.core.lifecycle, completion, userdata);

    if !handle.core.lifecycle.accepting.load(Ordering::Acquire) {
        return Err(FfiError::rejected(
            "client is shutting down and rejects new requests",
        ));
    }

    // Transfer one strong reference to C. The task retains the other reference
    // through callback completion; C releases its reference via destroy.
    unsafe {
        *out_request = Arc::into_raw(Arc::clone(&request)) as *mut tikv_async_request_t;
    }
    spawn_request(handle.core.runtime.handle(), request, inflight, future);

    Ok(())
}

fn take_request_output<'a>(
    request: &'a RequestState,
    expected: RequestKind,
) -> FfiResult<RefMut<'a, CompletionState>> {
    if request.kind != expected {
        return Err(FfiError::invalid_argument(format!(
            "request does not carry a {} result",
            expected.name()
        )));
    }
    let completion = request.completion.borrow_mut();
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
pub extern "C" fn tikv_async_request_error_message(
    request: *const tikv_async_request_t,
) -> *const c_char {
    let Ok(request) = request_ref(request) else {
        return ptr::null();
    };
    let completion = request.completion.borrow();
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
    // SAFETY: each accepted request exports exactly one Arc with into_raw.
    // The C contract requires releasing that reference exactly once.
    unsafe {
        drop(Arc::from_raw(request as *const RequestState));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn lifecycle() -> Arc<ClientLifecycle> {
        Arc::new(ClientLifecycle {
            accepting: AtomicBool::new(true),
            inflight: Mutex::new(0),
            drain_ready: Condvar::new(),
        })
    }

    struct CompletionReport {
        request_addr: usize,
        status: tikv_status_t,
        message: Option<String>,
    }

    extern "C" fn destroy_in_callback(
        request: *mut tikv_async_request_t,
        status: tikv_status_t,
        userdata: *mut c_void,
    ) {
        let sender = unsafe { &*(userdata as *const mpsc::Sender<CompletionReport>) };
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
            request_addr: request as usize,
            status,
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
            let (request, inflight) = make_request_state(
                RequestKind::Put,
                &lifecycle,
                destroy_in_callback,
                &sender as *const _ as *mut c_void,
            );
            let weak_request = Arc::downgrade(&request);
            let raw_request = Arc::into_raw(Arc::clone(&request)) as *mut tikv_async_request_t;
            let task = spawn_request(runtime.handle(), request, inflight, future);
            let report = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
            runtime.block_on(task).unwrap();
            assert_eq!(report.request_addr, raw_request as usize);
            assert_eq!(report.status, expected_status);
            assert_eq!(report.message.as_deref(), expected_message);
            assert!(matches!(
                receiver.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            assert_eq!(Arc::strong_count(&lifecycle), 1);
            assert!(weak_request.upgrade().is_none());
        }
    }

    struct BlockingCallback {
        entered: mpsc::Sender<tikv_status_t>,
        release: Mutex<mpsc::Receiver<()>>,
    }

    extern "C" fn blocking_callback(
        _request: *mut tikv_async_request_t,
        status: tikv_status_t,
        userdata: *mut c_void,
    ) {
        let callback = unsafe { &*(userdata as *const BlockingCallback) };
        let _ = callback.entered.send(status);
        let _ = callback.release.lock().unwrap().recv();
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
        let (request, inflight) = make_request_state(
            RequestKind::Put,
            &lifecycle,
            blocking_callback,
            &callback as *const _ as *mut c_void,
        );
        let weak_request = Arc::downgrade(&request);
        let raw_request = Arc::into_raw(Arc::clone(&request));
        let task = spawn_request(runtime.handle(), request, inflight, async move {
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
        let alive_during_callback = weak_request.upgrade().is_some();
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
        assert!(alive_during_callback);
        assert_eq!(Arc::strong_count(&lifecycle), 1);
        // The retained C handle keeps the request alive, but no longer keeps
        // the client's inflight registration after the callback returns.
        assert!(weak_request.upgrade().is_some());
        tikv_async_request_destroy(raw_request as *mut tikv_async_request_t);
        assert!(weak_request.upgrade().is_none());
    }

    extern "C" fn report_completion(
        _request: *mut tikv_async_request_t,
        status: tikv_status_t,
        userdata: *mut c_void,
    ) {
        let sender = unsafe { &*(userdata as *const mpsc::Sender<tikv_status_t>) };
        let _ = sender.send(status);
    }

    extern "C" fn handoff_completion(
        _request: *mut tikv_async_request_t,
        status: tikv_status_t,
        userdata: *mut c_void,
    ) {
        let callback = unsafe { &*(userdata as *const BlockingCallback) };
        // Hand request access to the receiver, then remain in the callback
        // without touching the request again.
        let _ = callback.entered.send(status);
        let _ = callback.release.lock().unwrap().recv();
    }

    #[test]
    fn result_can_be_taken_after_handoff_before_callback_returns() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let lifecycle = lifecycle();
        let (entered_sender, entered_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let callback = BlockingCallback {
            entered: entered_sender,
            release: Mutex::new(release_receiver),
        };
        let (request, inflight) = make_request_state(
            RequestKind::Get,
            &lifecycle,
            handoff_completion,
            &callback as *const _ as *mut c_void,
        );
        let weak_request = Arc::downgrade(&request);
        let raw_request = Arc::into_raw(Arc::clone(&request)) as *mut tikv_async_request_t;
        let task = spawn_request(runtime.handle(), request, inflight, async {
            Ok(RequestOutput::Get(Some(b"value".to_vec())))
        });
        let status = entered_receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap();

        let mut error = ptr::null_mut();
        let mut out_value = empty_owned_bytes();
        let mut out_found = false;
        let result_status = tikv_async_request_take_get_result(
            raw_request,
            &mut out_value,
            &mut out_found,
            &mut error,
        );
        let no_error = error.is_null();
        let found = out_found;
        let value = if out_value.ptr.is_null() {
            Vec::new()
        } else {
            unsafe { slice::from_raw_parts(out_value.ptr, out_value.len) }.to_vec()
        };
        tikv_owned_bytes_destroy(out_value);
        tikv_error_destroy(error);
        let second_status = tikv_async_request_take_get_result(
            raw_request,
            &mut out_value,
            &mut out_found,
            &mut error,
        );
        tikv_error_destroy(error);
        tikv_async_request_destroy(raw_request);
        let alive_during_callback = weak_request.upgrade().is_some();

        // Release the callback before asserting so a failed assertion cannot
        // leave a runtime worker blocked during runtime destruction.
        release_sender.send(()).unwrap();
        runtime.block_on(task).unwrap();
        assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
        assert_eq!(result_status, tikv_status_t::TIKV_STATUS_OK);
        assert!(no_error);
        assert!(found);
        assert_eq!(value, b"value");
        assert_eq!(second_status, tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT);
        assert!(alive_during_callback);
        assert!(weak_request.upgrade().is_none());
    }

    #[test]
    fn c_handle_keeps_result_alive_after_task_and_runtime_finish() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let lifecycle = lifecycle();
        let weak_lifecycle = Arc::downgrade(&lifecycle);
        let (sender, receiver) = mpsc::channel::<tikv_status_t>();
        let (request, inflight) = make_request_state(
            RequestKind::Get,
            &lifecycle,
            report_completion,
            &sender as *const _ as *mut c_void,
        );
        let weak_request = Arc::downgrade(&request);
        let raw_request = Arc::into_raw(Arc::clone(&request)) as *mut tikv_async_request_t;
        let task = spawn_request(runtime.handle(), request, inflight, async {
            Ok(RequestOutput::Get(Some(b"value".to_vec())))
        });
        let status = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        runtime.block_on(task).unwrap();
        drop(runtime);
        drop(lifecycle);

        assert_eq!(status, tikv_status_t::TIKV_STATUS_OK);
        assert!(weak_request.upgrade().is_some());
        let mut error = ptr::null_mut();
        let mut out_value = empty_owned_bytes();
        let mut out_found = false;
        let result_status = tikv_async_request_take_get_result(
            raw_request,
            &mut out_value,
            &mut out_found,
            &mut error,
        );
        assert_eq!(result_status, tikv_status_t::TIKV_STATUS_OK);
        assert!(error.is_null());
        assert!(out_found);
        assert_eq!(
            unsafe { slice::from_raw_parts(out_value.ptr, out_value.len) },
            b"value"
        );

        tikv_async_request_destroy(raw_request);
        assert!(weak_request.upgrade().is_none());
        assert!(weak_lifecycle.upgrade().is_none());
        // Extracted buffers have their own ownership, independent of the request.
        assert_eq!(
            unsafe { slice::from_raw_parts(out_value.ptr, out_value.len) },
            b"value"
        );
        tikv_owned_bytes_destroy(out_value);
    }
}
