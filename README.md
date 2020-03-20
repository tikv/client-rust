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

## Minimal Rust Version

This crate supports Rust 1.40 and above.

For development, a nightly Rust compiler is needed to compile the tests.