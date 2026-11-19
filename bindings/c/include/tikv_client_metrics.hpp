// Copyright (c) 2026 PingCAP, Inc.

#ifndef TIKV_CLIENT_METRICS_HPP
#define TIKV_CLIENT_METRICS_HPP

#include "tikv_client_async.hpp"
#include "tikv_client_metrics_c.h"

#include <cstddef>
#include <iterator>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace tikv {
namespace metrics {

class MetricName final {
public:
  constexpr explicit MetricName(std::string_view value) noexcept
      : value_(value) {}

  constexpr std::string_view value() const noexcept { return value_; }

  friend constexpr bool operator==(MetricName lhs, MetricName rhs) noexcept {
    return lhs.value_ == rhs.value_;
  }

  friend constexpr bool operator!=(MetricName lhs, MetricName rhs) noexcept {
    return !(lhs == rhs);
  }

private:
  std::string_view value_;
};

class MetricTypeLabel final {
public:
  constexpr explicit MetricTypeLabel(std::string_view value) noexcept
      : value_(value) {}

  constexpr std::string_view value() const noexcept { return value_; }

  friend constexpr bool operator==(MetricTypeLabel lhs,
                                   MetricTypeLabel rhs) noexcept {
    return lhs.value_ == rhs.value_;
  }

