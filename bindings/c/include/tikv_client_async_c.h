// Copyright (c) 2026 PingCAP, Inc.

#ifndef TIKV_CLIENT_ASYNC_C_H
#define TIKV_CLIENT_ASYNC_C_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Async C ABI contract.
 *
 * Design goals for this binding:
 *
 * - Expose a natural async ABI: submit + completion callback.
 * - Copy every cross-async-boundary input before submit returns.
 * - Make shutdown/drain/cancel semantics explicit and defensive.
 *
 * Threading model:
 *
 * - Submit functions may be called concurrently on the same client handle.
 * - Completion callbacks run on Rust async runtime worker threads, not on the
 *   submitter thread.
 * - Completion callbacks must therefore be thread-safe.
 *
 * Completion contract:
 *
 * - Every request accepted by a submit function triggers exactly one completion
 *   callback, including cancellation and shutdown-triggered cancellation.
 * - A submit function returning non-`TIKV_STATUS_OK` means the request was not
 *   accepted and no completion callback will run.
 *
 * Request handle contract:
 *
 * - Submit success returns a request handle.
 * - The request handle remains valid until `tikv_async_request_destroy`.
 * - It is valid to destroy the request handle before the callback fires; Rust
 *   keeps the internal state alive until the callback returns.
 * - Result accessors may be called only after completion.
 * - Returned buffers and errors are Rust-owned and must be released with the
 *   matching destroy helper from this header.
 *
 * Client shutdown contract:
 *
 * - `tikv_async_client_shutdown_begin` immediately rejects new submit calls.
 * - `tikv_async_client_shutdown_begin(..., false, ...)` lets in-flight requests
 *   finish normally.
 * - `tikv_async_client_shutdown_begin(..., true, ...)` requests cancellation of
 *   every in-flight request; each accepted request still completes through its
 *   callback with `TIKV_STATUS_CANCELLED`.
 * - `tikv_async_client_drain` blocks until every accepted in-flight request has
 *   finished its callback.
 * - `tikv_async_client_destroy` performs shutdown-begin with cancellation,
 *   drains, and then releases the client.
 * - Do not call `tikv_async_client_drain` or `tikv_async_client_destroy` from a
 *   completion callback belonging to the same client, or you may deadlock.
 */

typedef struct tikv_async_client_t tikv_async_client_t;
typedef struct tikv_async_request_t tikv_async_request_t;
typedef struct tikv_error_t tikv_error_t;

typedef struct tikv_slice_t {
  const uint8_t *ptr;
  size_t len;
} tikv_slice_t;

typedef struct tikv_slices_t {
  const tikv_slice_t *ptr;
  size_t len;
} tikv_slices_t;

typedef enum tikv_async_raw_mutation_op_t {
  TIKV_ASYNC_RAW_MUTATION_OP_PUT = 0,
  TIKV_ASYNC_RAW_MUTATION_OP_DELETE = 1
} tikv_async_raw_mutation_op_t;

typedef struct tikv_async_raw_mutation_t {
  tikv_async_raw_mutation_op_t op;
  tikv_slice_t key;
  /*
   * Used only for PUT.
   * DELETE ignores this field, so callers may pass {NULL, 0}.
   */
  tikv_slice_t value;
} tikv_async_raw_mutation_t;

typedef struct tikv_async_raw_mutations_t {
  const tikv_async_raw_mutation_t *ptr;
  size_t len;
} tikv_async_raw_mutations_t;

/*
 * TTL-capable mutation used by
 * `tikv_async_raw_batch_write_with_ttl_submit`.
 *
 * `ttl_secs` is a relative TTL in seconds and is used only for PUT. A value of
 * 0 disables TTL for the key. DELETE ignores both `value` and `ttl_secs`.
 */
typedef struct tikv_async_raw_mutation_with_ttl_t {
  tikv_async_raw_mutation_op_t op;
  tikv_slice_t key;
  tikv_slice_t value;
  uint64_t ttl_secs;
} tikv_async_raw_mutation_with_ttl_t;

typedef struct tikv_async_raw_mutations_with_ttl_t {
  const tikv_async_raw_mutation_with_ttl_t *ptr;
  size_t len;
} tikv_async_raw_mutations_with_ttl_t;

typedef struct tikv_owned_bytes_t {
  uint8_t *ptr;
  size_t len;
} tikv_owned_bytes_t;

typedef struct tikv_kv_pair_t {
  tikv_owned_bytes_t key;
  tikv_owned_bytes_t value;
} tikv_kv_pair_t;

