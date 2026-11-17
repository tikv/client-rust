// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

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
 * - Make shutdown and request ownership explicit.
 *
 * Keys and values are binary; NUL bytes are preserved. Range operations use
 * [start_key, end_key), with no upper bound only when end_key.len == 0.
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
 *   callback, including during client shutdown.
 * - A submit function returning non-`TIKV_STATUS_OK` means the request was not
 *   accepted and no completion callback will run.
 *
 * Request handle contract:
 *
 * - Submit success returns a request handle.
 * - The request handle remains valid until `tikv_async_request_destroy`.
 * - Destroy the request handle exactly once, either in its completion callback
 *   or after the caller has synchronized with the callback to observe completion.
 *   Do not destroy a pending request before its callback starts.
 * - A callback may run before the submit function returns. If the callback
 *   destroys the handle, the caller must not use the returned handle.
 * - Error and result accessors may be called only in the completion callback
 *   or after synchronizing with it to observe completion. The callback's
 *   `final_status` is the terminal request status.
 * - The caller must serialize all status/error/result accessor and destroy
 *   calls for the same request, including calls from its callback. Synchronize
 *   cross-thread ownership transfers.
 *   Different request handles may be accessed concurrently.
 * - Returned buffers and errors are Rust-owned and must be released with the
 *   matching destroy helper from this header.
 *
 * Client shutdown contract:
 *
 * - `tikv_async_client_shutdown_begin` immediately rejects new submit calls and
 *   lets accepted requests finish normally.
 * - `tikv_async_client_destroy` begins shutdown, waits for all accepted requests
 *   to finish their callbacks, and then releases the client.
 * - Client creation and destruction return `TIKV_STATUS_INVALID_ARGUMENT` in a
 *   Tokio context, including any completion callback. Rejected destruction
 *   leaves the client unchanged. Defer these operations to an application
 *   thread; a callback must not wait for that thread to drain its own client.
 * - Before destroying a client, stop all other calls using its handle.
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
  TIKV_STATUS_REJECTED = 4
} tikv_status_t;

/*
 * Completion callback type.
 *
 * `final_status` is the terminal status of the accepted request:
 *
 * - `TIKV_STATUS_OK`: request succeeded.
 * - `TIKV_STATUS_ERROR`: request failed; inspect
 *   `tikv_async_request_error_message`.
 * - `TIKV_STATUS_PANIC`: Rust caught a panic while executing the request.
 *
 * The callback may call request result accessors and
 * `tikv_async_request_destroy`.
 * Client creation and destruction are rejected here; see the shutdown contract.
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
 * Returns `TIKV_STATUS_INVALID_ARGUMENT` in a Tokio context (including completion
 * callbacks), leaving `*out_client` NULL. Use an application thread instead.
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
 * The same threading restriction as `tikv_async_client_new` applies.
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
 * Accepted requests finish normally and retain their operation status.
 */
tikv_status_t tikv_async_client_shutdown_begin(tikv_async_client_t *client,
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
 * The range is [start_key, end_key); an empty end_key means no upper bound.
 * Empty bounds are passed through to the Rust client for keyspace encoding.
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
 * Return the terminal request error message, or NULL if none.
 *
 * This may be called only after completion.
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

/*
 * Release a completed request handle. NULL is a no-op.
 *
 * Call exactly once, in the completion callback or after synchronizing with it.
 * Destroying a pending request before its callback starts is invalid.
 */
void tikv_async_request_destroy(tikv_async_request_t *request);

/*
 * Release a client after draining its callbacks. Returns `TIKV_STATUS_OK` on
 * success, including for NULL (a no-op).
 *
 * Returns `TIKV_STATUS_INVALID_ARGUMENT` in a Tokio context, including any
 * completion callback, leaving the client unchanged. Retry on an application
 * thread. Stop all other calls using the handle before successful destruction.
 */
tikv_status_t tikv_async_client_destroy(tikv_async_client_t *client);

#ifdef __cplusplus
}
#endif

#endif