  friend constexpr bool operator!=(MetricTypeLabel lhs,
                                   MetricTypeLabel rhs) noexcept {
    return !(lhs == rhs);
  }

private:
  std::string_view value_;
};

namespace metric_names {

inline constexpr MetricName TikvRequestTotal{
    TIKV_METRIC_TIKV_REQUEST_TOTAL};
inline constexpr MetricName TikvRequestDurationSeconds{
    TIKV_METRIC_TIKV_REQUEST_DURATION_SECONDS};
inline constexpr MetricName TikvFailedRequestTotal{
    TIKV_METRIC_TIKV_FAILED_REQUEST_TOTAL};
inline constexpr MetricName TikvFailedRequestDurationSeconds{
    TIKV_METRIC_TIKV_FAILED_REQUEST_DURATION_SECONDS};
inline constexpr MetricName PdRequestTotal{TIKV_METRIC_PD_REQUEST_TOTAL};
inline constexpr MetricName PdRequestDurationSeconds{
    TIKV_METRIC_PD_REQUEST_DURATION_SECONDS};
inline constexpr MetricName PdFailedRequestTotal{
    TIKV_METRIC_PD_FAILED_REQUEST_TOTAL};
inline constexpr MetricName PdFailedRequestDurationSeconds{
    TIKV_METRIC_PD_FAILED_REQUEST_DURATION_SECONDS};
inline constexpr MetricName PdTsoBatchSize{TIKV_METRIC_PD_TSO_BATCH_SIZE};

} // namespace metric_names

namespace metric_type_labels {

inline constexpr MetricTypeLabel RawGet{TIKV_METRIC_LABEL_TYPE_RAW_GET};
inline constexpr MetricTypeLabel RawBatchGet{
    TIKV_METRIC_LABEL_TYPE_RAW_BATCH_GET};
inline constexpr MetricTypeLabel RawGetKeyTtl{
    TIKV_METRIC_LABEL_TYPE_RAW_GET_KEY_TTL};
inline constexpr MetricTypeLabel RawPut{TIKV_METRIC_LABEL_TYPE_RAW_PUT};
inline constexpr MetricTypeLabel RawBatchPut{
    TIKV_METRIC_LABEL_TYPE_RAW_BATCH_PUT};
inline constexpr MetricTypeLabel RawDelete{
    TIKV_METRIC_LABEL_TYPE_RAW_DELETE};
inline constexpr MetricTypeLabel RawBatchDelete{
    TIKV_METRIC_LABEL_TYPE_RAW_BATCH_DELETE};
inline constexpr MetricTypeLabel RawScan{TIKV_METRIC_LABEL_TYPE_RAW_SCAN};
inline constexpr MetricTypeLabel RawBatchScan{
    TIKV_METRIC_LABEL_TYPE_RAW_BATCH_SCAN};
inline constexpr MetricTypeLabel RawDeleteRange{
    TIKV_METRIC_LABEL_TYPE_RAW_DELETE_RANGE};
inline constexpr MetricTypeLabel RawCompareAndSwap{
    TIKV_METRIC_LABEL_TYPE_RAW_COMPARE_AND_SWAP};
inline constexpr MetricTypeLabel RawCoprocessor{
    TIKV_METRIC_LABEL_TYPE_RAW_COPROCESSOR};
inline constexpr MetricTypeLabel KvGet{TIKV_METRIC_LABEL_TYPE_KV_GET};
inline constexpr MetricTypeLabel KvScan{TIKV_METRIC_LABEL_TYPE_KV_SCAN};
inline constexpr MetricTypeLabel KvPrewrite{
    TIKV_METRIC_LABEL_TYPE_KV_PREWRITE};
inline constexpr MetricTypeLabel KvCommit{TIKV_METRIC_LABEL_TYPE_KV_COMMIT};
inline constexpr MetricTypeLabel KvBatchGet{
    TIKV_METRIC_LABEL_TYPE_KV_BATCH_GET};
inline constexpr MetricTypeLabel KvBatchRollback{
    TIKV_METRIC_LABEL_TYPE_KV_BATCH_ROLLBACK};
inline constexpr MetricTypeLabel KvPessimisticRollback{
    TIKV_METRIC_LABEL_TYPE_KV_PESSIMISTIC_ROLLBACK};
inline constexpr MetricTypeLabel KvResolveLock{
    TIKV_METRIC_LABEL_TYPE_KV_RESOLVE_LOCK};
inline constexpr MetricTypeLabel KvScanLock{
    TIKV_METRIC_LABEL_TYPE_KV_SCAN_LOCK};
inline constexpr MetricTypeLabel KvPessimisticLock{
    TIKV_METRIC_LABEL_TYPE_KV_PESSIMISTIC_LOCK};
inline constexpr MetricTypeLabel KvTxnHeartBeat{
    TIKV_METRIC_LABEL_TYPE_KV_TXN_HEART_BEAT};
inline constexpr MetricTypeLabel KvCheckTxnStatus{
    TIKV_METRIC_LABEL_TYPE_KV_CHECK_TXN_STATUS};
inline constexpr MetricTypeLabel KvCheckSecondaryLocksRequest{
    TIKV_METRIC_LABEL_TYPE_KV_CHECK_SECONDARY_LOCKS_REQUEST};
inline constexpr MetricTypeLabel KvGc{TIKV_METRIC_LABEL_TYPE_KV_GC};
inline constexpr MetricTypeLabel KvDeleteRange{
    TIKV_METRIC_LABEL_TYPE_KV_DELETE_RANGE};
inline constexpr MetricTypeLabel UnsafeDestroyRange{
    TIKV_METRIC_LABEL_TYPE_UNSAFE_DESTROY_RANGE};
inline constexpr MetricTypeLabel PdGetRegion{
    TIKV_METRIC_LABEL_TYPE_PD_GET_REGION};
inline constexpr MetricTypeLabel PdGetRegionById{
    TIKV_METRIC_LABEL_TYPE_PD_GET_REGION_BY_ID};
inline constexpr MetricTypeLabel PdGetStore{
    TIKV_METRIC_LABEL_TYPE_PD_GET_STORE};
inline constexpr MetricTypeLabel PdGetAllStores{
    TIKV_METRIC_LABEL_TYPE_PD_GET_ALL_STORES};
inline constexpr MetricTypeLabel PdGetTimestamp{
    TIKV_METRIC_LABEL_TYPE_PD_GET_TIMESTAMP};
inline constexpr MetricTypeLabel PdUpdateGcSafepoint{
    TIKV_METRIC_LABEL_TYPE_PD_UPDATE_GC_SAFEPOINT};
inline constexpr MetricTypeLabel PdLoadKeyspace{
    TIKV_METRIC_LABEL_TYPE_PD_LOAD_KEYSPACE};

} // namespace metric_type_labels

using MetricSampleKind = ::tikv_metric_sample_kind_t;

namespace metric_sample_kinds {

inline constexpr MetricSampleKind Unknown = TIKV_METRIC_SAMPLE_UNKNOWN;
inline constexpr MetricSampleKind Counter = TIKV_METRIC_SAMPLE_COUNTER;
inline constexpr MetricSampleKind HistogramBucket =
    TIKV_METRIC_SAMPLE_HISTOGRAM_BUCKET;
inline constexpr MetricSampleKind HistogramSum =
    TIKV_METRIC_SAMPLE_HISTOGRAM_SUM;
inline constexpr MetricSampleKind HistogramCount =
    TIKV_METRIC_SAMPLE_HISTOGRAM_COUNT;

} // namespace metric_sample_kinds

struct MetricSample {
  std::string name;
  std::optional<std::string> type_label;
  MetricSampleKind kind = metric_sample_kinds::Unknown;
  double value = 0.0;
  double le = 0.0;

