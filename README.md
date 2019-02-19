# TiKV Client (Rust)

[![Build Status](https://travis-ci.org/tikv/client-rust.svg?branch=master)](https://travis-ci.org/pingcap/client-rust)
[![Documentation](https://docs.rs/tikv-client/badge.svg)](https://docs.rs/tikv-client/)

> Currently this crate is experimental and some portions (e.g. the Transactional API) are still in active development. You're encouraged to use this library for testing and to help us find problems!

This crate provides a clean, ready to use client for [TiKV](https://github.com/tikv/tikv), a
distributed transactional Key-Value database written in Rust.

With this crate you can easily connect to any TiKV deployment, interact with it, and mutate the data it contains.

This is an open source (Apache 2) project hosted by the Cloud Native Computing Foundation (CNCF) and maintained by the TiKV Authors. *We'd love it if you joined us in improving this project.*

## Install

There are no special requirements to use this. It is a Rust 2018 edition crate supporting stable and nightly.

To use this crate in your project, add it as a dependency in the `Cargo.toml` of your Rust project:

```toml
[dependencies]
# ...Your other dependencies...
tikv-client = "~0.1"
```

## Access the documentation

We recommend using the cargo-generated documentation to browse and understand the API. We've done
our best to include ample, tested, and understandable examples.

You can visit [docs.rs/tikv-client](https://docs.rs/tikv-client/), or build the documentation yourself.

You can access the documentation on your machine by running the following in any project that depends on `tikv-client`.

```bash
cargo doc --package tikv-client --open
# If it didn't work, browse file URL it tried to open with your browser.
```
