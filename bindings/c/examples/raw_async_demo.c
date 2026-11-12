// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

#include "tikv_client_async_c.h"

#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct request_latch_t {
  atomic_int done;
  tikv_status_t status;
} request_latch_t;

static tikv_slice_t slice_from_cstr(const char *value) {
  tikv_slice_t slice;
  slice.ptr = (const uint8_t *)value;
  slice.len = strlen(value);
  return slice;
}

static void print_error_and_destroy(const char *label, tikv_error_t *error) {
  const char *message = tikv_error_message(error);
  fprintf(stderr, "%s: %s\n", label, message == NULL ? "(null)" : message);
  tikv_error_destroy(error);
}

static int expect_ok(const char *label,
                     tikv_status_t status,
                     tikv_error_t *error) {
  if (status == TIKV_STATUS_OK) {
    return 1;
  }

  if (error != NULL) {
    print_error_and_destroy(label, error);
  } else {
    fprintf(stderr, "%s: status=%d\n", label, (int)status);
  }
  return 0;
}

static void print_bytes(const uint8_t *ptr, size_t len) {
  for (size_t i = 0; i < len; ++i) {
    putchar((int)ptr[i]);
  }
}

static void completion_cb(tikv_async_request_t *request,
                          tikv_status_t final_status,
                          void *userdata) {
  request_latch_t *latch = (request_latch_t *)userdata;
  latch->status = final_status;
  atomic_store_explicit(&latch->done, 1, memory_order_release);
  (void)request;
}

static int wait_latch_ok(const char *label,
                         tikv_async_request_t *request,
                         request_latch_t *latch) {
  while (!atomic_load_explicit(&latch->done, memory_order_acquire)) {
    struct timespec delay;
    delay.tv_sec = 0;
    delay.tv_nsec = 10 * 1000 * 1000;
    nanosleep(&delay, NULL);
  }

  if (latch->status == TIKV_STATUS_OK) {
    return 1;
  }

  const char *message = tikv_async_request_error_message(request);
  fprintf(stderr,
          "%s: completion status=%d message=%s\n",
          label,
          (int)latch->status,
          message == NULL ? "(null)" : message);
  return 0;
}

