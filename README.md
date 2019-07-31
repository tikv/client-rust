# TiKV Client (Rust)

[![Build Status](https://travis-ci.org/tikv/client-rust.svg?branch=master)](https://travis-ci.org/tikv/client-rust)
[![Documentation](https://docs.rs/tikv-client/badge.svg)](https://docs.rs/tikv-client/)

> Currently this crate is experimental and some portions (e.g. the Transactional API) are still in active development. You're encouraged to use this library for testing and to help us find problems!

This crate provides a clean, ready to use client for [TiKV](https://github.com/tikv/tikv), a
distributed transactional Key-Value database written in Rust.

With this crate you can easily connect to any TiKV deployment, interact with it, and mutate the data it contains.

This is an open source (Apache 2) project hosted by the Cloud Native Computing Foundation (CNCF) and maintained by the TiKV Authors. *We'd love it if you joined us in improving this project.*

## Using the client

The TiKV client is a Rust library (crate). It uses async/await internally and exposes some `async fn` APIs as well.

Async/await is a new feature in Rust and is currently unstable. To use it you'll need to add the feature flag `#![async_await]` to your crate and use a nightly compiler (see below).

To use this crate in your project, add it as a dependency in your `Cargo.toml`:

```toml
[dependencies]
# ...Your other dependencies...
tikv-client = { git = "https://github.com/tikv/client-rust.git" }
```

The client requires a Git dependency until we can [publish it](https://github.com/tikv/client-rust/issues/32).

There are [examples](examples) which show how to use the client in a Rust program.

## Access the documentation

We recommend using the cargo-generated documentation to browse and understand the API. We've done
our best to include ample, tested, and understandable examples.

You can visit [docs.rs/tikv-client](https://docs.rs/tikv-client/), or build the documentation yourself.

You can access the documentation on your machine by running the following in any project that depends on `tikv-client`.

```bash
cargo doc --package tikv-client --open
# If it didn't work, browse file URL it tried to open with your browser.
```

## Toolchain versions

To check what version of Rust you are using, run

```bash
rustc --version
```

You'll see something like `rustc 1.38.0-nightly (dddb7fca0 2019-07-30)` where the `1.38.0` is the toolchain version, and `nightly` is the channel (stable/beta/nightly). To install another toolchain use

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
