# TiKV Client (Rust)

[![Build Status](https://travis-ci.org/tikv/client-rust.svg?branch=master)](https://travis-ci.org/pingcap/client-rust)
[![Documentation](https://docs.rs/tikv-client/badge.svg)](https://docs.rs/tikv-client/)

> Currently this crate is experimental and some portions (e.g. the Transactional API) are still in active development. You're encouraged to use this library for testing and to help us find problems!

This crate provides a clean, ready to use client for [TiKV](https://github.com/tikv/tikv), a
distributed transactional Key-Value database written in Rust.

With this crate you can easily connect to any TiKV deployment, interact with it, and mutate the data it contains.

This is an open source (Apache 2) project hosted by the Cloud Native Computing Foundation (CNCF) and maintained by the TiKV Authors. *We'd love it if you joined us in improving this project.*

## Using the client

The TiKV client is a Rust library (crate). It requires version 1.36 of the compiler and standard libraries (which will be stable from the 4th July 2019, see below for ensuring compatibility).

To use this crate in your project, add it as a dependency in your `Cargo.toml`:

```toml
[dependencies]
# ...Your other dependencies...
tikv-client = { git = "https://github.com/tikv/client-rust.git" }
```

The client requires a Git dependency until we can [publish it](https://github.com/tikv/client-rust/issues/32).

There are [examples](examples) which show how to use the client in a Rust program.

The examples and documentation use async/await syntax. This is a new feature in Rust and is currently unstable. To use async/await you'll need to add the feature flag `#![async_await]` to your crate and use a nightly compiler (see below).

## Access the documentation

We recommend using the cargo-generated documentation to browse and understand the API. We've done
our best to include ample, tested, and understandable examples.

You can visit [docs.rs/tikv-client](https://docs.rs/tikv-client/), or build the documentation yourself.

You can access the documentation on your machine by running the following in any project that depends on `tikv-client`.

```bash
cargo doc --package tikv-client --open
# If it didn't work, browse file URL it tried to open with your browser.
```

## Running benchmarks

This crate uses [`criterion`](https://github.com/bheisler/criterion.rs) for benchmarking. Most benchmarks use [`proptest`](https://github.com/altsysrq/proptest) to generate values for bench runs.

Currently, all of the benchmarks are gated by the `integration-tests` feature, and require a functioning TiKV (and PD) cluster.

```bash
export PD_ADDRS=192.168.0.100:2379,192.168.0.101:2379,192.168.0.102:2379
cargo +nightly bench --features integration-tests
```

It is possible to limit the scope of benchmarks:

```bash
export PD_ADDRS=192.168.0.100:2379,192.168.0.101:2379,192.168.0.102:2379
cargo +nightly bench --features integration-tests raw
```

## Toolchain versions

To check what version of Rust you are using, run

```bash
rustc --version
```

You'll see something like `rustc 1.36.0-nightly (a784a8022 2019-05-09)` where the `1.36.0` is the toolchain version, and `nightly` is the channel (stable/beta/nightly). To install another toolchain use

```bash
rustup toolchain install nightly
```

Where `nightly` here is the channel to add. To update your toolchains, run

```bash
rustup update
```

To build your project using a specified toolchain, use something like

```bash
cargo +nightly build
```

Where `nightly` names the toolchain (by specifying the channel, in this case).