typedef struct tikv_kv_pairs_t {
  tikv_kv_pair_t *ptr;
  size_t len;
} tikv_kv_pairs_t;

typedef enum tikv_status_t {
  TIKV_STATUS_OK = 0,
  TIKV_STATUS_ERROR = 1,
  TIKV_STATUS_INVALID_ARGUMENT = 2,
  TIKV_STATUS_PANIC = 3,
  TIKV_STATUS_REJECTED = 4,
  TIKV_STATUS_CANCELLED = 5
} tikv_status_t;

/*
 * Completion callback type.
 *
 * `final_status` is the terminal status of the accepted request:
 *
 * - `TIKV_STATUS_OK`: request succeeded.
 * - `TIKV_STATUS_ERROR`: request failed; inspect
 *   `tikv_async_request_error_message`.
 * - `TIKV_STATUS_CANCELLED`: request was cancelled explicitly or by client
 *   shutdown; inspect `tikv_async_request_error_message` for details.
 * - `TIKV_STATUS_PANIC`: Rust caught a panic while executing the request.
 *
 * The callback may call request result accessors, `tikv_async_request_cancel`
 * (which becomes a no-op after completion), and `tikv_async_request_destroy`.
 */
typedef void (*tikv_async_completion_cb)(tikv_async_request_t *request,
                                         tikv_status_t final_status,
                                         void *userdata);

/*
 * Create a client.
 *
 * `pd_endpoints_csv` must be valid UTF-8 and contain at least one endpoint.
 * `timeout_secs == 0` leaves the client timeout at its library default.
 * `use_default_keyspace` enables API v2 default keyspace configuration.
 * For compatibility, this constructor has no gRPC response decoding limit.
 * Use the constructor below to choose a limit explicitly.
 */
tikv_status_t tikv_async_client_new(const char *pd_endpoints_csv,
                                    uint32_t timeout_secs,
                                    bool use_default_keyspace,
                                    tikv_async_client_t **out_client,
                                    tikv_error_t **out_error);

/*
 * Create a client with an explicit gRPC response decoding limit in bytes.
 *
 * `grpc_max_decoding_message_size == 0` uses the Rust client's default
 * (currently 4 MiB). Larger responses require an explicit limit.
 */
tikv_status_t tikv_async_client_new_with_grpc_max_decoding_message_size(
    const char *pd_endpoints_csv,
    uint32_t timeout_secs,
    bool use_default_keyspace,
    size_t grpc_max_decoding_message_size,
    tikv_async_client_t **out_client,
    tikv_error_t **out_error);

/*
 * Start shutdown.
 *
 * This function is idempotent.
 *
 * After the first successful call, every later submit on the same client is
 * rejected with `TIKV_STATUS_REJECTED`, and no callback is scheduled for those
 * rejected submits.
 *
 * If `cancel_inflight` is true, every currently accepted in-flight request is
 * asked to stop and will complete through its callback with
 * `TIKV_STATUS_CANCELLED`.
 */
tikv_status_t tikv_async_client_shutdown_begin(tikv_async_client_t *client,
                                               bool cancel_inflight,
                                               tikv_error_t **out_error);

/*
 * Wait until every accepted in-flight request has finished its callback.
 *
 * This function may be called before or after shutdown begins.
 * If shutdown has not yet begun, it only waits for the currently in-flight set;
 * it does not prevent new submissions racing from other threads.
 */
tikv_status_t tikv_async_client_drain(const tikv_async_client_t *client,
                                      tikv_error_t **out_error);

/*
 * Submit a raw put.
 *
 * Submit success means:
 *
 * - Rust copied the input buffers.
 * - Rust registered the request for eventual completion.
 * - Rust will invoke `completion` exactly once.
 */
tikv_status_t tikv_async_raw_put_submit(const tikv_async_client_t *client,
                                        tikv_slice_t key,
                                        tikv_slice_t value,
                                        tikv_async_completion_cb completion,
                                        void *userdata,
                                        tikv_async_request_t **out_request,
                                        tikv_error_t **out_error);

/*
 * Submit a raw put with a relative TTL in seconds.
 *
 * `ttl_secs == 0` disables TTL for the key. TiKV must be configured to support
 * RawKV TTL before callers submit a non-zero value.
 *
 * Submit success has the same copy and completion guarantees as
 * `tikv_async_raw_put_submit`.
 */
