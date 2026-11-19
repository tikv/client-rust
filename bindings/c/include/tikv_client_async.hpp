// Copyright (c) 2026 PingCAP, Inc.

#ifndef TIKV_CLIENT_ASYNC_HPP
#define TIKV_CLIENT_ASYNC_HPP

#include "tikv_client_async_c.h"

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <future>
#include <initializer_list>
#include <iterator>
#include <limits>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

/*
 * Header-only C++17 convenience wrapper for the async C FFI in
 * `tikv_client_async_c.h`.
 *
 * The C layer owns the actual Rust async runtime and TiKV client. This header
 * adds small RAII types, `Result<T>` error handling, and `std::future` based
 * completion waiting. All submit functions still follow the C FFI contract:
 * input buffers are copied by Rust before submit returns, and accepted
 * requests complete exactly once.
 *
 * For a runnable C++ example, see
 * `bindings/c/examples/raw_async_demo.cpp`.
 */
namespace tikv {
namespace async {

/*
 * Owning byte buffer used when callers want to copy TiKV bytes into C++
 * storage.
 */
using Bytes = std::vector<std::uint8_t>;

/*
 * Lightweight error payload copied out of a C `tikv_error_t`.
 *
 * Constructing from a raw C error takes ownership of that error pointer and
 * destroys it after copying the message. `Status` is usually the public type
 * callers should inspect directly.
 */
struct Error {
  std::string message;

  Error() = default;

  explicit Error(std::string error_message)
      : message(std::move(error_message)) {}

  explicit Error(tikv_error_t *raw_error) {
    if (raw_error != nullptr) {
      const char *raw_message = tikv_error_message(raw_error);
      if (raw_message != nullptr) {
        message = raw_message;
      }
      tikv_error_destroy(raw_error);
    }
  }

  bool empty() const noexcept { return message.empty(); }
};

/*
 * Status-only result used by operations that do not return a value.
 *
 * A default-constructed `Status` is OK. Non-OK statuses may carry a copied
 * error message. Use `Status::from_ffi` at C boundaries so `tikv_error_t`
 * ownership is handled consistently.
 */
class Status {
public:
  Status() = default;

  explicit Status(tikv_status_t code) : code_(code) {
    if (code_ != TIKV_STATUS_OK) {
      error_.emplace();
    }
  }

  Status(tikv_status_t code, std::string message) : code_(code) {
    if (code_ != TIKV_STATUS_OK) {
      error_.emplace(::tikv::async::Error(std::move(message)));
    }
  }

  Status(tikv_status_t code, ::tikv::async::Error error) : code_(code) {
    if (code_ != TIKV_STATUS_OK) {
      error_.emplace(std::move(error));
    }
  }

  static Status OK() { return Status(); }

  static Status Error(std::string message) {
    return Status(TIKV_STATUS_ERROR, std::move(message));
  }

  static Status InvalidArgument(std::string message = {}) {
    return Status(TIKV_STATUS_INVALID_ARGUMENT, std::move(message));
  }

  static Status Panic(std::string message = {}) {
    return Status(TIKV_STATUS_PANIC, std::move(message));
  }

  static Status Rejected(std::string message = {}) {
    return Status(TIKV_STATUS_REJECTED, std::move(message));
  }

  static Status Cancelled(std::string message = {}) {
    return Status(TIKV_STATUS_CANCELLED, std::move(message));
  }

  static Status from_ffi(tikv_status_t code, tikv_error_t *raw_error) {
    if (code == TIKV_STATUS_OK) {
      ::tikv::async::Error cleanup(raw_error);
      return Status();
    }
    if (raw_error == nullptr) {
      return Status(code);
    }
    return Status(code, ::tikv::async::Error(raw_error));
  }

  bool ok() const noexcept { return code_ == TIKV_STATUS_OK; }

  tikv_status_t code() const noexcept { return code_; }

  const char *code_name() const noexcept { return status_name(code_); }

  const ::tikv::async::Error *error() const noexcept {
    return error_.has_value() ? &*error_ : nullptr;
  }

  const std::string &error_message() const noexcept {
    static const std::string empty;
    if (!error_.has_value()) {
      return empty;
    }
    return error_->message;
  }

  std::string to_string() const {
    if (ok()) {
      return "OK";
    }
    if (error_.has_value() && !error_->message.empty()) {
      std::string result(code_name());
      result += ": ";
      result += error_->message;
      return result;
    }
    return code_name();
  }

private:
  /*
   * Convert a C FFI status code into a stable printable name.
   */
  static const char *status_name(tikv_status_t status) noexcept {
    switch (status) {
    case TIKV_STATUS_OK:
      return "OK";
    case TIKV_STATUS_ERROR:
      return "ERROR";
    case TIKV_STATUS_INVALID_ARGUMENT:
      return "INVALID_ARGUMENT";
    case TIKV_STATUS_PANIC:
      return "PANIC";
    case TIKV_STATUS_REJECTED:
      return "REJECTED";
    case TIKV_STATUS_CANCELLED:
      return "CANCELLED";
    }
    return "UNKNOWN";
  }

  tikv_status_t code_ = TIKV_STATUS_OK;
  std::optional<::tikv::async::Error> error_;
};

/*
 * Exception thrown by `Result<T>::value()` when exceptions are enabled and the
 * result is not OK.
 */
class BadResultAccess : public std::logic_error {
public:
  explicit BadResultAccess(Status status)
      : std::logic_error(status.to_string()), status_(std::move(status)) {}

  const Status &status() const noexcept { return status_; }

private:
  Status status_;
};

/*
 * Marker for borrowed/view values stored in `Result<T>`.
 *
 * `Result<T>::take()` consumes an owning result value. View-like values should
 * be read with `value()` so callers do not accidentally imply ownership
 * transfer.
 */
struct BorrowedResultValueTag {};

/*
 * Small `Result<T>` type for this wrapper.
 *
 * Successful results contain a value. Failed results contain a non-OK
 * `Status`. Check `ok()` before accessing the value, or call `value()` and
 * handle `BadResultAccess` when exceptions are enabled. Use `take()` to move
 * the value out after checking success.
 */
template <typename T>
class Result {
public:
  Result(const T &value) : value_(value) {}

  Result(T &&value) : value_(std::move(value)) {}

  Result(Status status) : status_(normalize_status(std::move(status))) {}

  bool ok() const noexcept { return status_.ok(); }

  explicit operator bool() const noexcept { return ok(); }

  const Status &status() const noexcept { return status_; }

  bool has_value() const noexcept { return value_.has_value(); }

  T &value() & {
    ensure_ok();
    return *value_;
  }

  const T &value() const & {
    ensure_ok();
    return *value_;
  }

