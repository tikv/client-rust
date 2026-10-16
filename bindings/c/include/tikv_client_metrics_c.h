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
 * Metric names currently emitted by client-rust.
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
#define TIKV_METRIC_PD_TSO_BATCH_SIZE "pd_tso_batch_size"

/*
 * Values currently used for the Prometheus label key `type`.
 */
#define TIKV_METRIC_LABEL_TYPE_RAW_GET "raw_get"
#define TIKV_METRIC_LABEL_TYPE_RAW_BATCH_GET "raw_batch_get"
#define TIKV_METRIC_LABEL_TYPE_RAW_GET_KEY_TTL "raw_get_key_ttl"
#define TIKV_METRIC_LABEL_TYPE_RAW_PUT "raw_put"
#define TIKV_METRIC_LABEL_TYPE_RAW_BATCH_PUT "raw_batch_put"
#define TIKV_METRIC_LABEL_TYPE_RAW_DELETE "raw_delete"
#define TIKV_METRIC_LABEL_TYPE_RAW_BATCH_DELETE "raw_batch_delete"
#define TIKV_METRIC_LABEL_TYPE_RAW_SCAN "raw_scan"
#define TIKV_METRIC_LABEL_TYPE_RAW_BATCH_SCAN "raw_batch_scan"
#define TIKV_METRIC_LABEL_TYPE_RAW_DELETE_RANGE "raw_delete_range"
#define TIKV_METRIC_LABEL_TYPE_RAW_COMPARE_AND_SWAP "raw_compare_and_swap"
#define TIKV_METRIC_LABEL_TYPE_RAW_COPROCESSOR "raw_coprocessor"

#define TIKV_METRIC_LABEL_TYPE_KV_GET "kv_get"
#define TIKV_METRIC_LABEL_TYPE_KV_SCAN "kv_scan"
#define TIKV_METRIC_LABEL_TYPE_KV_PREWRITE "kv_prewrite"
#define TIKV_METRIC_LABEL_TYPE_KV_COMMIT "kv_commit"
#define TIKV_METRIC_LABEL_TYPE_KV_BATCH_GET "kv_batch_get"
#define TIKV_METRIC_LABEL_TYPE_KV_BATCH_ROLLBACK "kv_batch_rollback"
#define TIKV_METRIC_LABEL_TYPE_KV_PESSIMISTIC_ROLLBACK                   \
  "kv_pessimistic_rollback"
#define TIKV_METRIC_LABEL_TYPE_KV_RESOLVE_LOCK "kv_resolve_lock"
#define TIKV_METRIC_LABEL_TYPE_KV_SCAN_LOCK "kv_scan_lock"
#define TIKV_METRIC_LABEL_TYPE_KV_PESSIMISTIC_LOCK "kv_pessimistic_lock"
#define TIKV_METRIC_LABEL_TYPE_KV_TXN_HEART_BEAT "kv_txn_heart_beat"
#define TIKV_METRIC_LABEL_TYPE_KV_CHECK_TXN_STATUS "kv_check_txn_status"
#define TIKV_METRIC_LABEL_TYPE_KV_CHECK_SECONDARY_LOCKS_REQUEST          \
  "kv_check_secondary_locks_request"
#define TIKV_METRIC_LABEL_TYPE_KV_GC "kv_gc"
#define TIKV_METRIC_LABEL_TYPE_KV_DELETE_RANGE "kv_delete_range"
#define TIKV_METRIC_LABEL_TYPE_UNSAFE_DESTROY_RANGE "unsafe_destroy_range"

#define TIKV_METRIC_LABEL_TYPE_PD_GET_REGION "get_region"
#define TIKV_METRIC_LABEL_TYPE_PD_GET_REGION_BY_ID "get_region_by_id"
#define TIKV_METRIC_LABEL_TYPE_PD_GET_STORE "get_store"
#define TIKV_METRIC_LABEL_TYPE_PD_GET_ALL_STORES "get_all_stores"
#define TIKV_METRIC_LABEL_TYPE_PD_GET_TIMESTAMP "get_timestamp"
#define TIKV_METRIC_LABEL_TYPE_PD_UPDATE_GC_SAFEPOINT                    \
  "update_gc_safepoint"
#define TIKV_METRIC_LABEL_TYPE_PD_LOAD_KEYSPACE "load_keyspace"

typedef struct tikv_metric_sample_t {
  const char *name;
  const char *type_label;
  tikv_metric_sample_kind_t kind;
  double value;
  double le;
} tikv_metric_sample_t;

/*
 * Collect a structured snapshot of the current process Prometheus default
 * registry.
 *
 * Strings referenced by `tikv_metric_sample_t` are owned by the snapshot and
 * remain valid until `tikv_metrics_snapshot_destroy`. `type_label` is NULL when
 * the sample has no Prometheus `type` label.
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
