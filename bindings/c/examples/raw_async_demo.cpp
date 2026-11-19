// Copyright (c) 2026 PingCAP, Inc.

#include "tikv_client_async.hpp"

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <thread>
#include <utility>
#include <vector>

namespace {

using tikv::async::AsyncRequest;
using tikv::async::Client;
using tikv::async::KvPairs;
using tikv::async::Mutation;
using tikv::async::MutationWithTtl;
using tikv::async::OptionalKeyTtl;
using tikv::async::OptionalValue;
using tikv::async::Result;
using tikv::async::Slice;
using tikv::async::Status;
using tikv::async::Unit;

bool expect_ok(const char *label, const Status &status) {
  if (status.ok()) {
    return true;
  }

  std::cerr << label << ": " << status.to_string() << '\n';
  return false;
}

bool parse_bool_arg(const char *value) {
  return std::strcmp(value, "1") == 0 || std::strcmp(value, "true") == 0 ||
         std::strcmp(value, "TRUE") == 0 ||
         std::strcmp(value, "default-keyspace") == 0;
}

template <typename T>
bool expect_submitted(const char *label, const AsyncRequest<T> &request) {
  if (request.submitted()) {
    return true;
  }

  std::cerr << label << ": " << request.submit_status().to_string() << '\n';
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

void print_bytes(Slice bytes) {
  if (bytes.size() == 0) {
    return;
  }
  std::cout.write(bytes.data(), static_cast<std::streamsize>(bytes.size()));
}

void print_pairs(const char *prefix, const KvPairs &pairs) {
  for (std::size_t i = 0; i < pairs.size(); ++i) {
    const auto pair = pairs[i];
    std::cout << "  " << prefix << '[' << i << "] key=";
    print_bytes(pair.key);
    std::cout << " value=";
    print_bytes(pair.value);
    std::cout << '\n';
  }
}

bool wait_unit(const char *submit_label,
               const char *wait_label,
               AsyncRequest<Unit> &request) {
  return expect_submitted(submit_label, request) &&
         expect_ok(wait_label, request.wait());
}

bool read_key_ttl(Client &client,
                  Slice key,
                  const char *submit_label,
                  const char *wait_label,
                  OptionalKeyTtl &ttl) {
  auto request = client.get_key_ttl_secs(key);
  if (!expect_submitted(submit_label, request)) {
    return false;
  }

  auto result = request.wait();
  if (!expect_result(wait_label, result)) {
    return false;
  }

  ttl = result.take();
  return true;
}

bool read_key_presence(Client &client,
                       Slice key,
                       const char *submit_label,
                       const char *wait_label,
                       bool &present) {
  auto request = client.get(key);
  if (!expect_submitted(submit_label, request)) {
    return false;
  }

  auto result = request.wait();
  if (!expect_result(wait_label, result)) {
    return false;
  }

  OptionalValue value = result.take();
  present = value.has_value();
  return true;
}

bool wait_for_keys_absent(Client &client,
                          const std::vector<Slice> &keys,
                          std::chrono::seconds timeout) {
  const auto deadline = std::chrono::steady_clock::now() + timeout;

  for (;;) {
    bool any_present = false;
    for (Slice key : keys) {
      bool present = false;
      if (!read_key_presence(client,
                             key,
                             "TTL expiry get_submit",
                             "wait TTL expiry get_submit",
                             present)) {
        return false;
      }
      any_present = any_present || present;
    }

    if (!any_present) {
      return true;
    }
    if (std::chrono::steady_clock::now() >= deadline) {
      std::cerr << "TTL keys did not expire within " << timeout.count()
                << " seconds\n";
      return false;
    }

    std::this_thread::sleep_for(std::chrono::milliseconds(200));
  }
}

bool expect_positive_ttl(const char *label,
                         const OptionalKeyTtl &ttl,
                         std::uint64_t configured_ttl_secs) {
  if (ttl.has_value() && *ttl > 0 && *ttl <= configured_ttl_secs) {
    return true;
  }

  std::cerr << label << ": expected remaining TTL in [1, "
            << configured_ttl_secs << "] seconds\n";
  return false;
}

bool expect_persistent_ttl(const char *label, const OptionalKeyTtl &ttl) {
  if (ttl.has_value() && *ttl == 0) {
    return true;
  }

  std::cerr << label << ": expected an existing key without TTL\n";
  return false;
}

bool run_ttl_integration(Client &client) {
  constexpr std::uint64_t kExpirySecs = 3;
  const Slice single_expiring_key("tikv-client-rust-c/ttl/single-expiring");
  const Slice ttl_cleared_key("tikv-client-rust-c/ttl/cleared");
  const Slice batch_expiring_key("tikv-client-rust-c/ttl/batch-expiring");
  const Slice batch_persistent_key("tikv-client-rust-c/ttl/batch-persistent");
  const Slice batch_deleted_key("tikv-client-rust-c/ttl/batch-deleted");
  const Slice value("ttl-value");

  std::cout << "running native TTL integration test\n";

  auto put_with_ttl =
      client.put_with_ttl(single_expiring_key, value, kExpirySecs);
  if (!wait_unit("put_with_ttl_submit",
                 "wait put_with_ttl_submit",
                 put_with_ttl)) {
    return false;
  }

  OptionalKeyTtl ttl;
  if (!read_key_ttl(client,
                    single_expiring_key,
                    "get_key_ttl_submit for single key",
                    "wait get_key_ttl_submit for single key",
                    ttl) ||
      !expect_positive_ttl("single-key TTL", ttl, kExpirySecs)) {
    return false;
  }

  auto put_to_clear =
      client.put_with_ttl(ttl_cleared_key, value, kExpirySecs);
  if (!wait_unit("put_with_ttl_submit before clearing TTL",
                 "wait put_with_ttl_submit before clearing TTL",
                 put_to_clear)) {
    return false;
  }

  auto clear_ttl = client.put_with_ttl(ttl_cleared_key, value, 0);
  if (!wait_unit("put_with_ttl_submit clearing TTL",
                 "wait put_with_ttl_submit clearing TTL",
                 clear_ttl) ||
      !read_key_ttl(client,
                    ttl_cleared_key,
                    "get_key_ttl_submit after clearing TTL",
                    "wait get_key_ttl_submit after clearing TTL",
                    ttl) ||
      !expect_persistent_ttl("cleared TTL", ttl)) {
    return false;
  }

  auto seed_batch_delete = client.put(batch_deleted_key, value);
  if (!wait_unit("put_submit before TTL batch delete",
                 "wait put_submit before TTL batch delete",
                 seed_batch_delete)) {
    return false;
  }

  auto batch_write = client.batch_write_with_ttl({
      MutationWithTtl::put(batch_expiring_key, value, kExpirySecs),
      MutationWithTtl::put(batch_persistent_key, value, 0),
      MutationWithTtl::remove(batch_deleted_key),
  });
  if (!wait_unit("batch_write_with_ttl_submit",
                 "wait batch_write_with_ttl_submit",
                 batch_write)) {
    return false;
  }

  if (!read_key_ttl(client,
                    batch_expiring_key,
                    "get_key_ttl_submit for TTL batch key",
                    "wait get_key_ttl_submit for TTL batch key",
                    ttl) ||
      !expect_positive_ttl("batch-key TTL", ttl, kExpirySecs) ||
      !read_key_ttl(client,
                    batch_persistent_key,
                    "get_key_ttl_submit for persistent batch key",
                    "wait get_key_ttl_submit for persistent batch key",
                    ttl) ||
      !expect_persistent_ttl("persistent batch-key TTL", ttl)) {
    return false;
  }

  bool present = false;
  if (!read_key_presence(client,
                         batch_deleted_key,
                         "get_submit for TTL batch-deleted key",
                         "wait get_submit for TTL batch-deleted key",
                         present)) {
    return false;
  }
  if (present) {
    std::cerr << "TTL batch delete did not remove its key\n";
    return false;
  }

  if (!wait_for_keys_absent(
          client,
          {single_expiring_key, batch_expiring_key},
          std::chrono::seconds(10))) {
    return false;
  }

  if (!read_key_ttl(client,
                    single_expiring_key,
                    "get_key_ttl_submit after expiry",
                    "wait get_key_ttl_submit after expiry",
                    ttl)) {
    return false;
  }
  if (ttl.has_value()) {
    std::cerr << "expired key still has a TTL result\n";
    return false;
  }

  for (Slice key : {ttl_cleared_key, batch_persistent_key}) {
    if (!read_key_presence(client,
                           key,
                           "get_submit for persistent TTL key",
                           "wait get_submit for persistent TTL key",
                           present)) {
      return false;
    }
    if (!present) {
      std::cerr << "persistent TTL test key unexpectedly disappeared\n";
      return false;
    }
  }

  auto cleanup = client.batch_write({
      Mutation::remove(ttl_cleared_key),
      Mutation::remove(batch_persistent_key),
  });
  if (!wait_unit("cleanup TTL batch_write_submit",
                 "wait cleanup TTL batch_write_submit",
                 cleanup)) {
    return false;
  }

  std::cout << "native TTL integration test passed\n";
  return true;
}

} // namespace

int main(int argc, char **argv) {
  const char *pd = argc > 1 ? argv[1] : "127.0.0.1:2379";
  const bool use_default_keyspace = argc > 2 && parse_bool_arg(argv[2]);
  const bool run_ttl_tests = argc > 3 && parse_bool_arg(argv[3]);
  const Slice scan_start("tikv-client-rust-c/");
  const Slice scan_end("tikv-client-rust-c0");
  const Slice key_a("tikv-client-rust-c/a");
  const Slice key_b("tikv-client-rust-c/b");
  const Slice value_a_v1("value-a-v1");
  const Slice value_a_v2("value-a-v2");
  const Slice value_b("value-b");

  std::vector<Mutation> write_items = {
      Mutation::put(key_a, value_a_v1),
      Mutation::put(key_b, value_b),
      Mutation::remove(key_a),
      Mutation::put(key_a, value_a_v2),
  };
  std::vector<Mutation> cleanup_items = {
      Mutation::remove(key_a),
      Mutation::remove(key_b),
  };
  std::vector<Slice> key_items = {key_a, key_b};

  auto client_result = Client::connect(pd, 5, use_default_keyspace);
  if (!expect_result("tikv_async_client_new", client_result)) {
    return 1;
  }
  Client client = client_result.take();

  auto reset = client.delete_range(scan_start, scan_end);
  if (!wait_unit("delete_range_submit before batch_write",
                 "wait delete_range_submit before batch_write",
                 reset)) {
    return 1;
  }

  auto batch_write = client.batch_write(write_items);
  if (!wait_unit("batch_write_submit",
                 "wait batch_write_submit",
                 batch_write)) {
    return 1;
  }

  auto batch_get = client.batch_get(key_items);
  if (!expect_submitted("batch_get_submit", batch_get)) {
    return 1;
  }
  auto scan = client.scan(scan_start, scan_end, 16);
  if (!expect_submitted("scan_submit", scan)) {
    return 1;
  }
  auto reverse_scan = client.scan_reverse(scan_start, scan_end, 16);
  if (!expect_submitted("scan_reverse_submit", reverse_scan)) {
    return 1;
  }

  if (!expect_ok("wait batch_get_submit", batch_get.wait_status()) ||
      !expect_ok("wait scan_submit", scan.wait_status()) ||
      !expect_ok("wait scan_reverse_submit", reverse_scan.wait_status())) {
    return 1;
  }

  auto batch_get_result = batch_get.take();
  if (!expect_result("take batch_get_result", batch_get_result)) {
    return 1;
  }
  KvPairs batch_get_pairs = batch_get_result.take();
  std::cout << "async batch_get returned " << batch_get_pairs.size()
            << " pairs\n";
  print_pairs("get", batch_get_pairs);

  auto scan_result = scan.take();
  if (!expect_result("take scan_result", scan_result)) {
    return 1;
  }
  KvPairs scan_pairs = scan_result.take();
  std::cout << "async scan returned " << scan_pairs.size() << " pairs\n";
  print_pairs("scan", scan_pairs);

  auto reverse_scan_result = reverse_scan.take();
  if (!expect_result("take reverse_scan_result", reverse_scan_result)) {
    return 1;
  }
  KvPairs reverse_scan_pairs = reverse_scan_result.take();
  std::cout << "async reverse scan returned " << reverse_scan_pairs.size()
            << " pairs\n";
  print_pairs("reverse", reverse_scan_pairs);

  auto cleanup = client.batch_write(cleanup_items);
  if (!wait_unit("cleanup batch_write_submit",
                 "wait cleanup batch_write_submit",
                 cleanup)) {
    return 1;
  }

  if (run_ttl_tests && !run_ttl_integration(client)) {
    return 1;
  }

  auto final_scan = client.scan(scan_start, scan_end, 16);
  if (!expect_submitted("final scan_submit", final_scan) ||
      !expect_ok("wait final scan_submit", final_scan.wait_status())) {
    return 1;
  }

  auto final_scan_result = final_scan.take();
  if (!expect_result("take final scan_result", final_scan_result)) {
    return 1;
  }
  KvPairs final_pairs = final_scan_result.take();
  std::cout << "async scan after cleanup returned " << final_pairs.size()
            << " pairs\n";

  Status status = client.shutdown_begin(false);
  if (!expect_ok("shutdown_begin", status)) {
    return 1;
  }
  status = client.drain();
  if (!expect_ok("drain", status)) {
    return 1;
  }

  return 0;
}