  T &&value() && {
    ensure_ok();
    return std::move(*value_);
  }

  const T &&value() const && {
    ensure_ok();
    return std::move(*value_);
  }

  template <
      typename U = T,
      typename std::enable_if<
          !std::is_base_of<
              BorrowedResultValueTag,
              typename std::remove_cv<
                  typename std::remove_reference<U>::type>::type>::value,
          int>::type = 0>
  T take() {
    ensure_ok();
    T result = std::move(*value_);
    value_.reset();
    return result;
  }

  T *operator->() {
    return &value();
  }

  const T *operator->() const {
    return &value();
  }

  T &operator*() & noexcept {
    return *value_;
  }

  const T &operator*() const & noexcept {
    return *value_;
  }

  T &&operator*() && noexcept {
    return std::move(*value_);
  }

private:
  static Status normalize_status(Status status) {
    if (status.ok()) {
      return Status::InvalidArgument("Result constructed from OK status without a value");
    }
    return status;
  }

  void ensure_ok() const {
    if (!ok()) {
      fail_bad_access(status_);
    }
    if (!value_.has_value()) {
      fail_bad_access(
          Status::InvalidArgument("Result value has already been taken"));
    }
  }

  [[noreturn]] static void fail_bad_access(const Status &status) {
#if defined(__cpp_exceptions) || defined(__EXCEPTIONS) || defined(_CPPUNWIND)
    throw BadResultAccess(status);
#else
    (void)status;
    std::abort();
#endif
  }

  Status status_;
  std::optional<T> value_;
};

/*
 * Non-owning byte slice view passed into the C FFI.
 *
 * `Slice` never copies bytes. It is safe to pass temporary stack buffers,
 * strings, or vectors to submit functions because the Rust side copies inputs
 * before submit returns. Do not keep a `Slice` after the storage it points to
 * has been destroyed.
 */
class Slice {
public:
  Slice() noexcept = default;

  Slice(const char *data, std::size_t size) noexcept
      : data_(data), size_(size) {}

  Slice(const std::uint8_t *data, std::size_t size) noexcept
      : data_(reinterpret_cast<const char *>(data)), size_(size) {}

  explicit Slice(tikv_slice_t raw) noexcept
      : data_(reinterpret_cast<const char *>(raw.ptr)), size_(raw.len) {}

  Slice(const char *value) noexcept
      : data_(value), size_(value == nullptr ? 0 : std::strlen(value)) {}

  Slice(const std::string &value) noexcept
      : data_(value.data()), size_(value.size()) {}

  /*
   * Deleted to avoid accidentally storing a view into a temporary string.
   */
  Slice(std::string &&value) = delete;

  Slice(std::string_view value) noexcept
      : data_(value.data()), size_(value.size()) {}

  tikv_slice_t to_tikv_slice() const noexcept {
    return tikv_slice_t{
        reinterpret_cast<const std::uint8_t *>(data_),
        size_,
    };
  }

  operator tikv_slice_t() const noexcept { return to_tikv_slice(); }

  const char *data() const noexcept { return data_; }

  const std::uint8_t *bytes() const noexcept {
    return reinterpret_cast<const std::uint8_t *>(data_);
  }

  std::size_t size() const noexcept { return size_; }

  bool empty() const noexcept { return size_ == 0; }

  void clear() noexcept {
    data_ = nullptr;
    size_ = 0;
  }

  void remove_prefix(std::size_t n) noexcept {
    if (n == 0 || data_ == nullptr) {
      return;
    }
    if (n > size_) {
      n = size_;
    }
    data_ += n;
    size_ -= n;
  }

  std::string_view view_string() const noexcept {
    if (data_ == nullptr || size_ == 0) {
      return {};
    }
    return std::string_view(data_, size_);
  }

  Bytes to_bytes() const {
    if (data_ == nullptr || size_ == 0) {
      return {};
    }
    const std::uint8_t *begin = bytes();
    return Bytes(begin, begin + size_);
  }

  std::string to_string() const {
    if (data_ == nullptr || size_ == 0) {
      return {};
    }
    return std::string(data_, size_);
  }

private:
  const char *data_ = nullptr;
  std::size_t size_ = 0;
};

/*
 * RAII owner for a byte buffer returned by the Rust FFI.
 *
 * The buffer is released with `tikv_owned_bytes_destroy` unless callers
 * explicitly transfer ownership with `release()`.
 */
class OwnedBytes {
public:
  OwnedBytes() noexcept = default;

  explicit OwnedBytes(tikv_owned_bytes_t raw) noexcept : raw_(raw) {}

  ~OwnedBytes() { reset(); }

  OwnedBytes(const OwnedBytes &) = delete;
  OwnedBytes &operator=(const OwnedBytes &) = delete;

  OwnedBytes(OwnedBytes &&other) noexcept : raw_(other.release()) {}

  OwnedBytes &operator=(OwnedBytes &&other) noexcept {
    if (this != &other) {
      reset();
      raw_ = other.release();
    }
    return *this;
  }

  bool empty() const noexcept { return raw_.len == 0; }

  std::size_t size() const noexcept { return raw_.len; }

  const std::uint8_t *data() const noexcept { return raw_.ptr; }

  Slice as_slice() const noexcept { return Slice(raw_.ptr, raw_.len); }

  std::string_view view_string() const noexcept {
    if (raw_.ptr == nullptr || raw_.len == 0) {
      return {};
    }
    return std::string_view(reinterpret_cast<const char *>(raw_.ptr), raw_.len);
  }

  Bytes to_bytes() const {
    if (raw_.ptr == nullptr || raw_.len == 0) {
      return {};
    }
    return Bytes(raw_.ptr, raw_.ptr + raw_.len);
  }

  std::string to_string() const {
    if (raw_.ptr == nullptr || raw_.len == 0) {
      return {};
    }
    return std::string(reinterpret_cast<const char *>(raw_.ptr), raw_.len);
  }

  tikv_owned_bytes_t release() noexcept {
    tikv_owned_bytes_t raw = raw_;
    raw_ = tikv_owned_bytes_t{nullptr, 0};
    return raw;
  }

  void reset(tikv_owned_bytes_t raw = tikv_owned_bytes_t{nullptr, 0}) noexcept {
    if (raw_.ptr != nullptr || raw_.len != 0) {
      tikv_owned_bytes_destroy(raw_);
    }
    raw_ = raw;
  }