int main(int argc, char **argv) {
  const char *pd = argc > 1 ? argv[1] : "127.0.0.1:2379";
  const tikv_slice_t scan_start = slice_from_cstr("tikv-client-rust-c/");
  const tikv_slice_t scan_end = slice_from_cstr("tikv-client-rust-c0");
  const tikv_slice_t key_a = slice_from_cstr("tikv-client-rust-c/a");
  const tikv_slice_t key_b = slice_from_cstr("tikv-client-rust-c/b");
  const tikv_slice_t value_a_v1 = slice_from_cstr("value-a-v1");
  const tikv_slice_t value_a_v2 = slice_from_cstr("value-a-v2");
  const tikv_slice_t value_b = slice_from_cstr("value-b");

  const tikv_async_raw_mutation_t write_items_buf[] = {
      {.op = TIKV_ASYNC_RAW_MUTATION_OP_PUT,
       .key = key_a,
       .value = value_a_v1},
      {.op = TIKV_ASYNC_RAW_MUTATION_OP_PUT,
       .key = key_b,
       .value = value_b},
      {.op = TIKV_ASYNC_RAW_MUTATION_OP_DELETE,
       .key = key_a,
       .value = {NULL, 0}},
      {.op = TIKV_ASYNC_RAW_MUTATION_OP_PUT,
       .key = key_a,
       .value = value_a_v2},
  };
  const tikv_async_raw_mutations_t write_items = {
      .ptr = write_items_buf,
      .len = sizeof(write_items_buf) / sizeof(write_items_buf[0]),
  };
  const tikv_async_raw_mutation_t cleanup_items_buf[] = {
      {.op = TIKV_ASYNC_RAW_MUTATION_OP_DELETE,
       .key = key_a,
       .value = {NULL, 0}},
      {.op = TIKV_ASYNC_RAW_MUTATION_OP_DELETE,
       .key = key_b,
       .value = {NULL, 0}},
  };
  const tikv_async_raw_mutations_t cleanup_items = {
      .ptr = cleanup_items_buf,
      .len = sizeof(cleanup_items_buf) / sizeof(cleanup_items_buf[0]),
  };
  const tikv_slice_t key_items_buf[] = {key_a, key_b};
  const tikv_slices_t key_items = {
      .ptr = key_items_buf,
      .len = sizeof(key_items_buf) / sizeof(key_items_buf[0]),
  };

  tikv_async_client_t *client = NULL;
  tikv_error_t *error = NULL;
  tikv_status_t status = tikv_async_client_new(pd, 5, false, &client, &error);
  if (!expect_ok("tikv_async_client_new", status, error)) {
    return 1;
  }

  request_latch_t reset_latch = {0};
  request_latch_t batch_write_latch = {0};
  request_latch_t batch_get_latch = {0};
  request_latch_t scan_latch = {0};
  request_latch_t reverse_scan_latch = {0};
  request_latch_t cleanup_latch = {0};
  request_latch_t final_scan_latch = {0};

  tikv_async_request_t *reset = NULL;
  tikv_async_request_t *batch_write = NULL;
  tikv_async_request_t *batch_get = NULL;
  tikv_async_request_t *scan = NULL;
  tikv_async_request_t *reverse_scan = NULL;
  tikv_async_request_t *cleanup = NULL;
  tikv_async_request_t *final_scan = NULL;

  status = tikv_async_raw_delete_range_submit(
      client, scan_start, scan_end, completion_cb, &reset_latch, &reset, &error);
  if (!expect_ok("delete_range_submit before batch_write", status, error) ||
      !wait_latch_ok("wait delete_range_submit before batch_write", reset,
                     &reset_latch)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(reset);
    return 1;
  }
  tikv_async_request_destroy(reset);

  status = tikv_async_raw_batch_write_submit(
      client, write_items, completion_cb, &batch_write_latch, &batch_write, &error);
  if (!expect_ok("batch_write_submit", status, error) ||
      !wait_latch_ok("wait batch_write_submit", batch_write, &batch_write_latch)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(batch_write);
    return 1;
  }
  tikv_async_request_destroy(batch_write);

  status = tikv_async_raw_batch_get_submit(
      client, key_items, completion_cb, &batch_get_latch, &batch_get, &error);
  if (!expect_ok("batch_get_submit", status, error)) {
    tikv_async_client_destroy(client);
    return 1;
  }
  status = tikv_async_raw_scan_submit(
      client, scan_start, scan_end, 16, completion_cb, &scan_latch, &scan, &error);
  if (!expect_ok("scan_submit", status, error)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(batch_get);
    return 1;
  }
  status = tikv_async_raw_scan_reverse_submit(client,
                                              scan_start,
                                              scan_end,
                                              16,
                                              completion_cb,
                                              &reverse_scan_latch,
                                              &reverse_scan,
                                              &error);
  if (!expect_ok("scan_reverse_submit", status, error)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(batch_get);
    tikv_async_request_destroy(scan);
    return 1;
  }

  if (!wait_latch_ok("wait batch_get_submit", batch_get, &batch_get_latch) ||
      !wait_latch_ok("wait scan_submit", scan, &scan_latch) ||
      !wait_latch_ok("wait scan_reverse_submit", reverse_scan,
                     &reverse_scan_latch)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(batch_get);
    tikv_async_request_destroy(scan);
    tikv_async_request_destroy(reverse_scan);
    return 1;
  }

  tikv_kv_pairs_t pairs = {0};
  status = tikv_async_request_take_batch_get_result(batch_get, &pairs, &error);
  if (!expect_ok("take batch_get_result", status, error)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(batch_get);
    tikv_async_request_destroy(scan);
    tikv_async_request_destroy(reverse_scan);
    return 1;
  }
  printf("async batch_get returned %zu pairs\n", pairs.len);
  for (size_t i = 0; i < pairs.len; ++i) {
    printf("  get[%zu] key=", i);
    print_bytes(pairs.ptr[i].key.ptr, pairs.ptr[i].key.len);
    printf(" value=");
    print_bytes(pairs.ptr[i].value.ptr, pairs.ptr[i].value.len);
    putchar('\n');
  }
  tikv_kv_pairs_destroy(pairs);
  tikv_async_request_destroy(batch_get);

  pairs.ptr = NULL;
  pairs.len = 0;
  status = tikv_async_request_take_scan_result(scan, &pairs, &error);
  if (!expect_ok("take scan_result", status, error)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(scan);
    tikv_async_request_destroy(reverse_scan);
    return 1;
  }
  printf("async scan returned %zu pairs\n", pairs.len);
  for (size_t i = 0; i < pairs.len; ++i) {
    printf("  scan[%zu] key=", i);
    print_bytes(pairs.ptr[i].key.ptr, pairs.ptr[i].key.len);
    printf(" value=");
    print_bytes(pairs.ptr[i].value.ptr, pairs.ptr[i].value.len);
    putchar('\n');
  }
  tikv_kv_pairs_destroy(pairs);
  tikv_async_request_destroy(scan);

  pairs.ptr = NULL;
  pairs.len = 0;
  status = tikv_async_request_take_scan_result(reverse_scan, &pairs, &error);
  if (!expect_ok("take reverse_scan_result", status, error)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(reverse_scan);
    return 1;
  }
  printf("async reverse scan returned %zu pairs\n", pairs.len);
  for (size_t i = 0; i < pairs.len; ++i) {
    printf("  reverse[%zu] key=", i);
    print_bytes(pairs.ptr[i].key.ptr, pairs.ptr[i].key.len);
    printf(" value=");
    print_bytes(pairs.ptr[i].value.ptr, pairs.ptr[i].value.len);
    putchar('\n');
  }
  tikv_kv_pairs_destroy(pairs);
  tikv_async_request_destroy(reverse_scan);

  status = tikv_async_raw_batch_write_submit(
      client, cleanup_items, completion_cb, &cleanup_latch, &cleanup, &error);
  if (!expect_ok("cleanup batch_write_submit", status, error) ||
      !wait_latch_ok("wait cleanup batch_write_submit", cleanup, &cleanup_latch)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(cleanup);
    return 1;
  }
  tikv_async_request_destroy(cleanup);

  status = tikv_async_raw_scan_submit(client,
                                      scan_start,
                                      scan_end,
                                      16,
                                      completion_cb,
                                      &final_scan_latch,
                                      &final_scan,
                                      &error);
  if (!expect_ok("final scan_submit", status, error) ||
      !wait_latch_ok("wait final scan_submit", final_scan, &final_scan_latch)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(final_scan);
    return 1;
  }

  pairs.ptr = NULL;
  pairs.len = 0;
  status = tikv_async_request_take_scan_result(final_scan, &pairs, &error);
  if (!expect_ok("take final scan_result", status, error)) {
    tikv_async_client_destroy(client);
    tikv_async_request_destroy(final_scan);
    return 1;
  }
  printf("async scan after cleanup returned %zu pairs\n", pairs.len);
  tikv_kv_pairs_destroy(pairs);
  tikv_async_request_destroy(final_scan);

  status = tikv_async_client_shutdown_begin(client, &error);
  if (!expect_ok("shutdown_begin", status, error)) {
    tikv_async_client_destroy(client);
    return 1;
  }
  return expect_ok("client_destroy", tikv_async_client_destroy(client), NULL) ? 0 : 1;
}
