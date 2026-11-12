// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

//! Asynchronous TiKV C ABI.
//!
//! # Submission safety contract
//!
//! A non-null client handle must be live and must not be destroyed during a
//! submit call. Input slices and arrays must be aligned and readable for their
//! declared lengths within one allocation, with sizes no larger than `isize::MAX`.
//! Input memory must not be mutated during submission; it is copied before the
//! call returns. Unused DELETE value slices are ignored.
//!
//! Non-null output slots must be aligned, writable, and exclusively accessible
//! for the call, and must not overlap each other, input memory, or handle state.
//! Null arguments are accepted only as documented in the C headers or rejected
//! as invalid.
//!
//! Completion callbacks must not unwind. Their userdata must remain valid and
//! usable on runtime worker threads until the callback returns. Request access
//! and destruction must be serialized, in the callback or after synchronizing
//! with it; a request handle must be destroyed exactly once. A callback may run
//! and destroy the handle before submission returns.
//! Client creation and destruction return `TIKV_STATUS_INVALID_ARGUMENT` in a
//! Tokio context, including completion callbacks. Rejected destruction leaves
//! the client unchanged. Defer these operations to an application thread;
//! callbacks must not wait for that thread to drain their own client.

#![allow(non_camel_case_types)]
#![deny(unsafe_op_in_unsafe_fn)]

mod client;
mod metrics;
mod raw;
mod request;
#[cfg(test)]
mod tests;

pub use client::*;
pub use metrics::*;
pub use raw::*;
pub use request::*;

use std::cell::{RefCell, RefMut};
use std::collections::HashSet;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
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
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum tikv_metric_sample_kind_t {
    TIKV_METRIC_SAMPLE_UNKNOWN = 0,
    TIKV_METRIC_SAMPLE_COUNTER = 1,
    TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET = 2,
    TIKV_METRIC_SAMPLE_HISTOGRAM_SUM = 3,
    TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT = 4,
    TIKV_METRIC_SAMPLE_GAUGE = 5,
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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tikv_metric_samples_t {
    pub ptr: *const tikv_metric_sample_t,
    pub len: usize,
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
    Get,
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
            Self::Get => "get",
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
    Put(KvPair),
    Delete(Key),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WriteBatchRun {
    Put(Vec<KvPair>),
    Delete(Vec<Key>),
}

#[derive(Debug)]
enum RequestOutput {
    /// Successful completion with no return data, analogous to Rust's unit type `()`.
    /// Used by put, delete, delete-range, and batch-write requests; the completion
    /// callback reports their status, so no result extraction accessor is needed.
    Unit,
    Get(Option<Vec<u8>>),
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

struct ClientLifecycle {
    accepting: AtomicBool,
    inflight: Mutex<usize>,
    drain_ready: Condvar,
    metrics: Arc<metrics::BindingMetrics>,
}

/// Keeps a client operation counted until its callback has returned.
///
/// This is separate from `RequestState` because C may retain a completed
/// request handle after the callback, while client shutdown only waits for
/// callbacks that are still running.
struct InflightGuard {
    lifecycle: Arc<ClientLifecycle>,
    // Drain registration precedes the final shutdown check. Only accepted
    // submissions also enter the public metrics.
    accepted: bool,
}

impl InflightGuard {
    fn new(lifecycle: &Arc<ClientLifecycle>) -> Self {
        inflight_add(lifecycle);
        Self {
            lifecycle: Arc::clone(lifecycle),
            accepted: false,
        }
    }

    fn accept(&mut self) {
        self.lifecycle.metrics.accept();
        self.accepted = true;
    }
}

impl Drop for InflightGuard {
    /// Release the registration after the callback returns, or undo it when
    /// shutdown rejects a submission. During shutdown, reaching zero wakes
    /// `wait_drain` while sharing the counter mutex prevents a missed wakeup.
    fn drop(&mut self) {
        let mut inflight = self
            .lifecycle
            .inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.accepted {
            self.lifecycle.metrics.release();
        }
        *inflight -= 1;
        if *inflight == 0 && !self.lifecycle.accepting.load(Ordering::Acquire) {
            self.lifecycle.drain_ready.notify_all();
        }
    }
}

struct AsyncClientCore {
    runtime: tokio::runtime::Runtime,
    client: RawClient,
    lifecycle: Arc<ClientLifecycle>,
}

struct AsyncClientHandle {
    core: Arc<AsyncClientCore>,
}

struct CompletionState {
    complete: bool,
    completion_status: Option<tikv_status_t>,
    error_message: Option<CString>,
    output: Option<RequestOutput>,
    output_taken: bool,
}

struct RequestState {
    kind: RequestKind,
    completion: RefCell<CompletionState>,
    callback: Option<tikv_async_completion_cb>,
    userdata: *mut c_void,
}

// SAFETY: the C contract requires callback userdata to be usable on a runtime
// worker thread.
unsafe impl Send for RequestState {}
// SAFETY: the task finishes writing completion and releases its borrow before
// invoking the callback. The C contract requires synchronization with that
// callback before access on another thread, and serializes all accessors and
// destruction for the same request. No completion borrows overlap across threads.
unsafe impl Sync for RequestState {}

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

/// Register work that client destruction must wait for before dropping its
/// runtime. Submitters and runtime workers update this per-client count
/// concurrently, so the mutex prevents lost increments/decrements. Increment
/// before spawning: even an immediately completed task must already be counted.
fn inflight_add(lifecycle: &ClientLifecycle) {
    let mut inflight = lifecycle
        .inflight
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *inflight += 1;
}

fn wait_drain(lifecycle: &ClientLifecycle) {
    let mut inflight = lifecycle
        .inflight
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while *inflight != 0 {
        inflight = lifecycle
            .drain_ready
            .wait(inflight)
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

/// # Safety
/// A non-null `error` must be a live error from this library. It must not be
/// destroyed while this function or the returned borrowed string is in use.
#[no_mangle]
pub unsafe extern "C" fn tikv_error_message(error: *const tikv_error_t) -> *const c_char {
    if error.is_null() {
        return ptr::null();
    }
    unsafe { (*error).message.as_ptr() }
}

/// # Safety
/// A non-null `error` must be a live error from this library, released exactly
/// once with no concurrent access or subsequent use of its borrowed message.
#[no_mangle]
pub unsafe extern "C" fn tikv_error_destroy(error: *mut tikv_error_t) {
    if error.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(error));
    }
}

/// # Safety
/// `bytes` must be empty or an unchanged buffer returned by this library. Its
/// allocation must be released exactly once, after all accesses have finished.
#[no_mangle]
pub unsafe extern "C" fn tikv_owned_bytes_destroy(bytes: tikv_owned_bytes_t) {
    if bytes.ptr.is_null() || bytes.len == 0 {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(bytes.ptr, bytes.len, bytes.len));
    }
}

/// # Safety
/// `pairs` must be empty or an unchanged array returned by this library. The
/// array and its nested buffers must still be owned by the caller and must be
/// released exactly once, after all accesses have finished.
#[no_mangle]
pub unsafe extern "C" fn tikv_kv_pairs_destroy(pairs: tikv_kv_pairs_t) {
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
