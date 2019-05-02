// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(async_await, await_macro)]

mod common;

use crate::common::parse_args;
use futures::{
    future,
    prelude::{StreamExt, TryStreamExt},
    stream, TryFutureExt,
};
use std::ops::RangeBounds;
use tikv_client::{
    transaction::{Client, IsolationLevel},
    Config, Key, KvPair, Value,
};

async fn puts(client: &Client, pairs: impl IntoIterator<Item = impl Into<KvPair>>) {
    let mut txn = client.begin();
    await!(future::join_all(
        pairs
            .into_iter()
            .map(Into::into)
            .map(|p| txn.set(p.key().clone(), p.value().clone()))
    ))
    .into_iter()
    .collect::<Result<Vec<()>, _>>()
    .expect("Could not set key value pairs");
    await!(txn.commit()).expect("Could not commit transaction");
}

async fn get(client: &Client, key: Key) -> Value {
    let txn = client.begin();
    await!(txn.get(key)).expect("Could not get value")
}

// Ignore a spurious warning from rustc (https://github.com/rust-lang/rust/issues/60566).
#[allow(unused_mut)]
async fn scan(client: &Client, range: impl RangeBounds<Key>, mut limit: usize) {
    await!(client
        .begin()
        .scan(range)
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
        .for_each(|pair| { future::ready(println!("{:?}", pair)) }));
}

async fn dels(client: &Client, keys: impl IntoIterator<Item = Key>) {
    let mut txn = client.begin();
    txn.set_isolation_level(IsolationLevel::ReadCommitted);
    let _: Vec<()> = await!(stream::iter(keys.into_iter())
        .then(|p| txn
            .delete(p)
            .unwrap_or_else(|e| panic!("error in delete: {:?}", e)))
        .collect());
    await!(txn.commit()).expect("Could not commit transaction");
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

    let txn = await!(Client::new(config)).expect("Could not connect to tikv");

    // set
    let key1: Key = b"key1".to_vec().into();
    let value1: Value = b"value1".to_vec().into();
    let key2: Key = b"key2".to_vec().into();
    let value2: Value = b"value2".to_vec().into();
    await!(puts(&txn, vec![(key1, value1), (key2, value2)]));

    // get
    let key1: Key = b"key1".to_vec().into();
    let value1 = await!(get(&txn, key1.clone()));
    println!("{:?}", (key1, value1));

    // scan
    let key1: Key = b"key1".to_vec().into();
    await!(scan(&txn, key1.., 10));

    // delete
    let key1: Key = b"key1".to_vec().into();
    let key2: Key = b"key2".to_vec().into();
    await!(dels(&txn, vec![key1, key2]));
}