  const tikv_owned_bytes_t &raw() const noexcept { return raw_; }

private:
  tikv_owned_bytes_t raw_{nullptr, 0};
};

/*
 * Value returned by raw get.
 *
 * This is move-only because it owns a Rust-allocated buffer and releases it
 * with `tikv_owned_bytes_destroy`.
 */
using Value = OwnedBytes;

/*
 * Result of a raw get.
 *
 * `std::nullopt` means the key was not present. An engaged empty `Value`
 * means the key was present and its value was the empty byte string.
 */
using OptionalValue = std::optional<Value>;

/*
 * Result of a raw get-key-TTL request.
 *
 * `std::nullopt` means the key was not present. An engaged value of zero means
 * the key was present without a TTL. A positive value is the remaining TTL in
 * seconds.
 */
using OptionalKeyTtl = std::optional<std::uint64_t>;

/*
 * Marker value for operations whose successful result is only "done".
 */
struct Unit {};

/*
 * Optional user callback invoked when an accepted request completes.
 *
 * The callback runs on a Rust runtime worker thread, so it must be thread-safe
 * and must not throw.
 */
using CompletionCallback = void (*)(const Status &status,
                                    void *userdata) noexcept;

/*
 * Per-request submit options.
 *
 * `callback` is optional; callers may instead wait on the returned
 * `AsyncRequest`. `userdata` is passed through unchanged to the callback.
 */
struct SubmitOptions {
  CompletionCallback callback = nullptr;
  void *userdata = nullptr;
};

/*
 * Mutation item used by `Client::batch_write`.
 *
 * `put` carries both key and value. `remove` carries only the key.
 */
struct Mutation {
  tikv_async_raw_mutation_op_t op = TIKV_ASYNC_RAW_MUTATION_OP_PUT;
  Slice key;
  Slice value;

  static Mutation put(Slice key, Slice value) noexcept {
    return Mutation{TIKV_ASYNC_RAW_MUTATION_OP_PUT, key, value};
  }

  static Mutation remove(Slice key) noexcept {
    return Mutation{TIKV_ASYNC_RAW_MUTATION_OP_DELETE, key, Slice()};
  }
};

/*
 * TTL-capable mutation item used by `Client::batch_write_with_ttl`.
 *
 * PUT requires an explicit relative TTL in seconds; zero disables TTL for the
 * key. DELETE ignores TTL and always carries zero.
 */
struct MutationWithTtl {
  tikv_async_raw_mutation_op_t op = TIKV_ASYNC_RAW_MUTATION_OP_PUT;
  Slice key;
  Slice value;
  std::uint64_t ttl_secs = 0;

  static MutationWithTtl put(Slice key,
                             Slice value,
                             std::uint64_t ttl_secs) noexcept {
    return MutationWithTtl{
        TIKV_ASYNC_RAW_MUTATION_OP_PUT,
        key,
        value,
        ttl_secs,
    };
  }

  static MutationWithTtl remove(Slice key) noexcept {
    return MutationWithTtl{
        TIKV_ASYNC_RAW_MUTATION_OP_DELETE,
        key,
        Slice(),
        0,
    };
  }
};

/*
 * Borrowed key/value pair view.
 *
 * The slices point into the owning `KvPairs` object and become invalid when
 * that object is moved, reset, or destroyed.
 */
struct KvPairView {
  Slice key;
  Slice value;
};

/*
 * RAII owner for a vector of key/value pairs returned by scan or batch-get.
 *
 * Iterators and `KvPairView` values borrow from this object. The underlying
 * Rust-owned memory is released with `tikv_kv_pairs_destroy`.
 */
class KvPairs {
public:
  class const_iterator {
  public:
    using iterator_category = std::forward_iterator_tag;
    using value_type = KvPairView;
    using difference_type = std::ptrdiff_t;
    using pointer = void;
    using reference = KvPairView;

    const_iterator() noexcept = default;

    explicit const_iterator(const tikv_kv_pair_t *ptr) noexcept : ptr_(ptr) {}

    KvPairView operator*() const noexcept { return KvPairs::pair_view(*ptr_); }

    const_iterator &operator++() noexcept {
      ++ptr_;
      return *this;
    }

    const_iterator operator++(int) noexcept {
      const_iterator current(*this);
      ++(*this);
      return current;
    }

    bool operator==(const const_iterator &other) const noexcept {
      return ptr_ == other.ptr_;
    }

    bool operator!=(const const_iterator &other) const noexcept {
      return !(*this == other);
    }

  private:
    const tikv_kv_pair_t *ptr_ = nullptr;
  };

  KvPairs() noexcept = default;

  explicit KvPairs(tikv_kv_pairs_t raw) noexcept : raw_(raw) {}

  ~KvPairs() { reset(); }

  KvPairs(const KvPairs &) = delete;
  KvPairs &operator=(const KvPairs &) = delete;

  KvPairs(KvPairs &&other) noexcept : raw_(other.release()) {}

  KvPairs &operator=(KvPairs &&other) noexcept {
    if (this != &other) {
      reset();
      raw_ = other.release();
    }
    return *this;
  }

  bool empty() const noexcept { return raw_.len == 0; }

  std::size_t size() const noexcept { return raw_.len; }

  KvPairView operator[](std::size_t index) const noexcept {
    return pair_view(raw_.ptr[index]);
  }

  const_iterator begin() const noexcept { return const_iterator(raw_.ptr); }

  const_iterator end() const noexcept {
    return const_iterator(raw_.ptr == nullptr ? nullptr : raw_.ptr + raw_.len);
  }

  tikv_kv_pairs_t release() noexcept {
    tikv_kv_pairs_t raw = raw_;
    raw_ = tikv_kv_pairs_t{nullptr, 0};
    return raw;
  }

  void reset(tikv_kv_pairs_t raw = tikv_kv_pairs_t{nullptr, 0}) noexcept {
    if (raw_.ptr != nullptr || raw_.len != 0) {
      tikv_kv_pairs_destroy(raw_);
    }
    raw_ = raw;
  }

  const tikv_kv_pairs_t &raw() const noexcept { return raw_; }

private:
  static Slice bytes_view(const tikv_owned_bytes_t &bytes) noexcept {
    return Slice(bytes.ptr, bytes.len);
  }

  static KvPairView pair_view(const tikv_kv_pair_t &pair) noexcept {
    return KvPairView{
        bytes_view(pair.key),
        bytes_view(pair.value),
    };
  }

  tikv_kv_pairs_t raw_{nullptr, 0};
};

/*
 * RAII owner for an accepted async request handle.
 *
 * A `Request` remains valid until destroyed or reset. Result extraction
 * helpers must be called only after successful completion, and each successful
 * result may be taken only once.
 */
class Request {
public:
  Request() noexcept = default;

  explicit Request(tikv_async_request_t *raw) noexcept : raw_(raw) {}

  ~Request() { reset(); }

