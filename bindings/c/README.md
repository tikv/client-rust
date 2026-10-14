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

The C ABI supports raw put, put with TTL, get, get-key-TTL, delete,
delete-range, forward and reverse scan, mixed put/delete batches, and
batch-get. Submitting a request copies its input buffers before returning.
An accepted request returns a handle and eventually invokes its completion
callback on a Rust runtime worker thread. The callback may read the result or
destroy the request handle. Each returned buffer, error, and snapshot has a
matching destroy function in the headers.

`tikv_async_client_new` leaves the gRPC response decoding limit unlimited for
compatibility. Use
`tikv_async_client_new_with_grpc_max_decoding_message_size` to set a limit in
bytes. Passing zero to the latter uses the Rust client's default (currently
4 MiB).

Cancellation is best effort. The underlying TiKV operation may already have
executed when cancellation is requested; in particular, cancelling a write
does not roll it back. `tikv_async_client_drain` waits for accepted callbacks
to finish. Do not call drain or client destruction from a callback belonging
to the same client.

An empty reverse-scan start key is unsupported by the underlying client.
Mixed write batches preserve the order of adjacent put and delete phases, but
may issue multiple TiKV requests.

## Metrics snapshots

`tikv_metrics_snapshot_collect` reads the process-wide Prometheus default
registry. The current C representation exports counter and histogram samples,
their metric name, the optional `type` label, and histogram bucket upper
bounds. It does not attribute samples to a particular C client. Strings in a
sample remain valid until its snapshot is destroyed with
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