tikv_status_t tikv_async_raw_put_with_ttl_submit(
    const tikv_async_client_t *client,
    tikv_slice_t key,
    tikv_slice_t value,
    uint64_t ttl_secs,
    tikv_async_completion_cb completion,
    void *userdata,
    tikv_async_request_t **out_request,
    tikv_error_t **out_error);

/*
 * Submit a raw get.
 *
 * On success, completion with `TIKV_STATUS_OK` makes the value available
 * through `tikv_async_request_take_get_result`.
 */
tikv_status_t tikv_async_raw_get_submit(const tikv_async_client_t *client,
                                        tikv_slice_t key,
                                        tikv_async_completion_cb completion,
                                        void *userdata,
                                        tikv_async_request_t **out_request,
                                        tikv_error_t **out_error);

/*
 * Submit a request for the remaining RawKV TTL of a key.
 *
 * On success, completion with `TIKV_STATUS_OK` makes the result available
 * through `tikv_async_request_take_get_key_ttl_result`.
 */
tikv_status_t tikv_async_raw_get_key_ttl_submit(
    const tikv_async_client_t *client,
    tikv_slice_t key,
    tikv_async_completion_cb completion,
    void *userdata,
    tikv_async_request_t **out_request,
    tikv_error_t **out_error);

/*
 * Submit a raw delete.
 */
tikv_status_t tikv_async_raw_delete_submit(const tikv_async_client_t *client,
                                           tikv_slice_t key,
                                           tikv_async_completion_cb completion,
                                           void *userdata,
                                           tikv_async_request_t **out_request,
                                           tikv_error_t **out_error);

/*
 * Submit a raw delete-range.
 *
 * An empty `end_key` means unbounded upper range.
 */
tikv_status_t
 tikv_async_raw_delete_range_submit(const tikv_async_client_t *client,
                                    tikv_slice_t start_key,
                                    tikv_slice_t end_key,
                                    tikv_async_completion_cb completion,
                                    void *userdata,
                                    tikv_async_request_t **out_request,
                                    tikv_error_t **out_error);

/*
 * Submit a forward raw scan.
 *
 * On success, completion with `TIKV_STATUS_OK` makes the pairs available
 * through `tikv_async_request_take_scan_result`.
 */
tikv_status_t tikv_async_raw_scan_submit(const tikv_async_client_t *client,
                                         tikv_slice_t start_key,
                                         tikv_slice_t end_key,
                                         uint32_t limit,
                                         tikv_async_completion_cb completion,
                                         void *userdata,
                                         tikv_async_request_t **out_request,
                                         tikv_error_t **out_error);

/*
 * Submit a backward raw scan.
 *
 * This maps to `client-rust` raw `scan_reverse`.
 * The returned pairs are in reverse lexicographical key order.
 *
 * Note: the underlying raw client does not support reverse scan from an empty
 * start key because locating the last region is not implemented.
 */
tikv_status_t
 tikv_async_raw_scan_reverse_submit(const tikv_async_client_t *client,
                                    tikv_slice_t start_key,
                                    tikv_slice_t end_key,
                                    uint32_t limit,
                                    tikv_async_completion_cb completion,
                                    void *userdata,
                                    tikv_async_request_t **out_request,
                                    tikv_error_t **out_error);

/*
 * Submit a mixed raw write batch.
 *
 * This reduces per-request overhead by letting the caller hand multiple put and
 * delete mutations to one async request.
 *
 * `client-rust` raw API exposes `batch_put` and `batch_delete`, not one mixed
 * mutation RPC. This binding therefore preserves the caller's put/delete phase
 * order while coalescing adjacent same-type mutations into as few TiKV calls as
 * practical.
 *
 * Example:
 *
 *   PUT a, PUT b, DELETE c, DELETE d, PUT e
 *
 * becomes:
 *
 *   batch_put(a, b) -> batch_delete(c, d) -> batch_put(e)
 *
 * Repeated PUTs to the same key inside one contiguous PUT run are split before
 * submit so that later writes still win deterministically.
 */
tikv_status_t tikv_async_raw_batch_write_submit(
    const tikv_async_client_t *client,
    tikv_async_raw_mutations_t mutations,
    tikv_async_completion_cb completion,
    void *userdata,
    tikv_async_request_t **out_request,
    tikv_error_t **out_error);