  Request(const Request &) = delete;
  Request &operator=(const Request &) = delete;

  Request(Request &&other) noexcept : raw_(other.release()) {}

  Request &operator=(Request &&other) noexcept {
    if (this != &other) {
      reset();
      raw_ = other.release();
    }
    return *this;
  }

  bool valid() const noexcept { return raw_ != nullptr; }

  explicit operator bool() const noexcept { return valid(); }

  /*
   * Return the borrowed raw C handle. The `Request` keeps ownership.
   */
  tikv_async_request_t *get() const noexcept { return raw_; }

  /*
   * Transfer ownership of the raw C handle to the caller.
   */
  tikv_async_request_t *release() noexcept {
    tikv_async_request_t *raw = raw_;
    raw_ = nullptr;
    return raw;
  }

  /*
   * Replace the owned request handle, destroying the previous one if present.
   */
  void reset(tikv_async_request_t *raw = nullptr) noexcept {
    if (raw_ != nullptr) {
      tikv_async_request_destroy(raw_);
    }
    raw_ = raw;
  }

  /*
   * Request best-effort cancellation.
   *
   * A successful return value of `true` means this call newly requested
   * cancellation. `false` means the request had already completed or had
   * already been asked to cancel.
   */
  Result<bool> cancel() {
    if (raw_ == nullptr) {
      return invalid_request_status();
    }

    bool cancel_requested = false;
    tikv_error_t *error = nullptr;
    const tikv_status_t code =
        tikv_async_request_cancel(raw_, &cancel_requested, &error);
    Status status = Status::from_ffi(code, error);
    if (!status.ok()) {
      return status;
    }
    return cancel_requested;
  }

  /*
   * Query whether the request callback has completed.
   */
  Result<bool> is_complete() const {
    if (raw_ == nullptr) {
      return invalid_request_status();
    }

    bool complete = false;
    tikv_error_t *error = nullptr;
    const tikv_status_t code =
        tikv_async_request_is_complete(raw_, &complete, &error);
    Status status = Status::from_ffi(code, error);
    if (!status.ok()) {
      return status;
    }
    return complete;
  }

  /*
   * Return the terminal completion status.
   *
   * This is valid only after the request has completed.
   */
  Result<tikv_status_t> completion_status() const {
    if (raw_ == nullptr) {
      return invalid_request_status();
    }

    tikv_status_t completion = TIKV_STATUS_ERROR;
    tikv_error_t *error = nullptr;
    const tikv_status_t code =
        tikv_async_request_completion_status(raw_, &completion, &error);
    Status status = Status::from_ffi(code, error);
    if (!status.ok()) {
      return status;
    }
    return completion;
  }

  /*
   * Copy the terminal error message out of the request state, if any.
   */
  std::string error_message() const {
    std::string_view message = error_message_view();
    if (message.empty()) {
      return {};
    }
    return std::string(message.data(), message.size());
  }

  /*
   * View the terminal error message without copying it.
   *
   * The view remains valid only while this `Request` owns the raw handle.
   */
  std::string_view error_message_view() const noexcept {
    const char *message = error_c_str();
    if (message == nullptr) {
      return {};
    }
    return std::string_view(message, std::strlen(message));
  }

  /*
   * Take the result from a completed raw get request.
   *
   * `std::nullopt` means the key was not present. Calling this twice on the
   * same successful request returns an error.
   */
  Result<OptionalValue> take_get_result() {
    if (raw_ == nullptr) {
      return invalid_request_status();
    }

    tikv_owned_bytes_t raw_value{nullptr, 0};
    bool found = false;
    tikv_error_t *error = nullptr;
    const tikv_status_t code =
        tikv_async_request_take_get_result(raw_, &raw_value, &found, &error);
    Status status = Status::from_ffi(code, error);
    if (!status.ok()) {
      tikv_owned_bytes_destroy(raw_value);
      return status;
    }

    if (!found) {
      tikv_owned_bytes_destroy(raw_value);
      return Result<OptionalValue>(OptionalValue(std::nullopt));
    }

    return Result<OptionalValue>(OptionalValue(std::in_place, raw_value));
  }

  /*
   * Take the result from a completed raw get-key-TTL request.
   *
   * `std::nullopt` means the key was not present. An engaged zero means the key
   * exists without a TTL. Calling this twice on the same successful request
   * returns an error.
   */
  Result<OptionalKeyTtl> take_get_key_ttl_result() {
    if (raw_ == nullptr) {
      return invalid_request_status();
    }

    std::uint64_t ttl_secs = 0;
    bool found = false;
    tikv_error_t *error = nullptr;
    const tikv_status_t code = tikv_async_request_take_get_key_ttl_result(
        raw_, &ttl_secs, &found, &error);
    Status status = Status::from_ffi(code, error);
    if (!status.ok()) {
      return status;
    }

    if (!found) {
      return Result<OptionalKeyTtl>(OptionalKeyTtl(std::nullopt));
    }

    return Result<OptionalKeyTtl>(OptionalKeyTtl(ttl_secs));
  }

  /*
   * Take the result from a completed scan or reverse-scan request.
   */
  Result<KvPairs> take_scan_result() {
    return take_pairs_result(tikv_async_request_take_scan_result);
  }

  /*
   * Take the result from a completed batch-get request.
   */
  Result<KvPairs> take_batch_get_result() {
    return take_pairs_result(tikv_async_request_take_batch_get_result);
  }

private:
  using TakePairsFn = std::function<tikv_status_t(const tikv_async_request_t *,
                                                  tikv_kv_pairs_t *,
                                                  tikv_error_t **)>;

  const char *error_c_str() const noexcept {
    if (raw_ == nullptr) {
      return nullptr;
    }
    return tikv_async_request_error_message(raw_);
  }

  static Status invalid_request_status() {
    return Status::InvalidArgument("request is null");
  }

  Result<KvPairs> take_pairs_result(const TakePairsFn &take_pairs) {
    if (raw_ == nullptr) {
      return invalid_request_status();
    }
    if (!take_pairs) {
      return Status::InvalidArgument("take pairs function is empty");
    }

    tikv_kv_pairs_t raw_pairs{nullptr, 0};
    tikv_error_t *error = nullptr;
    const tikv_status_t code = take_pairs(raw_, &raw_pairs, &error);
    Status status = Status::from_ffi(code, error);
    if (!status.ok()) {
      tikv_kv_pairs_destroy(raw_pairs);
      return status;
    }
    return Result<KvPairs>(KvPairs(raw_pairs));
  }

