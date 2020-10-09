// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

mod common;

use crate::common::parse_args;
use tikv_client::{Config, Key, TransactionClient as Client, Value};

#[tokio::main]
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

    // init
    let client = Client::new(config)
        .await
        .expect("Could not connect to tikv");

    let key1: Key = b"key1".to_vec().into();
    let value1: Value = b"value1".to_vec().into();
    let key2: Key = b"key2".to_vec().into();
    let value2: Value = b"value2".to_vec().into();
    let txn = client.begin().await.expect("Could not begin a transaction");
    for (key, value) in vec![(key1, value1), (key2, value2)] {
        txn.put(key, value).await.expect("Could not set key value");
    }

    let mut txn = client.begin().await.expect("Could not begin a transaction");
    // lock the key
    let key1: Key = b"key1".to_vec().into();
    txn.pessimistic_lock(vec![key1.clone()]).await.expect("Could not lock the key");
    {
        // another txn cannot write to the locked key
        let mut txn = client.begin().await.expect("Could not begin a transaction");
        let key1: Key = b"key1".to_vec().into();
        let value2: Value = b"value2".to_vec().into();
        txn.put(key1, value2).await.unwrap();
        let result = txn.commit().await;
        assert!(result.is_err());
    }
    // while this txn can still write it
    let value3: Value = b"value3".to_vec().into();
    let result = txn.put(key1, value3).await;
    assert!(result.is_ok());
    println!("done");
}
