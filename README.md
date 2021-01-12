# TiKV Client (Rust)

[![Build Status](https://travis-ci.org/tikv/client-rust.svg?branch=master)](https://travis-ci.org/tikv/client-rust)

> Currently this crate is experimental and some portions (e.g. the Transactional API) are still in active development. You're encouraged to use this library for testing and to help us find problems!

[Docs](https://www.tikv.dev/doc/rust-client/tikv_client/)

This crate provides a clean, ready to use client for [TiKV](https://github.com/tikv/tikv), a
distributed transactional Key-Value database written in Rust.

With this crate you can easily connect to any TiKV deployment, interact with it, and mutate the data it contains. It uses async/await internally and exposes some `async fn` APIs as well.

This is an open source (Apache 2) project hosted by the Cloud Native Computing Foundation (CNCF) and maintained by the TiKV Authors. *We'd love it if you joined us in improving this project.*

## Getting started

The TiKV client is a Rust library (crate). To use this crate in your project, add following dependencies in your `Cargo.toml`:

```toml
[dependencies]
tikv-client = { git = "https://github.com/tikv/client-rust.git" }
```

The client requires a Git dependency until we can [publish it](https://github.com/tikv/client-rust/issues/32).

The client provides two modes to interact with TiKV: raw and transactional. 
In the current version (0.0.0), the transactional API supports optimistic transactions. Pessimistic transactions are implemented but not well tested.

Important note: It is **not recommended or supported** to use both the raw and transactional APIs on the same database.

### Code examples

Raw mode:

```rust
let config = Config::new(vec!["127.0.0.1:2379"]);
let client = RawClient::new(config).await?;
client.put("key".to_owned(), "value".to_owned()).await;
let value = client.get("key".to_owned()).await;
```

Transactional mode:

```rust
let config = Config::new(vec!["127.0.0.1:2379"]);
let txn_client = TransactionClient::new(config).await?;
let mut txn = txn_client.begin().await?;
txn.put("key".to_owned(), "value".to_owned()).await?;
let value = txn.get("key".to_owned()).await;
txn.commit().await?;
```

There are some [examples](examples) which show how to use the client in a Rust program.

### API

#### Raw requests

| Request        | Main parameter type | Successful result type | Noteworthy Behavior                           |
| -------------- | ------------------- | ---------------------- | --------------------------------------------- |
| `put`          | `KvPair`            | `()`                   |                                               |
| `get`          | `Key`               | `Option<Value>`        |                                               |
| `delete`       | `Key`               | `()`                   |                                               |
| `scan`         | `BoundRange`        | `Vec<KvPair>`          |                                               |
| `batch_put`    | `Iter<KvPair>`      | `()`                   |                                               |
| `batch_get`    | `Iter<Key>`         | `Vec<KvPair>`          | Skip non-existent keys; Does not retain order |
| `batch_delete` | `Iter<Key>`         | `()`                   |                                               |
| `delete_range` | `BoundRange`        | `()`                   |                                               |

#### Transactional requests

| Request     | Main parameter type | Successful result type | Noteworthy Behavior                                          |
| ----------- | ------------------- | ---------------------- | ------------------------------------------------------------ |
| `put`       | `KvPair`            | `()`                   |                                                              |
| `get`       | `Key`               | `Option<value>`        |                                                              |
| `delete`    | `Key`               | `()`                   |                                                              |
| `scan`      | `BoundRange`        | `Iter<KvPair>`         |                                                              |
| `batch_get` | `Iter<Key>`         | `Iter<KvPair>`         | Skip non-existent keys; Does not retain order                |
| `lock_keys` | `Iter<Key>`            | `()`                   |                                                              |
| `gc`        | `Timestamp`         | `bool`                 | It returns whether the latest safepoint in PD equals the parameter |

For detailed behavior of each request, please refer to the [doc](#Access-the-documentation).

#### Experimental raw requests

You must be careful if you want to use the following request(s). Read the description for reasons.

| Request      | Main parameter type | Successful result type |
| ------------ | ------------------- | ---------------------- |
| `batch_scan` | `Iter<BoundRange>`  | `Vec<KvPair>`          |

The `each_limit` parameter does not work as expected. It does not limit the number of results returned of each range, instead it limits the number of results in each region of each range. As a result, you may get **more than** `each_limit` key-value pairs for each range. But you should not miss any entries.

The results of `batch_scan` are flattened. The order of ranges is retained.

### Useful types

To use the client, there are 4 types you will need. 

`Key` is simply a vector of bytes(`Vec<u8>`). `String` and `Vec<u8>` implements `Into<Key>`, so you can directly pass them to clients.

`Value` is just an alias of `Vec<u8>`.

`KvPair` is a tuple consisting of a `Key` and a `Value`. It also provides some convenience methods for conversion to and from other types.

`BoundRange` is used for range related requests like `scan`. It implements `From` for usual ranges so you can just create a range and pass them to the request.For instance, `client.scan("k2".to_owned()..="k5".to_owned(), 5)` or `client.delete_range(vec![]..)`.

## Access the documentation

We've done our best to include ample, tested, and understandable examples.

We recommend using the officially maintained documentation [here](https://www.tikv.dev/doc/rust-client/tikv_client/).

You can also access the documentation on your machine by running the following in any project that depends on `tikv-client`.

```bash
cargo doc --package tikv-client --open
# If it didn't work, browse file URL it tried to open with your browser.
```

## Minimal Rust version

This crate supports Rust 1.40 and above.

For development, a nightly Rust compiler is needed to compile the tests.