  static MetricSample from_c(const tikv_metric_sample_t &sample) {
    MetricSample result;
    if (sample.name != nullptr) {
      result.name = sample.name;
    }
    if (sample.type_label != nullptr) {
      result.type_label = sample.type_label;
    }
    result.kind = sample.kind;
    result.value = sample.value;
    result.le = sample.le;
    return result;
  }

  const std::string &name_string() const noexcept { return name; }

  const std::optional<std::string> &type_label_string() const noexcept {
    return type_label;
  }

  bool kind_is(MetricSampleKind expected) const noexcept {
    return kind == expected;
  }

  bool name_is(MetricName expected) const noexcept {
    return std::string_view(name.data(), name.size()) == expected.value();
  }

  bool has_no_type_label() const noexcept {
    return !type_label.has_value();
  }

  bool type_label_is(MetricTypeLabel expected) const noexcept {
    return type_label.has_value() &&
           std::string_view(type_label->data(), type_label->size()) ==
               expected.value();
  }

  bool type_label_is(std::optional<MetricTypeLabel> expected) const noexcept {
    if (!expected.has_value()) {
      return has_no_type_label();
    }
    return type_label_is(*expected);
  }

  bool is_counter(MetricName expected_name) const noexcept {
    return kind_is(metric_sample_kinds::Counter) && name_is(expected_name);
  }

  bool is_counter(MetricName expected_name,
                  MetricTypeLabel expected_label) const noexcept {
    return is_counter(expected_name) && type_label_is(expected_label);
  }

  bool is_counter(MetricName expected_name,
                  std::optional<MetricTypeLabel> expected_label) const noexcept {
    return is_counter(expected_name) && type_label_is(expected_label);
  }

  bool is_histogram_bucket(MetricName expected_name) const noexcept {
    return kind_is(metric_sample_kinds::HistogramBucket) &&
           name_is(expected_name);
  }

  bool is_histogram_bucket(MetricName expected_name,
                           MetricTypeLabel expected_label) const noexcept {
    return is_histogram_bucket(expected_name) && type_label_is(expected_label);
  }

  bool is_histogram_bucket(
      MetricName expected_name,
      std::optional<MetricTypeLabel> expected_label) const noexcept {
    return is_histogram_bucket(expected_name) && type_label_is(expected_label);
  }

  bool is_histogram_sum(MetricName expected_name) const noexcept {
    return kind_is(metric_sample_kinds::HistogramSum) &&
           name_is(expected_name);
  }

  bool is_histogram_sum(MetricName expected_name,
                        MetricTypeLabel expected_label) const noexcept {
    return is_histogram_sum(expected_name) && type_label_is(expected_label);
  }

  bool is_histogram_sum(
      MetricName expected_name,
      std::optional<MetricTypeLabel> expected_label) const noexcept {
    return is_histogram_sum(expected_name) && type_label_is(expected_label);
  }

  bool is_histogram_count(MetricName expected_name) const noexcept {
    return kind_is(metric_sample_kinds::HistogramCount) &&
           name_is(expected_name);
  }

  bool is_histogram_count(MetricName expected_name,
                          MetricTypeLabel expected_label) const noexcept {
    return is_histogram_count(expected_name) && type_label_is(expected_label);
  }

  bool is_histogram_count(
      MetricName expected_name,
      std::optional<MetricTypeLabel> expected_label) const noexcept {
    return is_histogram_count(expected_name) && type_label_is(expected_label);
  }
};

class MetricSampleView : public ::tikv::async::BorrowedResultValueTag {
public:
  explicit MetricSampleView(const tikv_metric_sample_t &sample) noexcept
      : sample_(sample) {}

  const tikv_metric_sample_t &raw() const noexcept { return sample_; }

  std::string_view name() const noexcept {
    if (sample_.name == nullptr) {
      return {};
    }
    return std::string_view(sample_.name);
  }

  std::optional<std::string_view> type_label() const noexcept {
    if (sample_.type_label == nullptr) {
      return std::nullopt;
    }
    return std::string_view(sample_.type_label);
  }

  MetricSampleKind kind() const noexcept {
    return sample_.kind;
  }

  bool kind_is(MetricSampleKind kind) const noexcept {
    return sample_.kind == kind;
  }

  bool name_is(MetricName name) const noexcept {
    return sample_.name != nullptr &&
           std::string_view(sample_.name) == name.value();
  }

  bool has_no_type_label() const noexcept {
    return sample_.type_label == nullptr;
  }

  bool type_label_is(MetricTypeLabel label) const noexcept {
    return type_label_is(std::optional<MetricTypeLabel>(label));
  }

