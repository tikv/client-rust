# TiKV Rust Client C Binding

这个目录提供 TiKV Rust Client 的异步 C ABI 和 C++ 头文件封装。

接口使用异步请求路径：

- 调用方提交请求时只做参数校验和必要 copy
- 真正的 TiKV 请求在 Rust async runtime 里执行
- 完成信号通过 completion callback + userdata 收口，而不是 `block_on`
- shutdown / drain / cancel 语义在 FFI 边界显式定义

## 当前覆盖能力

- `connect`
- `put`
- `put_with_ttl`
- `get`
- `get_key_ttl`
- `delete`
- `delete_range`
- `scan`
- `scan_reverse`
- `batch_write`（可混合 `put/delete`）
- `batch_write_with_ttl`（每个 `put` 独立设置 TTL）
- `batch_get`
- `shutdown_begin`
- `drain`
- `request_cancel`

## 构建

```bash
cargo build -p tikv-client-rust-c
```

输出产物位于：

- `target/debug/libtikv_client_rust_c.dylib` 或 Linux 下的 `libtikv_client_rust_c.so`
- `target/debug/libtikv_client_rust_c.a`
- 异步客户端头文件：`bindings/c/include/tikv_client_async_c.h`
- 指标快照头文件：`bindings/c/include/tikv_client_metrics_c.h`
- C++ 指标封装头文件：`bindings/c/include/tikv_client_metrics.hpp`

## 编译 C 示例

macOS:

```bash
clang \
  -Ibindings/c/include \
  bindings/c/examples/raw_async_demo.c \
  -Ltarget/debug \
  -ltikv_client_rust_c \
  -Wl,-rpath,$PWD/target/debug \
  -o target/debug/raw_async_demo
```

Linux:

```bash
cc \
  -Ibindings/c/include \
  bindings/c/examples/raw_async_demo.c \
  -Ltarget/debug \
  -ltikv_client_rust_c \
  -Wl,-rpath,'$ORIGIN' \
  -o target/debug/raw_async_demo
```
## 接口语义

- Rust 客户端的 gRPC response decode 上限仍默认为 4 MiB。为保持原有 C/C++
  行为，`tikv_async_client_new` 和 C++ `Client::connect` / `connect_as_shared`
  默认使用 `SIZE_MAX`（Rust 的 `usize::MAX`）。如需设置上限，在创建客户端时
  显式指定字节数；显式传入 `0` 则使用 Rust 的 4 MiB 默认值。C 接口可调用
  `tikv_async_client_new_with_grpc_max_decoding_message_size(pd, timeout, keyspace, 32 * 1024 * 1024, &client, &error)`；
  C++ 接口可调用 `Client::connect(pd, timeout, keyspace, 32 * 1024 * 1024)`。
- submit 返回 `TIKV_STATUS_OK` 只表示请求已被接收，且之后一定 callback 一次
- submit 返回非 `TIKV_STATUS_OK` 表示请求未被接收，不会触发 callback
- completion callback 运行在 Rust runtime worker 线程，不保证回到 submit 线程
- `tikv_async_request_destroy` 可以早于 callback 调用；Rust 会把内部状态保活到 callback 返回
- `tikv_async_request_cancel` 是 best-effort 逻辑取消；一旦请求最终被标记为取消，它仍然会通过 callback 以 `TIKV_STATUS_CANCELLED` 显式完成
- `tikv_async_client_shutdown_begin` 立即拒绝新 submit；若 `cancel_inflight=true`，则所有 in-flight 请求最终通过 callback 以 `TIKV_STATUS_CANCELLED` 收口
- `tikv_async_client_drain` 等待所有已接收请求完成 callback
- `tikv_async_client_destroy` 会执行 shutdown-begin(cancel) + drain，再释放 client
- `batch_write` 允许一个 batch 内混合 `put/delete`；底层仍复用 `client-rust` 的 `batch_put` / `batch_delete`，但保持调用方的 put/delete phase 顺序
- 连续 `put` 段内若同一 key 重复出现，binding 会在该位置切段，避免同一 key 的多次写入被压进同一个底层 `batch_put`
- `batch_get` 只返回存在的 key，且结果顺序不保证与输入一致
- `scan_reverse` 映射到底层 `scan_reverse`；由于 `client-rust` 现有限制，空 `start_key` 不被支持

