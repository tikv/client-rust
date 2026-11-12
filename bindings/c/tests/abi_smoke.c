// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

#include "tikv_client_async_c.h"
#include "tikv_client_metrics_c.h"

#include <assert.h>
#include <string.h>

/* Run through the public C headers and the built library without a cluster.
 * The RawKV example is also compiled and linked in CI to cover its API symbols.
 */
int main(void) {
  tikv_async_client_t *client = NULL;
  tikv_error_t *error = NULL;
  assert(tikv_async_client_destroy(NULL) == TIKV_STATUS_OK);

  /* Invalid endpoints fail before connecting to PD. Check enum return values,
   * output pointers, and Rust-owned error strings across the C ABI.
   */
  tikv_status_t status = tikv_async_client_new_with_grpc_max_decoding_message_size(
      "", 0, false, 1024, &client, &error);
  assert(status == TIKV_STATUS_INVALID_ARGUMENT);
  assert(client == NULL);
  assert(error != NULL);
  const char *message = tikv_error_message(error);
  assert(message != NULL);
  assert(strstr(message, "at least one endpoint") != NULL);
  tikv_error_destroy(error);
  error = NULL;

  /* This fresh process has no request metrics. Check the C array layout and empty
   * view contract, and release the snapshot using its public destroy function.
   * Numeric histogram values are covered by the Rust tests.
   */
  tikv_metrics_snapshot_t *snapshot = NULL;
  tikv_metric_samples_t samples = {NULL, 123};
  status = tikv_metrics_snapshot_collect(&snapshot, &samples, &error);
  assert(status == TIKV_STATUS_OK);
  assert(error == NULL);
  assert(snapshot != NULL);
  assert(samples.ptr == NULL);
  assert(samples.len == 0);
  tikv_metrics_snapshot_destroy(snapshot);

  /* Invalid input fails before request creation. Verify that the binding still
   * exports its rejection counter and the new gauge kind with a NULL label,
   * using only the public ABI and no running cluster.
   */
  tikv_async_request_t *request = NULL;
  tikv_slice_t invalid_key = {NULL, 1};
  status = tikv_async_raw_get_submit(NULL, invalid_key, NULL, NULL, &request, &error);
  assert(status == TIKV_STATUS_INVALID_ARGUMENT);
  assert(request == NULL);
  assert(error != NULL);
  tikv_error_destroy(error);
  error = NULL;
  status = tikv_metrics_snapshot_collect(&snapshot, &samples, &error);
  assert(status == TIKV_STATUS_OK);
  assert(error == NULL);
  assert(snapshot != NULL);
  assert(samples.ptr != NULL);
  assert(samples.len == 6);
  size_t gauges = 0;
  size_t rejections = 0;
  for (size_t i = 0; i < samples.len; ++i) {
    const tikv_metric_sample_t *sample = &samples.ptr[i];
    assert(sample->name != NULL);
    if (strcmp(sample->name, TIKV_METRIC_FFI_REQUESTS_INFLIGHT) == 0) {
      assert(sample->kind == TIKV_METRIC_SAMPLE_GAUGE);
      assert(sample->type_label == NULL);
      assert(sample->value == 0);
      ++gauges;
    } else {
      assert(sample->kind == TIKV_METRIC_SAMPLE_COUNTER);
      assert(sample->type_label != NULL);
      if (strcmp(sample->name, TIKV_METRIC_FFI_SUBMISSIONS_TOTAL) == 0 &&
          strcmp(sample->type_label, TIKV_METRIC_LABEL_TYPE_REJECTED) == 0) {
        assert(sample->value == 1);
        ++rejections;
      } else {
        assert(sample->value == 0);
      }
    }
  }
  assert(gauges == 1);
  assert(rejections == 1);
  tikv_metrics_snapshot_destroy(snapshot);
  return 0;
}
