// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(async_await, await_macro)]

mod common;

use crate::common::parse_args;
use futures::prelude::*;
use std::ops::RangeBounds;
use tikv_client::{Config, Key, KvPair, TransactionClient as Client, Value};

async fn puts(client: &Client, pairs: impl IntoIterator<Item = impl Into<KvPair>>) {
    let mut txn = client.begin().await.expect("Could not begin a transaction");
    for pair in pairs {
        let (key, value) = pair.into().into();
        txn.set(key, value);
    }
    txn.commit().await.expect("Could not commit transaction");
}

async fn get(client: &Client, key: Key) -> Option<Value> {
    let txn = client.begin().await.expect("Could not begin a transaction");
    txn.get(key).await.expect("Could not get value")
}

// Ignore a spurious warning from rustc (https://github.com/rust-lang/rust/issues/60566).
#[allow(unused_mut)]
async fn scan(client: &Client, range: impl RangeBounds<Key>, mut limit: usize) {
    let mut txn = client.begin().await.expect("Could not begin a transaction");
    txn.scan(range)
        .into_stream()
        .take_while(move |r| {
            assert!(r.is_ok(), "Could not scan keys");
            future::ready(if limit == 0 {
                false
            } else {
                limit -= 1;
                true
            })
        })
        .for_each(|pair| future::ready(println!("{:?}", pair)))
        .await;
}

async fn dels(client: &Client, keys: impl IntoIterator<Item = Key>) {
    let mut txn = client.begin().await.expect("Could not begin a transaction");
    for key in keys {
        txn.delete(key);
    }
    txn.commit().await.expect("Could not commit transaction");
}

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    // You can try running this example by passing your pd endpoints
    // (and SSL options if necessary) through command line arguments.
    let args = parse_args("txn");

    // Create a configuration to use for the example.
    // Optionally encrypt the traffic.
    let config = if let (Some(ca), Some(cert), Some(key)) = (args.ca, args.cert, args.key) {
        Config::new(args.pd).with_security(ca, cert, key)
    } else {
        Config::new(args.pd)
    };

    let txn = Client::connect(config)
        .await
        .expect("Could not connect to tikv");

    // set
    let key1: Key = b"key1".to_vec().into();
    let value1: Value = b"value1".to_vec().into();
    let key2: Key = b"key2".to_vec().into();
    let value2: Value = b"value2".to_vec().into();
    puts(&txn, vec![(key1, value1), (key2, value2)]).await;

    // get
    let key1: Key = b"key1".to_vec().into();
    let value1 = get(&txn, key1.clone()).await;
    println!("{:?}", (key1, value1));

    // scan
    let key1: Key = b"key1".to_vec().into();
    scan(&txn, key1.., 10).await;

    // delete
    let key1: Key = b"key1".to_vec().into();
    let key2: Key = b"key2".to_vec().into();
    dels(&txn, vec![key1, key2]).await;
}
