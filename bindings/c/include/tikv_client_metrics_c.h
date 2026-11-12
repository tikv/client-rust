// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

#ifndef TIKV_CLIENT_METRICS_C_H
#define TIKV_CLIENT_METRICS_C_H

#include "tikv_client_async_c.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct tikv_metrics_snapshot_t tikv_metrics_snapshot_t;

typedef enum tikv_metric_sample_kind_t {
  TIKV_METRIC_SAMPLE_UNKNOWN = 0,
  TIKV_METRIC_SAMPLE_COUNTER = 1,
  TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET = 2,
  TIKV_METRIC_SAMPLE_HISTOGRAM_SUM = 3,
  TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT = 4,
  TIKV_METRIC_SAMPLE_GAUGE = 5
} tikv_metric_sample_kind_t;

/*
 * Metric families exported by `tikv_metrics_snapshot_collect`.
 * Request totals are counters; request durations are histograms.
 * FFI submissions and completions are counters; the in-flight count is a gauge.
 */
#define TIKV_METRIC_TIKV_REQUEST_TOTAL "tikv_request_total"
#define TIKV_METRIC_TIKV_REQUEST_DURATION_SECONDS                         \
  "tikv_request_duration_seconds"
#define TIKV_METRIC_TIKV_FAILED_REQUEST_TOTAL "tikv_failed_request_total"
#define TIKV_METRIC_TIKV_FAILED_REQUEST_DURATION_SECONDS                  \
  "tikv_failed_request_duration_seconds"
#define TIKV_METRIC_PD_REQUEST_TOTAL "pd_request_total"
#define TIKV_METRIC_PD_REQUEST_DURATION_SECONDS "pd_request_duration_seconds"
#define TIKV_METRIC_PD_FAILED_REQUEST_TOTAL "pd_failed_request_total"
#define TIKV_METRIC_PD_FAILED_REQUEST_DURATION_SECONDS                    \
  "pd_failed_request_duration_seconds"
#define TIKV_METRIC_FFI_SUBMISSIONS_TOTAL "tikv_ffi_submissions_total"
#define TIKV_METRIC_FFI_COMPLETIONS_TOTAL "tikv_ffi_completions_total"
#define TIKV_METRIC_FFI_REQUESTS_INFLIGHT "tikv_ffi_requests_inflight"

/* FFI submission outcomes and final callback statuses, respectively. */
#define TIKV_METRIC_LABEL_TYPE_ACCEPTED "accepted"
#define TIKV_METRIC_LABEL_TYPE_REJECTED "rejected"
#define TIKV_METRIC_LABEL_TYPE_OK "ok"
#define TIKV_METRIC_LABEL_TYPE_ERROR "error"
#define TIKV_METRIC_LABEL_TYPE_PANIC "panic"

/*
 * Values of the Prometheus label key `type` used by the C RawKV operations
 * and their PD lookups. Other `type` values may appear when callers outside
 * the C API use these same metric families.
 */
#define TIKV_METRIC_LABEL_TYPE_RAW_GET "raw_get"
#define TIKV_METRIC_LABEL_TYPE_RAW_BATCH_GET "raw_batch_get"
#define TIKV_METRIC_LABEL_TYPE_RAW_PUT "raw_put"
#define TIKV_METRIC_LABEL_TYPE_RAW_BATCH_PUT "raw_batch_put"
#define TIKV_METRIC_LABEL_TYPE_RAW_DELETE "raw_delete"
#define TIKV_METRIC_LABEL_TYPE_RAW_BATCH_DELETE "raw_batch_delete"
#define TIKV_METRIC_LABEL_TYPE_RAW_SCAN "raw_scan"
#define TIKV_METRIC_LABEL_TYPE_RAW_DELETE_RANGE "raw_delete_range"

#define TIKV_METRIC_LABEL_TYPE_PD_GET_REGION "get_region"
#define TIKV_METRIC_LABEL_TYPE_PD_GET_REGION_BY_ID "get_region_by_id"
#define TIKV_METRIC_LABEL_TYPE_PD_GET_STORE "get_store"
#define TIKV_METRIC_LABEL_TYPE_PD_GET_ALL_STORES "get_all_stores"
#define TIKV_METRIC_LABEL_TYPE_PD_LOAD_KEYSPACE "load_keyspace"

typedef struct tikv_metric_sample_t {
  const char *name;
  const char *type_label;
  tikv_metric_sample_kind_t kind;
  double value;
  double le;
} tikv_metric_sample_t;

typedef struct tikv_metric_samples_t {
  const tikv_metric_sample_t *ptr;
  size_t len;
} tikv_metric_samples_t;

/*
 * Collect the eight TiKV/PD RPC families and three C binding families named
 * above, from the process Prometheus default registry. All values are shared
 * by client instances in the process. RPC values include callers outside the
 * C API; FFI values count only C RawKV submissions and their callbacks.
 *
 * FFI submission `type` is accepted or rejected. Rejected includes every
 * non-OK submit return, including invalid arguments, shutdown, and panics.
 * FFI completion `type` is ok, error, or panic, recorded after callback return.
 * The unlabeled in-flight gauge counts accepted requests through callback
 * return, excluding completed request handles retained by C. Client creation,
 * result access, and destruction are not submissions. FFI series are created
 * on the first successful client creation or RawKV submit call, even if rejected.
 *
 * Each counter/histogram sample has exactly one label named `type`; the gauge
 * has none. Other families, unexpected types, and incompatible labels are
 * skipped. Histogram samples include cumulative buckets (including +Inf), sum,
 * and count. Concurrent updates may be observed at different instants across
 * series; the snapshot is not an atomic view of the request lifecycle.
 *
 * On success, `out_samples` views every sample in one contiguous array owned
 * by `out_snapshot`. The array and its strings remain valid until
 * `tikv_metrics_snapshot_destroy`; callers must not modify or free them.
 * For an empty snapshot, `*out_samples` is {NULL, 0}.
 * `type_label` is NULL for the gauge, otherwise the Prometheus `type` value.
 * `le` is meaningful only for histogram buckets.
 * On failure, each non-NULL output pointer is cleared to NULL or {NULL, 0}.
 */
tikv_status_t tikv_metrics_snapshot_collect(
    tikv_metrics_snapshot_t **out_snapshot,
    tikv_metric_samples_t *out_samples,
    tikv_error_t **out_error);

void tikv_metrics_snapshot_destroy(tikv_metrics_snapshot_t *snapshot);

#ifdef __cplusplus
}
#endif

#endif