  bool type_label_is(std::optional<MetricTypeLabel> label) const noexcept {
    if (!label.has_value()) {
      return has_no_type_label();
    }
    return sample_.type_label != nullptr &&
           std::string_view(sample_.type_label) == label->value();
  }

  bool is_counter(MetricName name) const noexcept {
    return kind_is(metric_sample_kinds::Counter) && name_is(name);
  }

  bool is_counter(MetricName name, MetricTypeLabel label) const noexcept {
    return is_counter(name) && type_label_is(label);
  }

  bool is_counter(MetricName name,
                  std::optional<MetricTypeLabel> label) const noexcept {
    return is_counter(name) && type_label_is(label);
  }

  bool is_histogram_bucket(MetricName name) const noexcept {
    return kind_is(metric_sample_kinds::HistogramBucket) && name_is(name);
  }

  bool is_histogram_bucket(MetricName name,
                           MetricTypeLabel label) const noexcept {
    return is_histogram_bucket(name) && type_label_is(label);
  }

  bool is_histogram_bucket(MetricName name,
                           std::optional<MetricTypeLabel> label) const noexcept {
    return is_histogram_bucket(name) && type_label_is(label);
  }

  bool is_histogram_sum(MetricName name) const noexcept {
    return kind_is(metric_sample_kinds::HistogramSum) && name_is(name);
  }

  bool is_histogram_sum(MetricName name, MetricTypeLabel label) const noexcept {
    return is_histogram_sum(name) && type_label_is(label);
  }

  bool is_histogram_sum(MetricName name,
                        std::optional<MetricTypeLabel> label) const noexcept {
    return is_histogram_sum(name) && type_label_is(label);
  }

  bool is_histogram_count(MetricName name) const noexcept {
    return kind_is(metric_sample_kinds::HistogramCount) && name_is(name);
  }

  bool is_histogram_count(MetricName name, MetricTypeLabel label) const noexcept {
    return is_histogram_count(name) && type_label_is(label);
  }

  bool is_histogram_count(MetricName name,
                          std::optional<MetricTypeLabel> label) const noexcept {
    return is_histogram_count(name) && type_label_is(label);
  }

  double value() const noexcept { return sample_.value; }

  double le() const noexcept { return sample_.le; }

  MetricSample to_owned() const { return MetricSample::from_c(sample_); }

private:
  tikv_metric_sample_t sample_;
};

class MetricsSnapshot {
public:
  using Result = ::tikv::async::Result<MetricsSnapshot>;
  using SampleResult = ::tikv::async::Result<MetricSample>;
  using SampleViewResult = ::tikv::async::Result<MetricSampleView>;
  using SamplesResult = ::tikv::async::Result<std::vector<MetricSample>>;

  class const_iterator {
  public:
    using iterator_category = std::input_iterator_tag;
    using value_type = SampleResult;
    using difference_type = std::ptrdiff_t;
    using pointer = void;
    using reference = value_type;

    const_iterator() noexcept = default;

    const_iterator(const MetricsSnapshot *snapshot, std::size_t index) noexcept
        : snapshot_(snapshot), index_(index) {}

    value_type operator*() const {
      if (snapshot_ == nullptr) {
        return ::tikv::async::Status::InvalidArgument(
            "metrics snapshot iterator is empty");
      }
      return snapshot_->get(index_);
    }

    const_iterator &operator++() noexcept {
      ++index_;
      return *this;
    }

    const_iterator operator++(int) noexcept {
      const_iterator current(*this);
      ++(*this);
      return current;
    }

    bool operator==(const const_iterator &other) const noexcept {
      return snapshot_ == other.snapshot_ && index_ == other.index_;
    }

    bool operator!=(const const_iterator &other) const noexcept {
      return !(*this == other);
    }

  private:
    const MetricsSnapshot *snapshot_ = nullptr;
    std::size_t index_ = 0;
  };

  class view_iterator {
  public:
    using iterator_category = std::input_iterator_tag;
    using value_type = SampleViewResult;
    using difference_type = std::ptrdiff_t;
    using pointer = void;
    using reference = value_type;

    view_iterator() noexcept = default;

    view_iterator(const MetricsSnapshot *snapshot, std::size_t index) noexcept
        : snapshot_(snapshot), index_(index) {}

    value_type operator*() const {
      if (snapshot_ == nullptr) {
        return ::tikv::async::Status::InvalidArgument(
            "metrics snapshot view iterator is empty");
      }
      return snapshot_->get_view(index_);
    }

