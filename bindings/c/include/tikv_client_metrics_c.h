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
  TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT = 4
} tikv_metric_sample_kind_t;

/*
 * Metric families exported by `tikv_metrics_snapshot_collect`.
 * Request totals are counters; request durations are histograms.
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

/*
 * Collect a structured snapshot of only the eight client-rust TiKV/PD request
 * metric families named above, from the process Prometheus default registry.
 * Values are shared by all client instances in the process, including callers
 * outside the C API; they are not scoped to a particular C client handle.
 *
 * Only counters for request totals and histograms for request durations are
 * exported. Each sample must have exactly one label, named `type`. Other
 * families, unexpected metric types, and samples with missing or additional
 * labels are skipped. Histograms include cumulative buckets (including +Inf),
 * sum, and count.
 *
 * Strings referenced by `tikv_metric_sample_t` are owned by the snapshot and
 * remain valid until `tikv_metrics_snapshot_destroy`. `type_label` is always
 * present and contains the sample's Prometheus `type` label value.
 */
tikv_status_t tikv_metrics_snapshot_collect(
    tikv_metrics_snapshot_t **out_snapshot,
    tikv_error_t **out_error);

size_t tikv_metrics_snapshot_len(const tikv_metrics_snapshot_t *snapshot);

tikv_status_t tikv_metrics_snapshot_get(
    const tikv_metrics_snapshot_t *snapshot,
    size_t index,
    tikv_metric_sample_t *out_sample,
    tikv_error_t **out_error);

void tikv_metrics_snapshot_destroy(tikv_metrics_snapshot_t *snapshot);

#ifdef __cplusplus
}
#endif

#endif