## 编译 C++ 示例

macOS:

```bash
clang++ \
  -std=c++17 \
  -Ibindings/c/include \
  bindings/c/examples/raw_async_demo.cpp \
  -Ltarget/debug \
  -ltikv_client_rust_c \
  -Wl,-rpath,$PWD/target/debug \
  -o target/debug/raw_async_demo_cpp
```

Linux:

```bash
c++ \
  -std=c++17 \
  -Ibindings/c/include \
  bindings/c/examples/raw_async_demo.cpp \
  -Ltarget/debug \
  -ltikv_client_rust_c \
  -Wl,-rpath,'$ORIGIN' \
  -o target/debug/raw_async_demo_cpp
```

## 编译 Metrics C++ 示例

macOS:

```bash
clang++ \
  -std=c++17 \
  -Ibindings/c/include \
  bindings/c/examples/metrics_snapshot_demo.cpp \
  -Ltarget/debug \
  -ltikv_client_rust_c \
  -Wl,-rpath,$PWD/target/debug \
  -o target/debug/metrics_snapshot_demo
```

Linux:

```bash
c++ \
  -std=c++17 \
  -Ibindings/c/include \
  bindings/c/examples/metrics_snapshot_demo.cpp \
  -Ltarget/debug \
  -ltikv_client_rust_c \
  -Wl,-rpath,'$ORIGIN' \
  -o target/debug/metrics_snapshot_demo
```

Metrics C++ 封装中，`MetricsSnapshot::get()` 返回 owning
`MetricSample`，默认迭代器产出 `Result<MetricSample>`，可以安全传给其它
模块；只在当前 snapshot 生命周期内临时查看时，可以显式使用
`MetricsSnapshot::get_view()` 或 `MetricsSnapshot::views()` 获取零拷贝的
`MetricSampleView`。

```cpp
for (auto sample_result : snapshot) {
  MetricSample sample = sample_result.take();
}

for (auto view_result : snapshot.views()) {
  MetricSampleView view = view_result.value();
}
```

## C++ 使用方法


### 创建 Client

```cpp
Result<Client> client_result = Client::connect("127.0.0.1:2379", 5);

if (!client_result.ok()) {
  std::cerr << client_result.status().to_string() << '\n';
  return 1;
}

Client client = client_result.take();
```

多个对象共享同一个 client：

```cpp
Result<Client::SharedPtr> client_result =
    Client::connect_as_shared("127.0.0.1:2379", 5);

if (!client_result.ok()) {
  std::cerr << client_result.status().to_string() << '\n';
  return 1;
}

Client::SharedPtr client = client_result.value();
```

### 写入和读取

```cpp
// 写入
AsyncRequest<Unit> put_request =
    client.put("cpp-demo/key", "cpp-demo/value");

Status put_status = put_request.wait();
if (!put_status.ok()) {
  std::cerr << put_status.to_string() << '\n';
  return 1;
}
```

```cpp
//读取
AsyncRequest<OptionalValue> get_request = client.get("cpp-demo/key");

Result<OptionalValue> value_result = get_request.wait();
if (!value_result.ok()) {
  std::cerr << value_result.status().to_string() << '\n';
  return 1;
}

const OptionalValue &value = value_result.value();
if (value.has_value()) {
  std::cout << value->to_string() << '\n';
}
```

### 自定义 callback

```cpp
struct CallbackState {
  std::promise<Status> done;
};

void on_complete(const Status &status, void *userdata) noexcept {
  CallbackState *state = static_cast<CallbackState *>(userdata);

  // callback 运行在 Rust runtime worker 线程上，不能抛异常。
  // 如果要把状态传回调用线程，建议用 promise、队列或其他线程安全机制。
  try {
    state->done.set_value(status);
  } catch (...) {
  }
}

CallbackState callback_state;
std::future<Status> callback_done = callback_state.done.get_future();

SubmitOptions options;
options.callback = on_complete;
options.userdata = &callback_state;

// userdata 不会被 C++ wrapper 接管，调用方必须保证它活到 callback 结束。
AsyncRequest<Unit> put_request =
    client.put("cpp-demo/callback-key", "cpp-demo/callback-value", options);

if (!put_request.submitted()) {
  std::cerr << put_request.submit_status().to_string() << '\n';
  return 1;
}

// 对于已提交成功的请求，callback 会被调用一次。
// 这里等待 callback 自己发出的通知，而不是只等待 AsyncRequest::wait()。
Status callback_status = callback_done.get();
if (!callback_status.ok()) {
  std::cerr << callback_status.to_string() << '\n';
  return 1;
}
```

