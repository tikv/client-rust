// Copyright 2018 The TiKV Project Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

use futures::{future, Future, Stream};
use std::ops::RangeBounds;
use std::path::PathBuf;
use tikv_client::{
    transaction::{Client, IsolationLevel},
    Config, Key, KvPair, Value,
};

fn puts(client: &Client, pairs: impl IntoIterator<Item = impl Into<KvPair>>) {
    let mut txn = client.begin();
    let _: Vec<()> = future::join_all(
        pairs
            .into_iter()
            .map(Into::into)
            .map(|p| txn.set(p.key().clone(), p.value().clone())),
    )
    .wait()
    .expect("Could not set key value pairs");
    txn.commit().wait().expect("Could not commit transaction");
}

fn get(client: &Client, key: Key) -> Value {
    let txn = client.begin();
    txn.get(key).wait().expect("Could not get value")
}

fn scan(client: &Client, range: impl RangeBounds<Key>, mut limit: usize) {
    client
        .begin()
        .scan(range)
        .take_while(move |_| {
            Ok(if limit == 0 {
                false
            } else {
                limit -= 1;
                true
            })
        })
        .for_each(|pair| {
            println!("{:?}", pair);
            Ok(())
        })
        .wait()
        .expect("Could not scan keys");
}

fn dels(client: &Client, keys: impl IntoIterator<Item = Key>) {
    let mut txn = client.begin();
    txn.set_isolation_level(IsolationLevel::ReadCommitted);
    let _: Vec<()> = keys
        .into_iter()
        .map(|p| {
            txn.delete(p).wait().expect("Could not delete key");
        })
        .collect();
    txn.commit().wait().expect("Could not commit transaction");
}

fn main() {
    let config = Config::new(vec!["127.0.0.1:2379"]).with_security(
        PathBuf::from("/path/to/ca.pem"),
        PathBuf::from("/path/to/client.pem"),
        PathBuf::from("/path/to/client-key.pem"),
    );
    let txn = Client::new(&config)
        .wait()
        .expect("Could not connect to tikv");

    // set
    let key1: Key = b"key1".to_vec().into();
    let value1: Value = b"value1".to_vec().into();
    let key2: Key = b"key2".to_vec().into();
    let value2: Value = b"value2".to_vec().into();
    puts(&txn, vec![(key1, value1), (key2, value2)]);

    // get
    let key1: Key = b"key1".to_vec().into();
    let value1 = get(&txn, key1.clone());
    println!("{:?}", (key1, value1));

    // scan
    let key1: Key = b"key1".to_vec().into();
    scan(&txn, key1.., 10);

    // delete
    let key1: Key = b"key1".to_vec().into();
    let key2: Key = b"key2".to_vec().into();
    dels(&txn, vec![key1, key2]);
}
