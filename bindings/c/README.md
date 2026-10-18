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
Status, error, and result accessors may be used only in the completion callback
or after synchronizing with it to observe completion. Serialize all accessors
and destruction for the same request, including calls from its callback. A
cross-thread handoff must happen after the previous thread's last access and
must synchronize the two threads. Different requests may be accessed concurrently.
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
the client handle before destroying it. Do not destroy a client from one of its
own callbacks.

An empty reverse-scan start key is unsupported by the underlying client.
Mixed write batches preserve the order of adjacent put and delete phases, but
may issue multiple TiKV requests.

## Metrics snapshots

`tikv_metrics_snapshot_collect` exports only the eight client-rust TiKV/PD
request counter and duration histogram families named in
`tikv_client_metrics_c.h`. It reads the process-wide Prometheus default registry,
so values include all client instances, including callers outside the C API.
It accepts only the expected metric type for each family and samples whose sole
label is `type`; other families and incompatible samples are skipped. Histogram
samples include cumulative buckets (including +Inf), sum, and count. Strings in
a sample remain valid until its snapshot is destroyed with
`tikv_metrics_snapshot_destroy`.

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