带返回值的请求也可以使用 callback；callback 只负责通知终态，结果仍然从
`AsyncRequest` 取：

```cpp
CallbackState callback_state;
std::future<Status> callback_done = callback_state.done.get_future();

SubmitOptions options;
options.callback = on_complete;
options.userdata = &callback_state;

AsyncRequest<OptionalValue> get_request =
    client.get("cpp-demo/key", options);

if (!get_request.submitted()) {
  std::cerr << get_request.submit_status().to_string() << '\n';
  return 1;
}

Status callback_status = callback_done.get();
if (!callback_status.ok()) {
  std::cerr << callback_status.to_string() << '\n';
  return 1;
}

Result<OptionalValue> value_result = get_request.take();
if (!value_result.ok()) {
  std::cerr << value_result.status().to_string() << '\n';
  return 1;
}

const OptionalValue &value = value_result.value();
if (value.has_value()) {
  std::cout << value->to_string() << '\n';
}
```

### 批量读取

```cpp
std::vector<Slice> keys = {
    Slice("cpp-demo/key"),
    Slice("cpp-demo/missing"),
};

AsyncRequest<KvPairs> batch_get_request = client.batch_get(keys);

Result<KvPairs> batch_get_result = batch_get_request.wait();
if (!batch_get_result.ok()) {
  std::cerr << batch_get_result.status().to_string() << '\n';
  return 1;
}

KvPairs pairs = batch_get_result.take();
for (std::size_t index = 0; index < pairs.size(); ++index) {
  KvPairView pair = pairs[index];
  std::cout << pair.key.to_string() << " = "
            << pair.value.to_string() << '\n';
}
```

### 扫描

```cpp
AsyncRequest<KvPairs> scan_request =
    client.scan("cpp-demo/", "cpp-demo0", 128);

Result<KvPairs> scan_result = scan_request.wait();
if (!scan_result.ok()) {
  std::cerr << scan_result.status().to_string() << '\n';
  return 1;
}

KvPairs pairs = scan_result.take();
for (std::size_t index = 0; index < pairs.size(); ++index) {
  KvPairView pair = pairs[index];
  std::cout << pair.key.to_string() << " = "
            << pair.value.to_string() << '\n';
}
```

### 关闭 Client

```cpp
Status shutdown_status = client.shutdown_begin(false);
if (!shutdown_status.ok()) {
  std::cerr << shutdown_status.to_string() << '\n';
  return 1;
}

Status drain_status = client.drain();
if (!drain_status.ok()) {
  std::cerr << drain_status.to_string() << '\n';
  return 1;
}
```

## 运行

先准备一个可访问的 PD endpoint，例如 `127.0.0.1:2379`：

```bash
./target/debug/raw_async_demo 127.0.0.1:2379
./target/debug/raw_async_demo_cpp 127.0.0.1:2379
./target/debug/metrics_snapshot_demo 127.0.0.1:2379
```

C++ demo 的参数依次为 PD endpoint、是否使用 default keyspace，以及是否运行
native TTL 集成测试。前两个参数的既有行为保持不变；第三个参数默认为
`false`。开启 TTL 集成测试时，TiKV 必须启用 RawKV TTL，例如使用仓库中的
`config/tikv.toml`（包含 `storage.enable-ttl = true`）。API v2 集群还应使用
default keyspace：

```bash
./target/debug/raw_async_demo_cpp 127.0.0.1:2379 true true
```

TTL 集成测试覆盖单条 TTL 写入与查询、用 `ttl_secs = 0` 清除 TTL、带不同
TTL 的批量 put/delete、真实到期，以及无 TTL key 在等待期间持续存在。测试
使用最长 10 秒的轮询等待到期，不依赖固定时长的 sleep。

`metrics_snapshot_demo` 会在同一进程内发起一次 raw put/get/delete，
然后通过 `tikv_metrics_snapshot_collect` 读取当前进程的 Prometheus
registry，并通过 C++ `MetricName` / `MetricTypeLabel` 封装验证
`tikv_request_total{type="raw_get"}` 以及 raw_get duration histogram
样本是否存在。