    view_iterator &operator++() noexcept {
      ++index_;
      return *this;
    }

    view_iterator operator++(int) noexcept {
      view_iterator current(*this);
      ++(*this);
      return current;
    }

    bool operator==(const view_iterator &other) const noexcept {
      return snapshot_ == other.snapshot_ && index_ == other.index_;
    }

    bool operator!=(const view_iterator &other) const noexcept {
      return !(*this == other);
    }

  private:
    const MetricsSnapshot *snapshot_ = nullptr;
    std::size_t index_ = 0;
  };

  class ViewRange {
  public:
    explicit ViewRange(const MetricsSnapshot *snapshot) noexcept
        : snapshot_(snapshot) {}

    view_iterator begin() const noexcept {
      return view_iterator(snapshot_, 0);
    }

    view_iterator end() const noexcept {
      return view_iterator(snapshot_,
                           snapshot_ == nullptr ? 0 : snapshot_->size());
    }

  private:
    const MetricsSnapshot *snapshot_ = nullptr;
  };

  MetricsSnapshot() noexcept = default;

  MetricsSnapshot(const MetricsSnapshot &) = delete;
  MetricsSnapshot &operator=(const MetricsSnapshot &) = delete;

  MetricsSnapshot(MetricsSnapshot &&other) noexcept : raw_(other.release()) {}

  MetricsSnapshot &operator=(MetricsSnapshot &&other) noexcept {
    if (this != &other) {
      reset(other.release());
    }
    return *this;
  }

  ~MetricsSnapshot() { reset(); }

  static Result collect() {
    tikv_metrics_snapshot_t *raw = nullptr;
    tikv_error_t *error = nullptr;
    const tikv_status_t code = tikv_metrics_snapshot_collect(&raw, &error);
    ::tikv::async::Status status = ::tikv::async::Status::from_ffi(code, error);
    if (!status.ok()) {
      return status;
    }
    return MetricsSnapshot(raw);
  }

  bool valid() const noexcept { return raw_ != nullptr; }

  explicit operator bool() const noexcept { return valid(); }

  std::size_t size() const noexcept {
    return tikv_metrics_snapshot_len(raw_);
  }

  const_iterator begin() const noexcept { return const_iterator(this, 0); }

  const_iterator end() const noexcept { return const_iterator(this, size()); }

  view_iterator view_begin() const noexcept { return view_iterator(this, 0); }

  view_iterator view_end() const noexcept {
    return view_iterator(this, size());
  }

  ViewRange views() const noexcept { return ViewRange(this); }

  SampleResult get(std::size_t index) const {
    auto raw_sample = get_raw_sample(index);
    if (!raw_sample.ok()) {
      return raw_sample.status();
    }
    return MetricSample::from_c(raw_sample.value());
  }

  SampleViewResult get_view(std::size_t index) const {
    auto raw_sample = get_raw_sample(index);
    if (!raw_sample.ok()) {
      return raw_sample.status();
    }
    return MetricSampleView(raw_sample.value());
  }

  SamplesResult samples() const {
    std::vector<MetricSample> result;
    result.reserve(size());
    for (std::size_t i = 0; i < size(); ++i) {
      auto sample = get(i);
      if (!sample.ok()) {
        return sample.status();
      }
      result.push_back(sample.take());
    }
    return result;
  }

  tikv_metrics_snapshot_t *get_raw() const noexcept { return raw_; }

  tikv_metrics_snapshot_t *release() noexcept {
    tikv_metrics_snapshot_t *raw = raw_;
    raw_ = nullptr;
    return raw;
  }

  void reset(tikv_metrics_snapshot_t *raw = nullptr) noexcept {
    if (raw_ != nullptr) {
      tikv_metrics_snapshot_destroy(raw_);
    }
    raw_ = raw;
  }

private:
  explicit MetricsSnapshot(tikv_metrics_snapshot_t *raw) noexcept : raw_(raw) {}

  ::tikv::async::Result<tikv_metric_sample_t>
  get_raw_sample(std::size_t index) const {
    tikv_metric_sample_t sample = {};
    tikv_error_t *error = nullptr;
    const tikv_status_t code =
        tikv_metrics_snapshot_get(raw_, index, &sample, &error);
    ::tikv::async::Status status = ::tikv::async::Status::from_ffi(code, error);
    if (!status.ok()) {
      return status;
    }
    return sample;
  }

  tikv_metrics_snapshot_t *raw_ = nullptr;
};

} // namespace metrics
} // namespace tikv

#endif // TIKV_CLIENT_METRICS_HPP