/*
 * Submit a TTL-capable mixed raw write batch.
 *
 * This is the ABI-compatible TTL counterpart of
 * `tikv_async_raw_batch_write_submit`. Each PUT may carry its own relative TTL
 * in `ttl_secs`; PUTs with `ttl_secs == 0` keep the existing non-TTL behavior.
 * DELETE ignores `ttl_secs`.
 *
 * The implementation maps PUT runs to `client-rust`
 * `batch_put_with_ttl` and DELETE runs to `batch_delete`, preserving mutation
 * phase order and later-write-wins behavior for repeated keys.
 *
 * TiKV must be configured to support RawKV TTL before callers submit a
 * non-zero TTL.
 */
tikv_status_t tikv_async_raw_batch_write_with_ttl_submit(
    const tikv_async_client_t *client,
    tikv_async_raw_mutations_with_ttl_t mutations,
    tikv_async_completion_cb completion,
    void *userdata,
    tikv_async_request_t **out_request,
    tikv_error_t **out_error);

/*
 * Submit a raw batch-get.
 *
 * TiKV raw batch-get returns only existing entries and does not preserve the
 * input key order.
 * On success, completion with `TIKV_STATUS_OK` makes the pairs available
 * through `tikv_async_request_take_batch_get_result`.
 */
tikv_status_t tikv_async_raw_batch_get_submit(const tikv_async_client_t *client,
                                              tikv_slices_t keys,
                                              tikv_async_completion_cb completion,
                                              void *userdata,
                                              tikv_async_request_t **out_request,
                                              tikv_error_t **out_error);

/*
 * Request lifecycle helpers.
 */

/*
 * Ask an accepted request to cancel.
 *
 * This function is best-effort and idempotent.
 *
 * - `*out_cancel_requested == true` means this call newly requested
 *   cancellation.
 * - `*out_cancel_requested == false` means the request had already completed or
 *   cancellation had already been requested earlier.
 *
 * If the request was still pending, its completion callback still runs exactly
 * once with `TIKV_STATUS_CANCELLED`.
 */
tikv_status_t tikv_async_request_cancel(tikv_async_request_t *request,
                                        bool *out_cancel_requested,
                                        tikv_error_t **out_error);

/*
 * Return whether the request has completed.
 */
tikv_status_t tikv_async_request_is_complete(const tikv_async_request_t *request,
                                             bool *out_complete,
                                             tikv_error_t **out_error);

/*
 * Return the terminal request status.
 *
 * This may be called only after completion.
 */
tikv_status_t tikv_async_request_completion_status(
    const tikv_async_request_t *request,
    tikv_status_t *out_status,
    tikv_error_t **out_error);

/*
 * Return the terminal request error message, or NULL if none.
 *
 * The returned pointer is borrowed from the request state and remains valid
 * until `tikv_async_request_destroy`.
 */
const char *tikv_async_request_error_message(const tikv_async_request_t *request);

/*
 * Result extraction helpers.
 *
 * Each successful result may be taken only once.
 * They may be called only after completion, and only for a matching request
 * kind that completed with `TIKV_STATUS_OK`.
 */
tikv_status_t tikv_async_request_take_get_result(
    const tikv_async_request_t *request,
    tikv_owned_bytes_t *out_value,
    bool *out_found,
    tikv_error_t **out_error);

/*
 * Take a get-key-TTL result.
 *
 * `*out_found == false` means the key does not exist and `*out_ttl_secs` is 0.
 * `*out_found == true && *out_ttl_secs == 0` means the key exists without a
 * TTL. Otherwise `*out_ttl_secs` is the remaining TTL in seconds.
 */
tikv_status_t tikv_async_request_take_get_key_ttl_result(
    const tikv_async_request_t *request,
    uint64_t *out_ttl_secs,
    bool *out_found,
    tikv_error_t **out_error);

tikv_status_t tikv_async_request_take_scan_result(
    const tikv_async_request_t *request,
    tikv_kv_pairs_t *out_pairs,
    tikv_error_t **out_error);

tikv_status_t tikv_async_request_take_batch_get_result(
    const tikv_async_request_t *request,
    tikv_kv_pairs_t *out_pairs,
    tikv_error_t **out_error);

/*
 * Destroy helpers.
 */
const char *tikv_error_message(const tikv_error_t *error);
void tikv_error_destroy(tikv_error_t *error);

void tikv_owned_bytes_destroy(tikv_owned_bytes_t bytes);
void tikv_kv_pairs_destroy(tikv_kv_pairs_t pairs);

void tikv_async_request_destroy(tikv_async_request_t *request);
void tikv_async_client_destroy(tikv_async_client_t *client);

#ifdef __cplusplus
}
#endif

#endif