  tikv_async_request_t *raw_ = nullptr;
};

/*
 * Submitted async request that eventually yields a value of type `T`.
 *
 * `submitted()` tells whether the submit call itself was accepted. Once a
 * request is accepted, `wait_status()` waits for terminal completion and
 * `take()` extracts the result. For convenience, `wait()` does both.
 */
template <typename T>
class AsyncRequest {
public:
  /*
   * Converts a completed raw request handle into the typed result.
   */
  using Extractor = std::function<Result<T>(Request &)>;

  AsyncRequest() = default;

  /*
   * Build a request object for a submit call that was rejected synchronously.
   */
  static AsyncRequest rejected(Status status) {
    AsyncRequest request;
    request.submit_status_ = normalize_rejected_status(std::move(status));
    return request;
  }

  /*
   * Build a request object for a submit call accepted by the C FFI.
   */
  static AsyncRequest submitted(Request request,
                                std::shared_future<Status> done,
                                Extractor extractor) {
    AsyncRequest async_request;
    async_request.submit_status_ = Status::OK();
    async_request.request_ = std::move(request);
    async_request.done_ = std::move(done);
    async_request.extractor_ = std::move(extractor);
    return async_request;
  }

  AsyncRequest(const AsyncRequest &) = delete;
  AsyncRequest &operator=(const AsyncRequest &) = delete;

  AsyncRequest(AsyncRequest &&) noexcept = default;
  AsyncRequest &operator=(AsyncRequest &&) noexcept = default;

  bool submitted() const noexcept { return submit_status_.ok(); }

  /*
   * Status returned by the submit call itself.
   *
   * A non-OK status means no async callback will run.
   */
  const Status &submit_status() const noexcept { return submit_status_; }

  /*
   * Non-blocking completion check.
   */
  bool ready() const {
    if (!submitted()) {
      return true;
    }
    if (!done_.valid()) {
      return true;
    }
    return done_.wait_for(std::chrono::seconds(0)) == std::future_status::ready;
  }

  /*
   * Wait for completion and return only the terminal status.
   */
  Status wait_status() const {
    if (!submit_status_.ok()) {
      return submit_status_;
    }
    if (!done_.valid()) {
      return Status::InvalidArgument("async request has no completion signal");
    }
    return done_.get();
  }

  /*
   * Extract the typed result after successful completion.
   *
   * The underlying C result can be taken only once.
   */
  Result<T> take() {
    if (!submit_status_.ok()) {
      return submit_status_;
    }
    if (!extractor_) {
      return Status::InvalidArgument("async request has no result extractor");
    }
    return extractor_(request_);
  }

  /*
   * Wait for completion, then take the typed result.
   */
  Result<T> wait() {
    Status status = wait_status();
    if (!status.ok()) {
      return status;
    }
    return take();
  }

  /*
   * Request best-effort cancellation of the accepted request.
   */
  Result<bool> cancel() {
    if (!submit_status_.ok()) {
      return submit_status_;
    }
    return request_.cancel();
  }

  /*
   * Access the underlying request handle for lower-level operations.
   */
  Request &request() noexcept { return request_; }

  const Request &request() const noexcept { return request_; }

private:
  static Status normalize_rejected_status(Status status) {
    if (status.ok()) {
      return Status::InvalidArgument("async request rejected with OK status");
    }
    return status;
  }

  Status submit_status_ = Status::InvalidArgument("async request is not submitted");
  Request request_;
  std::shared_future<Status> done_;
  Extractor extractor_;
};

/*
 * `AsyncRequest` specialization for operations that return no payload.
 *
 * `wait()` returns the terminal `Status` directly because there is no value to
 * extract after completion.
 */
template <>
class AsyncRequest<Unit> {
public:
  AsyncRequest() = default;

  static AsyncRequest rejected(Status status) {
    AsyncRequest request;
    request.submit_status_ = normalize_rejected_status(std::move(status));
    return request;
  }

  static AsyncRequest submitted(Request request, std::shared_future<Status> done) {
    AsyncRequest async_request;
    async_request.submit_status_ = Status::OK();
    async_request.request_ = std::move(request);
    async_request.done_ = std::move(done);
    return async_request;
  }

  AsyncRequest(const AsyncRequest &) = delete;
  AsyncRequest &operator=(const AsyncRequest &) = delete;

  AsyncRequest(AsyncRequest &&) noexcept = default;
  AsyncRequest &operator=(AsyncRequest &&) noexcept = default;

  bool submitted() const noexcept { return submit_status_.ok(); }

  /*
   * Status returned by the submit call itself.
   */
  const Status &submit_status() const noexcept { return submit_status_; }

  /*
   * Non-blocking completion check.
   */
  bool ready() const {
    if (!submitted()) {
      return true;
    }
    if (!done_.valid()) {
      return true;
    }
    return done_.wait_for(std::chrono::seconds(0)) == std::future_status::ready;
  }

  /*
   * Wait for terminal completion.
   */
  Status wait() const {
    if (!submit_status_.ok()) {
      return submit_status_;
    }
    if (!done_.valid()) {
      return Status::InvalidArgument("async request has no completion signal");
    }
    return done_.get();
  }

  /*
   * Request best-effort cancellation of the accepted request.
   */
  Result<bool> cancel() {
    if (!submit_status_.ok()) {
      return submit_status_;
    }
    return request_.cancel();
  }

  /*
   * Access the underlying request handle for lower-level operations.
   */
  Request &request() noexcept { return request_; }

  const Request &request() const noexcept { return request_; }

private:
  static Status normalize_rejected_status(Status status) {
    if (status.ok()) {
      return Status::InvalidArgument("async request rejected with OK status");
    }
    return status;
  }

  Status submit_status_ = Status::InvalidArgument("async request is not submitted");
  Request request_;
  std::shared_future<Status> done_;
};

/*
 * RAII owner for an async TiKV raw client.
 *
 * Destroying a `Client` begins shutdown with cancellation, drains accepted
 * requests, and releases the C client handle. Call `shutdown_begin(false)` and
 * `drain()` explicitly if you want in-flight requests to finish normally.
 */
class Client {
public:
  /*
   * Shared C++ owner type for callers that want many application objects to
   * hold the same client instance. The C ABI handle is still owned by exactly
   * one `Client` object and is destroyed when the last shared owner releases it.
   */
  using SharedPtr = std::shared_ptr<Client>;

  Client() noexcept = default;

  explicit Client(tikv_async_client_t *raw) noexcept : raw_(raw) {}

