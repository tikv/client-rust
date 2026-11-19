// Copyright (c) 2026 PingCAP, Inc.

#include "tikv_client_async.hpp"
#include "tikv_client_metrics.hpp"

#include <cstring>
#include <iostream>
#include <utility>

namespace {

using tikv::async::AsyncRequest;
using tikv::async::Client;
using tikv::async::OptionalValue;
using tikv::async::Result;
using tikv::async::Slice;
using tikv::async::Status;
using tikv::async::Unit;
using tikv::metrics::MetricSample;
using tikv::metrics::MetricsSnapshot;
namespace metric_names = tikv::metrics::metric_names;
namespace metric_type_labels = tikv::metrics::metric_type_labels;

bool expect_ok(const char *label, const Status &status) {
  if (status.ok()) {
    return true;
  }

  std::cerr << label << ": " << status.to_string() << '\n';
  return false;
}

template <typename T>
bool expect_result(const char *label, const Result<T> &result) {
  if (result.ok()) {
    return true;
  }

  std::cerr << label << ": " << result.status().to_string() << '\n';
  return false;
}

template <typename T>
bool expect_submitted(const char *label, const AsyncRequest<T> &request) {
  if (request.submitted()) {
    return true;
  }

  std::cerr << label << ": " << request.submit_status().to_string() << '\n';
  return false;
}

bool wait_unit(const char *submit_label,
               const char *wait_label,
               AsyncRequest<Unit> &request) {
  return expect_submitted(submit_label, request) &&
         expect_ok(wait_label, request.wait());
}

bool parse_bool_arg(const char *value) {
  return std::strcmp(value, "1") == 0 || std::strcmp(value, "true") == 0 ||
         std::strcmp(value, "TRUE") == 0 ||
         std::strcmp(value, "default-keyspace") == 0;
}

bool run_raw_get(Client *client) {
  const Slice key("ffi-metrics-demo/key");
  const Slice value("ffi-metrics-demo/value");

  auto cleanup_before = client->remove(key);
  if (!wait_unit("raw_delete_submit before put",
                 "wait raw_delete before put",
                 cleanup_before)) {
    return false;
  }

  auto put = client->put(key, value);
  if (!wait_unit("raw_put_submit", "wait raw_put", put)) {
    return false;
  }

  auto get = client->get(key);
  if (!expect_submitted("raw_get_submit", get)) {
    return false;
  }

  auto get_result = get.wait();
  if (!expect_result("wait raw_get", get_result)) {
    return false;
  }

  OptionalValue found = get_result.take();
  if (!found.has_value()) {
    std::cerr << "raw_get returned no value\n";
    return false;
  }

  std::cout << "raw_get value=" << found->to_string() << '\n';

  auto cleanup_after = client->remove(key);
  return wait_unit("raw_delete_submit after get",
                   "wait raw_delete after get",
                   cleanup_after);
}

}  // namespace

int main(int argc, char **argv) {
  const char *pd = argc > 1 ? argv[1] : "127.0.0.1:2379";
  const bool use_default_keyspace = argc > 2 && parse_bool_arg(argv[2]);

  auto client_result = Client::connect(pd, 5, use_default_keyspace);
  if (!expect_result("tikv_async_client_new", client_result)) {
    return 1;
  }

  Client client = client_result.take();
  if (!run_raw_get(&client)) {
    return 1;
  }

  auto snapshot_result = MetricsSnapshot::collect();
  if (!expect_result("tikv_metrics_snapshot_collect", snapshot_result)) {
    return 1;
  }
  MetricsSnapshot snapshot = snapshot_result.take();

  bool found_raw_get_total = false;
  bool found_raw_get_duration_bucket = false;
  bool found_raw_get_duration_count = false;

  for (auto sample_result : snapshot) {
    if (!expect_result("tikv_metrics_snapshot_get", sample_result)) {
      return 1;
    }
    MetricSample sample = sample_result.take();

    if (sample.is_counter(metric_names::TikvRequestTotal,
                          metric_type_labels::RawGet)) {
      found_raw_get_total = true;
      std::cout << metric_names::TikvRequestTotal.value() << "{type=\""
                << metric_type_labels::RawGet.value() << "\"} "
                << sample.value << '\n';
    }

    if (sample.is_histogram_bucket(
            metric_names::TikvRequestDurationSeconds,
            metric_type_labels::RawGet)) {
      found_raw_get_duration_bucket = true;
      std::cout << metric_names::TikvRequestDurationSeconds.value()
                << "_bucket{type=\"" << metric_type_labels::RawGet.value()
                << "\",le=\"" << sample.le << "\"} " << sample.value
                << '\n';
    }

    if (sample.is_histogram_count(
            metric_names::TikvRequestDurationSeconds,
            metric_type_labels::RawGet)) {
      found_raw_get_duration_count = true;
      std::cout << metric_names::TikvRequestDurationSeconds.value()
                << "_count{type=\"" << metric_type_labels::RawGet.value()
                << "\"} " << sample.value << '\n';
    }
  }

  if (!found_raw_get_total) {
    std::cerr << "No raw_get request counter sample was present.\n";
  }
  if (!found_raw_get_duration_bucket) {
    std::cerr << "No raw_get request duration bucket was present.\n";
  }
  if (!found_raw_get_duration_count) {
    std::cerr << "No raw_get request duration count was present.\n";
  }

  Status status = client.shutdown_begin(false);
  if (!expect_ok("shutdown_begin", status)) {
    return 1;
  }
  status = client.drain();
  if (!expect_ok("drain", status)) {
    return 1;
  }

  return found_raw_get_total && found_raw_get_duration_bucket &&
                 found_raw_get_duration_count
             ? 0
             : 1;
}
