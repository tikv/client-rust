// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

mod common;

use crate::common::parse_args;
use slog::Drain;
use tikv_client::{Config, Key, TransactionClient as Client, TransactionOptions, Value};

#[tokio::main]
async fn main() {
    // You can try running this example by passing your pd endpoints
    // (and SSL options if necessary) through command line arguments.
    let args = parse_args("txn");

    let logger = {
        let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
        slog::Logger::root(
            slog_term::FullFormat::new(plain)
                .build()
                .filter_level(slog::Level::Debug)
                .fuse(),
            slog::o!(),
        )
    };

    // Create a configuration to use for the example.
    // Optionally encrypt the traffic.
    let config = if let (Some(ca), Some(cert), Some(key)) = (args.ca, args.cert, args.key) {
        Config::default().with_security(ca, cert, key)
    } else {
        Config::default()
    };

    // init
    let client = Client::new_with_config(args.pd, config, Some(logger))
        .await
        .expect("Could not connect to tikv");

    // let key0: Key = b"key00".to_vec().into();
    let key0: Key = 0x00000000_u32.to_be_bytes().to_vec().into();
    let value0: Value = b"value0".to_vec();
    // let key1: Key = b"key01".to_vec().into();
    let key1: Key = 0x40000000_u32.to_be_bytes().to_vec().into();
    let value1: Value = b"value1".to_vec();
    // let key2: Key = b"key02".to_vec().into();
    let key2: Key = 0x80000000_u32.to_be_bytes().to_vec().into();
    let value2: Value = b"value2".to_vec();
    let mut txn0 = client
        .begin_optimistic()
        .await
        .expect("Could not begin a transaction");
    for (key, value) in vec![
        (key1.clone(), value1),
        (key2.clone(), value2),
        (key0.clone(), value0),
    ] {
        txn0.put(key, value).await.expect("Could not set key value");
    }
    txn0.commit().await.expect("Could not commit");
    drop(txn0);
    let mut txn1 = client
        .begin_pessimistic()
        .await
        .expect("Could not begin a transaction");
    // lock the key
    let value = txn1
        .get_for_update(key1.clone())
        .await
        .expect("Could not get_and_lock the key");
    println!("{:?}", (&key1, value));
    {
        // another txn cannot write to the locked key
        let mut txn2 = client
            .begin_with_options(TransactionOptions::new_optimistic().no_resolve_locks())
            .await
            .expect("Could not begin a transaction");
        let value2: Value = b"value2".to_vec();
        txn2.put(key1.clone(), value2).await.unwrap();
        let result = txn2.commit().await;
        println!("txn2: {:?}", result);
        assert!(result.is_err());
    }

    // txn4: partial successful locking
    let mut txn4 = client
        .begin_with_options(TransactionOptions::new_pessimistic().no_resolve_locks())
        .await
        .expect("Could not begin a transaction");
    let result = txn4
        .lock_keys(vec![key0.clone(), key2.clone(), key1.clone()])
        .await;
    println!("txn4: {:?}", result);
    // assert!(result.is_err());
    // txn4.rollback().await.unwrap();
    // let value4: Value = b"value4".to_vec();
    // txn4.put(key1.clone(), value4).await.unwrap();
    // let result = txn4.commit().await;

    {
        // txn5: lock on part of keys in txn4
        let mut txn5 = client
            .begin_with_options(TransactionOptions::new_pessimistic().no_resolve_locks())
            .await
            .expect("Could not begin a transaction");
        println!("txn5 begin");
        let result = txn5.lock_keys(vec![key0, key2]).await;
        println!("txn5: {:?}", result);
        // assert!(result.is_ok());
        txn5.rollback().await.unwrap();
    }

    txn4.rollback().await.unwrap();

    // while this txn can still write it
    let value3: Value = b"value3".to_vec();
    txn1.put(key1.clone(), value3).await.unwrap();
    txn1.commit().await.unwrap();
    let mut txn3 = client
        .begin_optimistic()
        .await
        .expect("Could not begin a transaction");
    let result = txn3.get(key1.clone()).await.unwrap().unwrap();
    txn3.commit()
        .await
        .expect("Committing read-only transaction should not fail");
    println!("{:?}", (key1, result));
}