  /*
   * Connect to one or more PD endpoints.
   *
   * `pd_endpoints_csv` is a comma-separated endpoint list such as
   * `"127.0.0.1:2379,127.0.0.1:2381"`. `timeout_secs == 0` uses the Rust
   * client's default timeout. Set `use_default_keyspace` for API v2 clusters
   * that require the default keyspace. By default, the gRPC response decoding
   * size is unlimited for compatibility. Pass a byte limit explicitly to
   * override it; zero uses the Rust client's 4 MiB default.
   */
  static Result<Client> connect(const char *pd_endpoints_csv,
                                std::uint32_t timeout_secs = 0,
                                bool use_default_keyspace = false,
                                std::size_t grpc_max_decoding_message_size =
                                    std::numeric_limits<std::size_t>::max()) {
    tikv_async_client_t *raw_client = nullptr;
    tikv_error_t *error = nullptr;
    const tikv_status_t code =
        tikv_async_client_new_with_grpc_max_decoding_message_size(
            pd_endpoints_csv,
            timeout_secs,
            use_default_keyspace,
            grpc_max_decoding_message_size,
            &raw_client,
            &error);
    Status status = Status::from_ffi(code, error);
    if (!status.ok()) {
      if (raw_client != nullptr) {
        tikv_async_client_destroy(raw_client);
      }
      return status;
    }
    return Client(raw_client);
  }

  /*
   * String overload for `connect`.
   */
  static Result<Client> connect(const std::string &pd_endpoints_csv,
                                std::uint32_t timeout_secs = 0,
                                bool use_default_keyspace = false,
                                std::size_t grpc_max_decoding_message_size =
                                    std::numeric_limits<std::size_t>::max()) {
    return connect(pd_endpoints_csv.c_str(),
                   timeout_secs,
                   use_default_keyspace,
                   grpc_max_decoding_message_size);
  }

  /*
   * Try to connect and return the resulting client inside a `std::shared_ptr`.
   *
   * This is a convenience factory for dependency injection or worker objects
   * that should share one C++ `Client`. It does not clone the C ABI handle.
   */
  static Result<SharedPtr> connect_as_shared(const char *pd_endpoints_csv,
                                             std::uint32_t timeout_secs = 0,
                                             bool use_default_keyspace = false,
                                             std::size_t grpc_max_decoding_message_size =
                                                 std::numeric_limits<std::size_t>::max()) {
    auto client_result = connect(pd_endpoints_csv,
                                 timeout_secs,
                                 use_default_keyspace,
                                 grpc_max_decoding_message_size);
    if (!client_result.ok()) {
      return client_result.status();
    }

    return std::make_shared<Client>(client_result.take());
  }

  /*
   * String overload for `connect_as_shared`.
   */
  static Result<SharedPtr> connect_as_shared(const std::string &pd_endpoints_csv,
                                             std::uint32_t timeout_secs = 0,
                                             bool use_default_keyspace = false,
                                             std::size_t grpc_max_decoding_message_size =
                                                 std::numeric_limits<std::size_t>::max()) {
    return connect_as_shared(pd_endpoints_csv.c_str(),
                             timeout_secs,
                             use_default_keyspace,
                             grpc_max_decoding_message_size);
  }

  ~Client() { reset(); }

  Client(const Client &) = delete;
  Client &operator=(const Client &) = delete;

  Client(Client &&other) noexcept : raw_(other.release()) {}

  Client &operator=(Client &&other) noexcept {
    if (this != &other) {
      reset();
      raw_ = other.release();
    }
    return *this;
  }

  bool valid() const noexcept { return raw_ != nullptr; }

  explicit operator bool() const noexcept { return valid(); }

  /*
   * Return the borrowed raw C client handle. The `Client` keeps ownership.
   */
  tikv_async_client_t *get() const noexcept { return raw_; }

  /*
   * Transfer ownership of the raw C client handle to the caller.
   */
  tikv_async_client_t *release() noexcept {
    tikv_async_client_t *raw = raw_;
    raw_ = nullptr;
    return raw;
  }

  /*
   * Replace the owned client handle, destroying the previous one if present.
   */
  void reset(tikv_async_client_t *raw = nullptr) noexcept {
    if (raw_ != nullptr) {
      tikv_async_client_destroy(raw_);
    }
    raw_ = raw;
  }

  /*
   * Start client shutdown.
   *
   * After shutdown begins, new submit calls are rejected. If
   * `cancel_inflight` is true, accepted in-flight requests complete as
   * cancelled.
   */
  Status shutdown_begin(bool cancel_inflight = false) {
    if (raw_ == nullptr) {
      return invalid_client_status();
    }

    tikv_error_t *error = nullptr;
    const tikv_status_t code =
        tikv_async_client_shutdown_begin(raw_, cancel_inflight, &error);
    return Status::from_ffi(code, error);
  }

  /*
   * Wait for all currently accepted requests to finish their callbacks.
   */
  Status drain() const {
    if (raw_ == nullptr) {
      return invalid_client_status();
    }

    tikv_error_t *error = nullptr;
    const tikv_status_t code = tikv_async_client_drain(raw_, &error);
    return Status::from_ffi(code, error);
  }

  /*
   * Submit a raw put request.
   *
   * The key and value bytes are copied by Rust before this function returns.
   */
  AsyncRequest<Unit> put(Slice key,
                         Slice value,
                         SubmitOptions options = SubmitOptions()) const {
    return submit_unit(
        [this, key, value](tikv_async_completion_cb completion,
                           void *userdata,
                           tikv_async_request_t **out_request,
                           tikv_error_t **out_error) {
          return tikv_async_raw_put_submit(raw_,
                                           key.to_tikv_slice(),
                                           value.to_tikv_slice(),
                                           completion,
                                           userdata,
                                           out_request,
                                           out_error);
        },
        options);
  }

  /*
   * Submit a raw put with a relative TTL in seconds.
   *
   * `ttl_secs == 0` disables TTL for the key. The key and value bytes are
   * copied by Rust before this function returns.
   */
  AsyncRequest<Unit> put_with_ttl(
      Slice key,
      Slice value,
      std::uint64_t ttl_secs,
      SubmitOptions options = SubmitOptions()) const {
    return submit_unit(
        [this, key, value, ttl_secs](tikv_async_completion_cb completion,
                                    void *userdata,
                                    tikv_async_request_t **out_request,
                                    tikv_error_t **out_error) {
          return tikv_async_raw_put_with_ttl_submit(raw_,
                                                    key.to_tikv_slice(),
                                                    value.to_tikv_slice(),
                                                    ttl_secs,
                                                    completion,
                                                    userdata,
                                                    out_request,
                                                    out_error);
        },
        options);
  }

