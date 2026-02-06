# TSO Waker Criterion Benchmark

Date: 2026-02-06
Repo: `tikv/client-rust`
Branch: `mingley/tso-waker-criterion`
Host: macOS 26.2 (Darwin 25.2.0), Apple M4 Pro, arm64
Rust toolchain: 1.84.1

## Goal

Quantify the latency impact of reducing TSO stream wake/registration churn in
`src/pd/timestamp.rs`.

These numbers are a point-in-time snapshot from this branch. Re-run the
benchmark after meaningful changes to either `src/pd/timestamp.rs` or
`benches/tso_waker_policy.rs`.

## Method

Benchmark framework:
- Criterion (`cargo bench`)

Bench target:
- `benches/tso_waker_policy.rs`

Command used:

```bash
cargo bench --bench tso_waker_policy -- --noplot
```

Criterion configuration in benchmark:
- warmup: 2 seconds
- measurement: 6 seconds
- samples: 100

The benchmark compares old vs new policies in two isolated hot paths:
- `response/*`: wake policy when processing responses
- `register/*`: self-waker registration policy in no-request branch

Note: the old/new response benchmarks intentionally do asymmetric work
(always wake vs transition-only wake), so the speedup reflects the amortized
benefit of skipping redundant wake calls under this simulation pattern.

## Results (Absolute Latency)

From Criterion output (`time` line):

- `tso_waker_policy/response/old`: `[3.2519 ns 3.2712 ns 3.2926 ns]`
- `tso_waker_policy/response/new`: `[763.41 ps 766.39 ps 769.43 ps]`

- `tso_waker_policy/register/old`: `[2.3768 ns 2.3819 ns 2.3874 ns]`
- `tso_waker_policy/register/new`: `[286.76 ps 287.51 ps 288.27 ps]`

Median-based speedups:
- response path: `3.2712 ns / 0.76639 ns = 4.27x`
- registration path: `2.3819 ns / 0.28751 ns = 8.28x`

## Interpretation

The new policy materially reduces per-operation latency in both isolated paths,
with sub-nanosecond median latency for the optimized variants in this synthetic
microbenchmark.

This benchmark is intentionally focused on internal policy overhead. It does not
by itself measure end-to-end PD/TSO RPC latency in a real TiKV deployment.
