// Copyright (c) 2026 PingCAP, Inc.

#![allow(non_camel_case_types)]

use std::collections::HashSet;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::Duration;

use futures::FutureExt;
use prometheus::proto::MetricType;
use tikv_client::{BoundRange, Config, Key, KvPair, RawClient};
use tokio::runtime::Handle;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_slice_t {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_slices_t {
    pub ptr: *const tikv_slice_t,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_async_raw_mutation_t {
    pub op: tikv_async_raw_mutation_op_t,
    pub key: tikv_slice_t,
    pub value: tikv_slice_t,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_async_raw_mutations_t {
    pub ptr: *const tikv_async_raw_mutation_t,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_async_raw_mutation_with_ttl_t {
    pub op: tikv_async_raw_mutation_op_t,
    pub key: tikv_slice_t,
    pub value: tikv_slice_t,
    pub ttl_secs: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_async_raw_mutations_with_ttl_t {
    pub ptr: *const tikv_async_raw_mutation_with_ttl_t,
    pub len: usize,
}

pub type tikv_async_raw_mutation_op_t = c_int;

const TIKV_ASYNC_RAW_MUTATION_OP_PUT: tikv_async_raw_mutation_op_t = 0;
const TIKV_ASYNC_RAW_MUTATION_OP_DELETE: tikv_async_raw_mutation_op_t = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_owned_bytes_t {
    pub ptr: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_kv_pair_t {
    pub key: tikv_owned_bytes_t,
    pub value: tikv_owned_bytes_t,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_kv_pairs_t {
    pub ptr: *mut tikv_kv_pair_t,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum tikv_status_t {
    TIKV_STATUS_OK = 0,
    TIKV_STATUS_ERROR = 1,
    TIKV_STATUS_INVALID_ARGUMENT = 2,
    TIKV_STATUS_PANIC = 3,
    TIKV_STATUS_REJECTED = 4,
    TIKV_STATUS_CANCELLED = 5,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum tikv_metric_sample_kind_t {
    TIKV_METRIC_SAMPLE_UNKNOWN = 0,
    TIKV_METRIC_SAMPLE_COUNTER = 1,
    TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET = 2,
    TIKV_METRIC_SAMPLE_HISTOGRAM_SUM = 3,
    TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT = 4,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_metric_sample_t {
    pub name: *const c_char,
    pub type_label: *const c_char,
    pub kind: tikv_metric_sample_kind_t,
    pub value: f64,
    pub le: f64,
}

pub type tikv_async_completion_cb = extern "C" fn(
    request: *mut tikv_async_request_t,
    final_status: tikv_status_t,
    userdata: *mut c_void,
);

pub struct tikv_error_t {
    message: CString,
}

#[repr(C)]
pub struct tikv_async_client_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct tikv_async_request_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct tikv_metrics_snapshot_t {
    _private: [u8; 0],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestKind {
    Put,
    PutWithTtl,
    Get,
    GetKeyTtl,
    Delete,
    DeleteRange,
    Scan,
    ScanReverse,
    BatchWrite,
    BatchGet,
}

impl RequestKind {
    fn name(self) -> &'static str {
        match self {
            Self::Put => "put",
            Self::PutWithTtl => "put_with_ttl",
            Self::Get => "get",
            Self::GetKeyTtl => "get_key_ttl",
            Self::Delete => "delete",
            Self::DeleteRange => "delete_range",
            Self::Scan => "scan",
            Self::ScanReverse => "scan_reverse",
            Self::BatchWrite => "batch_write",
            Self::BatchGet => "batch_get",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MutationOp {
    Put,
    Delete,
}

impl MutationOp {
    fn from_ffi(raw: tikv_async_raw_mutation_op_t, name: &str) -> FfiResult<Self> {
        match raw {
            TIKV_ASYNC_RAW_MUTATION_OP_PUT => Ok(Self::Put),
            TIKV_ASYNC_RAW_MUTATION_OP_DELETE => Ok(Self::Delete),
            _ => Err(FfiError::invalid_argument(format!(
                "{name}.op has invalid value {raw}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OwnedMutation {
    Put(KvPair, u64),
    Delete(Key),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WriteBatchRun {
    Put(Vec<(KvPair, u64)>),
    Delete(Vec<Key>),
}

#[derive(Debug)]
enum RequestOutput {
    Unit,
    Get(Option<Vec<u8>>),
    KeyTtl(Option<u64>),
    Pairs(Vec<KvPair>),
}

#[derive(Clone, Debug)]
struct FfiError {
    status: tikv_status_t,
    message: String,
}

type FfiResult<T> = Result<T, FfiError>;
type PinBoxRequestFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = FfiResult<RequestOutput>> + Send + 'static>>;

impl FfiError {
    fn error(message: impl Into<String>) -> Self {
        Self {
            status: tikv_status_t::TIKV_STATUS_ERROR,
            message: message.into(),
        }
    }

    fn invalid_argument(message: impl Into<String>) -> Self {
        Self {
            status: tikv_status_t::TIKV_STATUS_INVALID_ARGUMENT,
            message: message.into(),
        }
    }

    fn rejected(message: impl Into<String>) -> Self {
        Self {
            status: tikv_status_t::TIKV_STATUS_REJECTED,
            message: message.into(),
        }
    }
}

struct DrainState {
    inflight: usize,
}

struct ClientLifecycle {
    accepting: AtomicBool,
    cancel_inflight: AtomicBool,
    drain: Mutex<DrainState>,
    drain_ready: Condvar,
    requests: Mutex<Vec<usize>>,
}

struct AsyncClientCore {
    runtime: tokio::runtime::Runtime,
    client: RawClient,
    lifecycle: ClientLifecycle,
}

struct AsyncClientHandle {
    core: Arc<AsyncClientCore>,
}

struct CompletionState {
    complete: bool,
    callback_running: bool,
    callback_done: bool,
    completion_status: Option<tikv_status_t>,
    error_message: Option<CString>,
    output: Option<RequestOutput>,
    output_taken: bool,
    cancel_requested: bool,
}

struct RequestState {
    kind: RequestKind,
    completion: Mutex<CompletionState>,
    ready: Condvar,
    client: Weak<AsyncClientCore>,
    callback: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
    retain_count: AtomicUsize,
}

unsafe impl Send for RequestState {}
unsafe impl Sync for RequestState {}

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

fn clear_error_slot(out_error: *mut *mut tikv_error_t) {
    if out_error.is_null() {
        return;
    }

    unsafe {
        *out_error = ptr::null_mut();
    }
}

fn set_error_slot(out_error: *mut *mut tikv_error_t, message: String) {
    if out_error.is_null() {
        return;
    }

    let sanitized = message.replace('\0', " ");
    let c_message = CString::new(sanitized).unwrap_or_else(|_| {
        CString::new("failed to build FFI error message").expect("static string")
    });
    let error = Box::new(tikv_error_t { message: c_message });
    unsafe {
        *out_error = Box::into_raw(error);
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return format!("panic across FFI boundary: {message}");
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return format!("panic across FFI boundary: {message}");
    }
    "panic across FFI boundary".to_owned()
}

fn run_ffi<T, F>(out_error: *mut *mut tikv_error_t, f: F) -> Result<T, tikv_status_t>
where
    F: FnOnce() -> FfiResult<T>,
{
    clear_error_slot(out_error);

    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            set_error_slot(out_error, error.message);
            Err(error.status)
        }
        Err(payload) => {
            set_error_slot(out_error, panic_message(payload));
            Err(tikv_status_t::TIKV_STATUS_PANIC)
        }
    }
}

fn empty_owned_bytes() -> tikv_owned_bytes_t {
    tikv_owned_bytes_t {
        ptr: ptr::null_mut(),
        len: 0,
    }
}

fn empty_kv_pairs() -> tikv_kv_pairs_t {
    tikv_kv_pairs_t {
        ptr: ptr::null_mut(),
        len: 0,
    }
}

fn boxed_future<F>(future: F) -> PinBoxRequestFuture
where
    F: std::future::Future<Output = FfiResult<RequestOutput>> + Send + 'static,
{
    Box::pin(future)
}

fn lock_completion(request: &RequestState) -> std::sync::MutexGuard<'_, CompletionState> {
    request
        .completion
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn request_ptr(raw: *mut tikv_async_request_t) -> *mut RequestState {
    raw as *mut RequestState
}

fn request_ref<'a>(raw: *const tikv_async_request_t) -> FfiResult<&'a RequestState> {
    if raw.is_null() {
        return Err(FfiError::invalid_argument("request must not be null"));
    }
    Ok(unsafe { &*(raw as *const RequestState) })
}

fn client_ref<'a>(raw: *const tikv_async_client_t) -> FfiResult<&'a AsyncClientHandle> {
    if raw.is_null() {
        return Err(FfiError::invalid_argument("client must not be null"));
    }
    Ok(unsafe { &*(raw as *const AsyncClientHandle) })
}

fn retain_request(request: *mut RequestState) {
    unsafe {
        (*request).retain_count.fetch_add(1, Ordering::AcqRel);
    }
}

fn release_request(request: *mut RequestState) {
    let previous = unsafe { (*request).retain_count.fetch_sub(1, Ordering::AcqRel) };
    if previous == 1 {
        unsafe {
            drop(Box::from_raw(request));
        }
    }
}

fn inflight_add(core: &AsyncClientCore) {
    let mut drain = core
        .lifecycle
        .drain
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    drain.inflight += 1;
}

fn inflight_done(core: &AsyncClientCore) {
    let mut drain = core
        .lifecycle
        .drain
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    drain.inflight -= 1;
    if drain.inflight == 0 {
        core.lifecycle.drain_ready.notify_all();
    }
}

fn wait_drain(core: &AsyncClientCore) {
    let mut drain = core
        .lifecycle
        .drain
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while drain.inflight != 0 {
        drain = core
            .lifecycle
            .drain_ready
            .wait(drain)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn bytes_to_owned(bytes: Vec<u8>) -> tikv_owned_bytes_t {
    if bytes.is_empty() {
        return empty_owned_bytes();
    }

    let mut boxed = bytes.into_boxed_slice();
    let owned = tikv_owned_bytes_t {
        ptr: boxed.as_mut_ptr(),
        len: boxed.len(),
    };
    mem::forget(boxed);
    owned
}

fn pairs_to_owned(pairs: Vec<KvPair>) -> tikv_kv_pairs_t {
    if pairs.is_empty() {
        return empty_kv_pairs();
    }

    let ffi_pairs: Vec<tikv_kv_pair_t> = pairs
        .into_iter()
        .map(|pair| {
            let (key, value): (Key, Vec<u8>) = pair.into();
            tikv_kv_pair_t {
                key: bytes_to_owned(key.into()),
                value: bytes_to_owned(value),
            }
        })
        .collect();
    let mut ffi_pairs = ffi_pairs.into_boxed_slice();
    let owned = tikv_kv_pairs_t {
        ptr: ffi_pairs.as_mut_ptr(),
        len: ffi_pairs.len(),
    };
    mem::forget(ffi_pairs);
    owned
}

fn slice_is_empty(raw: tikv_slice_t) -> bool {
    raw.len == 0
}

fn slice_to_vec(raw: tikv_slice_t, name: &str) -> FfiResult<Vec<u8>> {
    if raw.len == 0 {
        return Ok(Vec::new());
    }
    if raw.ptr.is_null() {
        return Err(FfiError::invalid_argument(format!(
            "{name} pointer is null while length is {}",
            raw.len
        )));
    }

    let bytes = unsafe { slice::from_raw_parts(raw.ptr, raw.len) };
    Ok(bytes.to_vec())
}

fn slices_to_vec<'a, T>(ptr: *const T, len: usize, name: &str) -> FfiResult<&'a [T]> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(FfiError::invalid_argument(format!(
            "{name} pointer is null while length is {len}"
        )));
    }

    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

fn slice_to_key(raw: tikv_slice_t, name: &str) -> FfiResult<Key> {
    Ok(Key::from(slice_to_vec(raw, name)?))
}

fn slices_to_keys(raw: tikv_slices_t, name: &str) -> FfiResult<Vec<Key>> {
    let slices = slices_to_vec(raw.ptr, raw.len, name)?;
    slices
        .iter()
        .enumerate()
        .map(|(index, item)| slice_to_key(*item, &format!("{name}[{index}]")))
        .collect()
}

fn slice_mutations_to_owned(raw: tikv_async_raw_mutations_t) -> FfiResult<Vec<OwnedMutation>> {
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

fn slice_mutations_with_ttl_to_owned(
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

fn build_write_batch_runs(mutations: Vec<OwnedMutation>) -> Vec<WriteBatchRun> {
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

fn parse_pd_endpoints(pd_endpoints_csv: *const c_char) -> FfiResult<Vec<String>> {
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

fn build_config(
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

fn submit_common<F>(
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
pub extern "C" fn tikv_error_message(error: *const tikv_error_t) -> *const c_char {
    if error.is_null() {
        return ptr::null();
    }
    unsafe { (*error).message.as_ptr() }
}

#[no_mangle]
pub extern "C" fn tikv_error_destroy(error: *mut tikv_error_t) {
    if error.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(error));
    }
}

#[no_mangle]
pub extern "C" fn tikv_owned_bytes_destroy(bytes: tikv_owned_bytes_t) {
    if bytes.ptr.is_null() || bytes.len == 0 {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(bytes.ptr, bytes.len, bytes.len));
    }
}

#[no_mangle]
pub extern "C" fn tikv_kv_pairs_destroy(pairs: tikv_kv_pairs_t) {
    if pairs.ptr.is_null() || pairs.len == 0 {
        return;
    }
    unsafe {
        let pairs = Vec::from_raw_parts(pairs.ptr, pairs.len, pairs.len);
        for pair in pairs {
            tikv_owned_bytes_destroy(pair.key);
            tikv_owned_bytes_destroy(pair.value);
        }
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

#[no_mangle]
pub extern "C" fn tikv_async_request_destroy(request: *mut tikv_async_request_t) {
    if request.is_null() {
        return;
    }
    release_request(request_ptr(request));
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let key =
            unsafe { slice::from_raw_parts((*out_pairs.ptr).key.ptr, (*out_pairs.ptr).key.len) };
        let value = unsafe {
            slice::from_raw_parts((*out_pairs.ptr).value.ptr, (*out_pairs.ptr).value.len)
        };
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
}