  /*
   * Submit a raw get request.
   *
   * Use `wait()` or `wait_status()` followed by `take()` to retrieve an
   * `OptionalValue`.
   */
  AsyncRequest<OptionalValue> get(Slice key,
                                  SubmitOptions options = SubmitOptions()) const {
    return submit_value<OptionalValue>(
        [this, key](tikv_async_completion_cb completion,
                    void *userdata,
                    tikv_async_request_t **out_request,
                    tikv_error_t **out_error) {
          return tikv_async_raw_get_submit(raw_,
                                           key.to_tikv_slice(),
                                           completion,
                                           userdata,
                                           out_request,
                                           out_error);
        },
        [](Request &request) { return request.take_get_result(); },
        options);
  }

  /*
   * Submit a request for the remaining raw key TTL in seconds.
   *
   * This method returns an `AsyncRequest`. Call `wait()` on that request to
   * obtain a `Result<OptionalKeyTtl>`.
   */
  AsyncRequest<OptionalKeyTtl> get_key_ttl_secs(
      Slice key,
      SubmitOptions options = SubmitOptions()) const {
    return submit_value<OptionalKeyTtl>(
        [this, key](tikv_async_completion_cb completion,
                    void *userdata,
                    tikv_async_request_t **out_request,
                    tikv_error_t **out_error) {
          return tikv_async_raw_get_key_ttl_submit(raw_,
                                                   key.to_tikv_slice(),
                                                   completion,
                                                   userdata,
                                                   out_request,
                                                   out_error);
        },
        [](Request &request) { return request.take_get_key_ttl_result(); },
        options);
  }

  /*
   * Submit a raw delete request for one key.
   */
  AsyncRequest<Unit> remove(Slice key,
                            SubmitOptions options = SubmitOptions()) const {
    return submit_unit(
        [this, key](tikv_async_completion_cb completion,
                    void *userdata,
                    tikv_async_request_t **out_request,
                    tikv_error_t **out_error) {
          return tikv_async_raw_delete_submit(raw_,
                                              key.to_tikv_slice(),
                                              completion,
                                              userdata,
                                              out_request,
                                              out_error);
        },
        options);
  }

  /*
   * Submit a raw delete-range request.
   *
   * An empty `end_key` means the range is unbounded above.
   */
  AsyncRequest<Unit> delete_range(Slice start_key,
                                  Slice end_key,
                                  SubmitOptions options = SubmitOptions()) const {
    return submit_unit(
        [this, start_key, end_key](tikv_async_completion_cb completion,
                                   void *userdata,
                                   tikv_async_request_t **out_request,
                                   tikv_error_t **out_error) {
          return tikv_async_raw_delete_range_submit(raw_,
                                                    start_key.to_tikv_slice(),
                                                    end_key.to_tikv_slice(),
                                                    completion,
                                                    userdata,
                                                    out_request,
                                                    out_error);
        },
        options);
  }

  /*
   * Submit a forward raw scan over `[start_key, end_key)`.
   *
   * `limit` caps the number of returned pairs. An empty `end_key` means the
   * range is unbounded above.
   */
  AsyncRequest<KvPairs> scan(Slice start_key,
                             Slice end_key,
                             std::uint32_t limit,
                             SubmitOptions options = SubmitOptions()) const {
    return submit_value<KvPairs>(
        [this, start_key, end_key, limit](tikv_async_completion_cb completion,
                                          void *userdata,
                                          tikv_async_request_t **out_request,
                                          tikv_error_t **out_error) {
          return tikv_async_raw_scan_submit(raw_,
                                            start_key.to_tikv_slice(),
                                            end_key.to_tikv_slice(),
                                            limit,
                                            completion,
                                            userdata,
                                            out_request,
                                            out_error);
        },
        [](Request &request) { return request.take_scan_result(); },
        options);
  }

  /*
   * Submit a reverse raw scan over `[start_key, end_key)`.
   *
   * Returned pairs are in reverse lexicographical key order. The underlying
   * Rust raw client currently does not support reverse scanning from an empty
   * start key.
   */
  AsyncRequest<KvPairs> scan_reverse(Slice start_key,
                                     Slice end_key,
                                     std::uint32_t limit,
                                     SubmitOptions options = SubmitOptions()) const {
    return submit_value<KvPairs>(
        [this, start_key, end_key, limit](tikv_async_completion_cb completion,
                                          void *userdata,
                                          tikv_async_request_t **out_request,
                                          tikv_error_t **out_error) {
          return tikv_async_raw_scan_reverse_submit(raw_,
                                                    start_key.to_tikv_slice(),
                                                    end_key.to_tikv_slice(),
                                                    limit,
                                                    completion,
                                                    userdata,
                                                    out_request,
                                                    out_error);
        },
        [](Request &request) { return request.take_scan_result(); },
        options);
  }

  /*
   * Submit a raw batch-get request.
   *
   * Only existing keys are returned, and TiKV does not guarantee that result
   * order matches input key order.
   */
  AsyncRequest<KvPairs> batch_get(const std::vector<Slice> &keys,
                                  SubmitOptions options = SubmitOptions()) const {
    std::vector<tikv_slice_t> raw_keys;
    raw_keys.reserve(keys.size());
    for (const Slice &key : keys) {
      raw_keys.push_back(key.to_tikv_slice());
    }

    const tikv_slices_t ffi_keys{raw_keys.empty() ? nullptr : raw_keys.data(),
                                 raw_keys.size()};
    /*
     * The C submit call synchronously copies `ffi_keys`, so it is safe for this
     * vector-backed view to die after `submit_value` returns.
     */
    return submit_value<KvPairs>(
        [this, ffi_keys](tikv_async_completion_cb completion,
                         void *userdata,
                         tikv_async_request_t **out_request,
                         tikv_error_t **out_error) {
          return tikv_async_raw_batch_get_submit(raw_,
                                                 ffi_keys,
                                                 completion,
                                                 userdata,
                                                 out_request,
                                                 out_error);
        },
        [](Request &request) { return request.take_batch_get_result(); },
        options);
  }

  /*
   * Initializer-list overload for `batch_get`.
   */
  AsyncRequest<KvPairs> batch_get(std::initializer_list<Slice> keys,
                                  SubmitOptions options = SubmitOptions()) const {
    return batch_get(std::vector<Slice>(keys), options);
  }

