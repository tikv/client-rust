// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

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
    if Handle::try_current().is_ok() {
        return Err(FfiError::invalid_argument(
            "client creation cannot run in a Tokio context (including completion callbacks)",
        ));
    }
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
            lifecycle: Arc::new(ClientLifecycle {
                accepting: AtomicBool::new(true),
                inflight: Mutex::new(0),
                drain_ready: Condvar::new(),
                metrics: Arc::clone(metrics::binding_metrics()),
            }),
        }),
    })
}
/// # Safety
/// Non-null `pd_endpoints_csv` must point to a readable NUL-terminated string.
/// Non-null output slots must be aligned, writable, exclusively accessible,
/// and disjoint from each other and the input string for the call.
#[no_mangle]
pub unsafe extern "C" fn tikv_async_client_new(
    pd_endpoints_csv: *const c_char,
    timeout_secs: u32,
    use_default_keyspace: bool,
    out_client: *mut *mut tikv_async_client_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    // SAFETY: the caller provides the same string and output-slot guarantees
    // required by the explicit-limit constructor.
    unsafe {
        tikv_async_client_new_with_grpc_max_decoding_message_size(
            pd_endpoints_csv,
            timeout_secs,
            use_default_keyspace,
            usize::MAX,
            out_client,
            out_error,
        )
    }
}

/// # Safety
/// Non-null `pd_endpoints_csv` must point to a readable NUL-terminated string.
/// Non-null output slots must be aligned, writable, exclusively accessible,
/// and disjoint from each other and the input string for the call.
#[no_mangle]
pub unsafe extern "C" fn tikv_async_client_new_with_grpc_max_decoding_message_size(
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

/// # Safety
/// A non-null `client` must be live and must not be destroyed during this call.
/// Non-null `out_error` must point to aligned, exclusively writable storage
/// that does not overlap the client.
#[no_mangle]
pub unsafe extern "C" fn tikv_async_client_shutdown_begin(
    client: *mut tikv_async_client_t,
    out_error: *mut *mut tikv_error_t,
) -> tikv_status_t {
    match run_ffi(out_error, || {
        let handle = client_ref(client)?;
        handle
            .core
            .lifecycle
            .accepting
            .store(false, Ordering::Release);
        Ok(())
    }) {
        Ok(()) => tikv_status_t::TIKV_STATUS_OK,
        Err(status) => status,
    }
}

/// # Safety
/// A non-null `client` must be a live handle from this library. Before successful
/// destruction, all other calls using it must have stopped. Destroy it exactly
/// once successfully; `TIKV_STATUS_INVALID_ARGUMENT` leaves the handle unchanged.
#[no_mangle]
pub unsafe extern "C" fn tikv_async_client_destroy(
    client: *mut tikv_async_client_t,
) -> tikv_status_t {
    if client.is_null() {
        return tikv_status_t::TIKV_STATUS_OK;
    }
    // Check before taking ownership or beginning shutdown. This also rejects
    // destruction of a different client from a completion callback.
    if Handle::try_current().is_ok() {
        return tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT;
    }
    let handle = unsafe { Box::from_raw(client as *mut AsyncClientHandle) };
    handle
        .core
        .lifecycle
        .accepting
        .store(false, Ordering::Release);
    wait_drain(&handle.core.lifecycle);
    drop(handle);
    tikv_status_t::TIKV_STATUS_OK
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct CallbackContext {
        clients: [*mut tikv_async_client_t; 2],
        reports: mpsc::Sender<([tikv_status_t; 4], bool)>,
    }

    extern "C" fn check_client_lifecycle(
        request: *mut tikv_async_request_t,
        status: tikv_status_t,
        userdata: *mut c_void,
    ) {
        let context = unsafe { &*(userdata as *const CallbackContext) };
        let mut created = ptr::null_mut();
        // Exercise the public ABI on a real completion worker, including
        // destruction of both the callback's client and an unrelated client.
        let statuses = unsafe {
            [
                status,
                tikv_async_client_new(
                    c"127.0.0.1:1".as_ptr(),
                    1,
                    false,
                    &mut created,
                    ptr::null_mut(),
                ),
                tikv_async_client_destroy(context.clients[0]),
                tikv_async_client_destroy(context.clients[1]),
            ]
        };
        unsafe { tikv_async_request_destroy(request) };
        let _ = context.reports.send((statuses, created.is_null()));
    }

    #[test]
    fn raw_callback_rejects_client_lifecycle_calls_without_changing_clients() {
        let endpoints =
            CString::new(std::env::var("PD_ADDRS").unwrap_or_else(|_| "127.0.0.1:2379".to_owned()))
                .unwrap();
        let clients = std::array::from_fn(|_| {
            let mut client = ptr::null_mut();
            assert_eq!(
                unsafe {
                    tikv_async_client_new(endpoints.as_ptr(), 5, true, &mut client, ptr::null_mut())
                },
                tikv_status_t::TIKV_STATUS_OK,
            );
            client
        });
        let (reports, receiver) = mpsc::channel();
        let context = CallbackContext { clients, reports };
        // Reusing both handles proves rejected destruction neither consumes
        // them nor begins shutdown. Repeat on the first client to test self-use.
        for client in [clients[0], clients[0], clients[1]] {
            let mut request = ptr::null_mut();
            assert_eq!(
                unsafe {
                    tikv_async_raw_get_submit(
                        client,
                        tikv_slice_t {
                            ptr: b"lifecycle-test".as_ptr(),
                            len: 14,
                        },
                        Some(check_client_lifecycle),
                        &context as *const _ as *mut c_void,
                        &mut request,
                        ptr::null_mut(),
                    )
                },
                tikv_status_t::TIKV_STATUS_OK,
            );
            let (statuses, outputs_null) = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
            let mut expected = [tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT; 4];
            expected[0] = tikv_status_t::TIKV_STATUS_OK;
            assert_eq!(statuses, expected);
            assert!(outputs_null);
        }
        for client in clients {
            assert_eq!(
                unsafe { tikv_async_client_destroy(client) },
                tikv_status_t::TIKV_STATUS_OK,
            );
        }
    }
}
