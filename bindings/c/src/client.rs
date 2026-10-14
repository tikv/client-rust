// Copyright (c) 2026 PingCAP, Inc.

use super::*;

pub(crate) fn parse_pd_endpoints(pd_endpoints_csv: *const c_char) -> FfiResult<Vec<String>> {
    if pd_endpoints_csv.is_null() {
        return Err(FfiError::invalid_argument(
            "pd_endpoints_csv must not be null",
        ));
    }

    let csv = unsafe { CStr::from_ptr(pd_endpoints_csv) };
    let csv = csv
        .to_str()
        .map_err(|_| FfiError::invalid_argument("pd_endpoints_csv must be valid UTF-8"))?;

    let endpoints: Vec<String> = csv
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if endpoints.is_empty() {
        return Err(FfiError::invalid_argument(
            "pd_endpoints_csv must contain at least one endpoint",
        ));
    }

    Ok(endpoints)
}

pub(crate) fn build_config(
    timeout_secs: u32,
    use_default_keyspace: bool,
    grpc_max_decoding_message_size: usize,
) -> Config {
    let mut config = Config::default();
    if timeout_secs > 0 {
        config = config.with_timeout(Duration::from_secs(timeout_secs.into()));
    }
    if use_default_keyspace {
        config = config.with_default_keyspace();
    }
    if grpc_max_decoding_message_size > 0 {
        config = config.with_grpc_max_decoding_message_size(grpc_max_decoding_message_size);
    }
    config
}

fn make_runtime() -> FfiResult<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| FfiError::error(error.to_string()))
}

fn new_client_handle(
    pd_endpoints_csv: *const c_char,
    timeout_secs: u32,
    use_default_keyspace: bool,
    grpc_max_decoding_message_size: usize,
) -> FfiResult<AsyncClientHandle> {
    let endpoints = parse_pd_endpoints(pd_endpoints_csv)?;
    let config = build_config(
        timeout_secs,
        use_default_keyspace,
        grpc_max_decoding_message_size,
    );
    let runtime = make_runtime()?;
    let client = runtime
        .block_on(RawClient::new_with_config(endpoints, config))
        .map_err(|error| FfiError::error(error.to_string()))?;

    Ok(AsyncClientHandle {
        core: Arc::new(AsyncClientCore {
            runtime,
            client,
            lifecycle: ClientLifecycle {
                accepting: AtomicBool::new(true),
                cancel_inflight: AtomicBool::new(false),
                drain: Mutex::new(DrainState { inflight: 0 }),
                drain_ready: Condvar::new(),
                requests: Mutex::new(Vec::new()),
            },
        }),
    })
}
#[no_mangle]
pub extern "C" fn tikv_async_client_new(
    pd_endpoints_csv: *const c_char,
    timeout_secs: u32,
    use_default_keyspace: bool,
    out_client: *mut *mut tikv_async_client_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    tikv_async_client_new_with_grpc_max_decoding_message_size(
        pd_endpoints_csv,
        timeout_secs,
        use_default_keyspace,
        usize::MAX,
        out_client,
        out_error,
    )
}

#[no_mangle]
pub extern "C" fn tikv_async_client_new_with_grpc_max_decoding_message_size(
    pd_endpoints_csv: *const c_char,
    timeout_secs: u32,
    use_default_keyspace: bool,
    grpc_max_decoding_message_size: usize,
    out_client: *mut *mut tikv_async_client_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    if out_client.is_null() {
        clear_error_slot(out_error);
        set_error_slot(out_error, "out_client must not be null".to_owned());
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        *out_client = ptr::null_mut();
    }

    match run_ffi(out_error, || {
        let handle = new_client_handle(
            pd_endpoints_csv,
            timeout_secs,
            use_default_keyspace,
            grpc_max_decoding_message_size,
        )?;
        unsafe {
            *out_client = Box::into_raw(Box::new(handle)) as *mut tikv_async_client_t;
        }
        Ok(())
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_client_shutdown_begin(
    client: *mut tikv_async_client_t,
    cancel_inflight: bool,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    match run_ffi(out_error, || {
        let handle = client_ref(client)?;
        handle
            .core
            .lifecycle
            .accepting
            .store(false, Ordering::Release);
        if cancel_inflight {
            handle
                .core
                .lifecycle
                .cancel_inflight
                .store(true, Ordering::Release);
            let requests = handle
                .core
                .lifecycle
                .requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            for request_addr in requests {
                let request = unsafe { &*(request_addr as *const RequestState) };
                let mut completion = lock_completion(request);
                completion.cancel_requested = true;
            }
        }
        Ok(())
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_client_drain(
    client: *const tikv_async_client_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    match run_ffi(out_error, || {
        let handle = client_ref(client)?;
        wait_drain(&handle.core);
        Ok(())
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

#[no_mangle]
pub extern "C" fn tikv_async_client_destroy(client: *mut tikv_async_client_t) {
    if client.is_null() {
        return;
    }
    let handle = unsafe { Box::from_raw(client as *mut AsyncClientHandle) };
    handle
        .core
        .lifecycle
        .accepting
        .store(false, Ordering::Release);
    handle
        .core
        .lifecycle
        .cancel_inflight
        .store(true, Ordering::Release);
    let requests = handle
        .core
        .lifecycle
        .requests
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    for request_addr in requests {
        let request = unsafe { &*(request_addr as *const RequestState) };
        let mut completion = lock_completion(request);
        completion.cancel_requested = true;
    }
    wait_drain(&handle.core);
}