  /*
   * Submit a mixed raw write batch.
   *
   * The C/Rust layer preserves put/delete phase order while coalescing
   * adjacent compatible mutations into efficient lower-level calls.
   */
  AsyncRequest<Unit> batch_write(const std::vector<Mutation> &mutations,
                                 SubmitOptions options = SubmitOptions()) const {
    std::vector<tikv_async_raw_mutation_t> raw_mutations;
    raw_mutations.reserve(mutations.size());
    for (const Mutation &mutation : mutations) {
      raw_mutations.push_back(tikv_async_raw_mutation_t{
          mutation.op,
          mutation.key.to_tikv_slice(),
          mutation.value.to_tikv_slice(),
      });
    }

    const tikv_async_raw_mutations_t ffi_mutations{
        raw_mutations.empty() ? nullptr : raw_mutations.data(),
        raw_mutations.size(),
    };
    /*
     * The C submit call synchronously copies `ffi_mutations`, so it is safe for
     * this vector-backed view to die after `submit_unit` returns.
     */
    return submit_unit(
        [this, ffi_mutations](tikv_async_completion_cb completion,
                              void *userdata,
                              tikv_async_request_t **out_request,
                              tikv_error_t **out_error) {
          return tikv_async_raw_batch_write_submit(raw_,
                                                   ffi_mutations,
                                                   completion,
                                                   userdata,
                                                   out_request,
                                                   out_error);
        },
        options);
  }

  /*
   * Initializer-list overload for `batch_write`.
   */
  AsyncRequest<Unit> batch_write(std::initializer_list<Mutation> mutations,
                                 SubmitOptions options = SubmitOptions()) const {
    return batch_write(std::vector<Mutation>(mutations), options);
  }

  /*
   * Submit a TTL-capable mixed raw write batch.
   *
   * Each PUT carries its own relative TTL in seconds. DELETE ignores TTL. The
   * C submit call synchronously copies all mutation inputs before this method
   * returns.
   */
  AsyncRequest<Unit> batch_write_with_ttl(
      const std::vector<MutationWithTtl> &mutations,
      SubmitOptions options = SubmitOptions()) const {
    std::vector<tikv_async_raw_mutation_with_ttl_t> raw_mutations;
    raw_mutations.reserve(mutations.size());
    for (const MutationWithTtl &mutation : mutations) {
      raw_mutations.push_back(tikv_async_raw_mutation_with_ttl_t{
          mutation.op,
          mutation.key.to_tikv_slice(),
          mutation.value.to_tikv_slice(),
          mutation.ttl_secs,
      });
    }

    const tikv_async_raw_mutations_with_ttl_t ffi_mutations{
        raw_mutations.empty() ? nullptr : raw_mutations.data(),
        raw_mutations.size(),
    };
    return submit_unit(
        [this, ffi_mutations](tikv_async_completion_cb completion,
                              void *userdata,
                              tikv_async_request_t **out_request,
                              tikv_error_t **out_error) {
          return tikv_async_raw_batch_write_with_ttl_submit(raw_,
                                                            ffi_mutations,
                                                            completion,
                                                            userdata,
                                                            out_request,
                                                            out_error);
        },
        options);
  }

  /*
   * Initializer-list overload for `batch_write_with_ttl`.
   */
  AsyncRequest<Unit> batch_write_with_ttl(
      std::initializer_list<MutationWithTtl> mutations,
      SubmitOptions options = SubmitOptions()) const {
    return batch_write_with_ttl(std::vector<MutationWithTtl>(mutations),
                                options);
  }

private:
  /*
   * Heap state handed to the C callback.
   *
   * The callback takes ownership with `std::unique_ptr`, fulfills the future,
   * then invokes the optional user callback.
   */
  struct RequestCompletionState {
    std::promise<Status> completion;
    SubmitOptions options;
  };

  using SubmitFn = std::function<tikv_status_t(tikv_async_completion_cb,
                                               void *,
                                               tikv_async_request_t **,
                                               tikv_error_t **)>;

  static Status invalid_client_status() {
    return Status::InvalidArgument("client is null");
  }

  static Status build_completion_status(tikv_async_request_t *request,
                                        tikv_status_t final_status) {
    if (final_status == TIKV_STATUS_OK) {
      return Status::OK();
    }

    const char *message = tikv_async_request_error_message(request);
    if (message != nullptr) {
      return Status(final_status, std::string(message));
    }
    return Status(final_status);
  }

  /*
   * C callback bridge used for every request submitted through this wrapper.
   */
  static void handle_request_completion(tikv_async_request_t *request,
                                        tikv_status_t final_status,
                                        void *userdata) noexcept {
    std::unique_ptr<RequestCompletionState> state(
        static_cast<RequestCompletionState *>(userdata));

    Status completion_status = build_completion_status(request, final_status);
    state->completion.set_value(completion_status);

    if (state->options.callback != nullptr) {
      state->options.callback(completion_status, state->options.userdata);
    }
  }

  /*
   * Common submit path for requests whose successful completion yields a
   * payload.
   */
  template <typename T>
  AsyncRequest<T> submit_value(SubmitFn submit,
                               typename AsyncRequest<T>::Extractor extractor,
                               SubmitOptions options) const {
    if (raw_ == nullptr) {
      return AsyncRequest<T>::rejected(invalid_client_status());
    }
    if (!submit) {
      return AsyncRequest<T>::rejected(Status::InvalidArgument("submit function is empty"));
    }

    RequestCompletionState *state = new RequestCompletionState();
    state->options = options;
    std::shared_future<Status> done = state->completion.get_future().share();

    tikv_async_request_t *raw_request = nullptr;
    tikv_error_t *error = nullptr;
    const tikv_status_t code =
        submit(handle_request_completion, state, &raw_request, &error);
    Status submit_status = Status::from_ffi(code, error);
    if (!submit_status.ok()) {
      delete state;
      return AsyncRequest<T>::rejected(std::move(submit_status));
    }

    return AsyncRequest<T>::submitted(Request(raw_request),
                                      std::move(done),
                                      std::move(extractor));
  }

  /*
   * Common submit path for requests whose successful completion has no
   * payload.
   */
  AsyncRequest<Unit> submit_unit(SubmitFn submit, SubmitOptions options) const {
    if (raw_ == nullptr) {
      return AsyncRequest<Unit>::rejected(invalid_client_status());
    }
    if (!submit) {
      return AsyncRequest<Unit>::rejected(Status::InvalidArgument("submit function is empty"));
    }

    RequestCompletionState *state = new RequestCompletionState();
    state->options = options;
    std::shared_future<Status> done = state->completion.get_future().share();

    tikv_async_request_t *raw_request = nullptr;
    tikv_error_t *error = nullptr;
    const tikv_status_t code =
        submit(handle_request_completion, state, &raw_request, &error);
    Status submit_status = Status::from_ffi(code, error);
    if (!submit_status.ok()) {
      delete state;
      return AsyncRequest<Unit>::rejected(std::move(submit_status));
    }

    return AsyncRequest<Unit>::submitted(Request(raw_request), std::move(done));
  }

  tikv_async_client_t *raw_ = nullptr;
};

} // namespace async
} // namespace tikv

#endif // TIKV_CLIENT_ASYNC_HPP
