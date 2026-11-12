# TiKV Rust Client C binding

This workspace crate exposes an asynchronous C ABI for TiKV RawKV operations.
The public headers are [`tikv_client_async_c.h`](include/tikv_client_async_c.h)
and [`tikv_client_metrics_c.h`](include/tikv_client_metrics_c.h).

## Build

From the repository root:

```sh
cargo build -p tikv-client-rust-c
```

The build produces `target/debug/libtikv_client_rust_c.a` and a shared library
(`.so` on Linux or `.dylib` on macOS). Include headers from
`bindings/c/include` and link against `tikv_client_rust_c`.

## API overview

The C ABI supports raw put, get, delete, delete-range, forward and reverse
scan, mixed put/delete batches, and batch-get. Submitting a request copies its
input buffers before returning.
An accepted request returns a handle and eventually invokes its completion
callback on a Rust runtime worker thread. The callback may read the result or
destroy the request handle. Destroy each request exactly once, either in its
callback or after synchronizing with the callback to observe completion; do not
destroy a pending request. A callback can run before submit returns, so the
returned handle must not be used if the callback already destroyed it.
Error and result accessors may be used only in the completion callback or after
synchronizing with it to observe completion. The callback's `final_status` is
the terminal request status. Serialize all accessors and destruction for the
same request, including calls from its callback. Synchronize cross-thread
ownership transfers. Different requests may be accessed concurrently.
Each returned buffer, error, and snapshot has a matching destroy function in the
headers.

`tikv_async_client_new` leaves the gRPC response decoding limit unlimited for
compatibility. Use
`tikv_async_client_new_with_grpc_max_decoding_message_size` to set a limit in
bytes. Passing zero to the latter uses the Rust client's default (currently
4 MiB).

`tikv_async_client_shutdown_begin` rejects new submissions and lets accepted
requests finish normally. `tikv_async_client_destroy` begins shutdown, waits for
all accepted callbacks to return, and releases the client. Stop other calls using
the client handle before destroying it. Destruction returns `TIKV_STATUS_OK` on
success, including for a null handle.
Client creation and destruction return `TIKV_STATUS_INVALID_ARGUMENT` in a Tokio
context, including any completion callback. Rejected creation leaves the output
client null; rejected destruction leaves the client unchanged for retry. Defer
these operations to an application thread; a callback must not wait for that
thread to drain its own client.

Reverse scans pass empty bounds through to the Rust client's keyspace encoding.
Mixed write batches preserve the order of adjacent put and delete phases, but
may issue multiple TiKV requests.

With a running cluster, the callback lifecycle regression test runs with the
workspace's `integration-tests` feature and `PD_ADDRS` setting:

```sh
cargo test -p tikv-client-rust-c --features integration-tests raw_callback
```

## Metrics snapshots

`tikv_metrics_snapshot_collect` exports the eight client-rust TiKV/PD RPC
families and three C binding families named in `tikv_client_metrics_c.h`. It
reads the process-wide Prometheus default registry, so values aggregate all
client instances. RPC values also include callers outside the C API.

The C binding families count RawKV submissions:

- `tikv_ffi_submissions_total{type="accepted"|"rejected"}` counts each submit
  once. Every non-OK return counts as rejected, including invalid arguments,
  shutdown rejection, and a panic before acceptance.
- `tikv_ffi_completions_total{type="ok"|"error"|"panic"}` counts accepted
  requests by final status after their callbacks return.
- `tikv_ffi_requests_inflight` is an unlabeled gauge of accepted requests whose
  callbacks have not returned. Retaining a completed C request handle does not
  keep it in flight.

Client creation, result access, and destruction do not count as submissions.
The FFI series are initialized on the first successful client creation or
RawKV submit call, including a rejected call. The snapshot is not an atomic
view across series when requests are being submitted or completed concurrently.

Only the expected types and labels are exported. Counters and histograms have
one `type` label; its value is in `type_label`. The gauge has `type_label = NULL`.
Histogram samples include cumulative buckets (including +Inf), sum, and count.
One call returns the snapshot handle and a read-only contiguous array. The
array and its strings remain valid until `tikv_metrics_snapshot_destroy`.
For an empty snapshot, the array view is `{NULL, 0}`.

The existing `tikv_request_total` and `pd_request_total` metrics count
underlying RPC attempts. A single C operation can issue multiple RPCs due to
retries, scans, or mixed write batches.

## C example

Build the library first, then compile the example from the repository root.

macOS:

```sh
clang -Ibindings/c/include bindings/c/examples/raw_async_demo.c \
  -Ltarget/debug -ltikv_client_rust_c \
  -Wl,-rpath,$PWD/target/debug -o target/debug/raw_async_demo
```

Linux:

```sh
cc -Ibindings/c/include bindings/c/examples/raw_async_demo.c \
  -Ltarget/debug -ltikv_client_rust_c \
  -Wl,-rpath,'$ORIGIN' -o target/debug/raw_async_demo
```

With a TiKV cluster and PD endpoint running:

```sh
./target/debug/raw_async_demo 127.0.0.1:2379
```
