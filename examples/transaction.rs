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

use clap::{crate_version, App, Arg};
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
    let (pd, security) = parse_args();

    // Create a configuration to use for the example.
    // Optionally encrypt the traffic.
    let config = if let Some((ca, cert, key)) = security {
        Config::new(pd).with_security(ca, cert, key)
    } else {
        Config::new(pd)
    };

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

fn parse_args() -> (Vec<String>, Option<(PathBuf, PathBuf, PathBuf)>) {
    let matches = App::new("Transactional API Example of the Rust Client for TiKV")
        .version(crate_version!())
        .author("The TiKV Project Authors")
        .arg(
            Arg::with_name("pd")
                .long("pd")
                .aliases(&["pd-endpoint", "pd-endpoints"])
                .value_name("PD_URL")
                .help("Sets PD endpoints")
                .long_help("Sets PD endpoints. Uses `,` to separate multiple PDs")
                .takes_value(true)
                .multiple(true)
                .value_delimiter(",")
                .required(true),
        )
        // A cyclic dependency between CA, cert and key is made
        // to ensure that no security options are missing.
        .arg(
            Arg::with_name("ca")
                .long("ca")
                .value_name("CA_PATH")
                .help("Sets the CA")
                .long_help("Sets the CA. Must be used with --cert and --key")
                .takes_value(true)
                .requires("cert"),
        )
        .arg(
            Arg::with_name("cert")
                .long("cert")
                .value_name("CERT_PATH")
                .help("Sets the certificate")
                .long_help("Sets the certificate. Must be used with --ca and --key")
                .takes_value(true)
                .requires("key"),
        )
        .arg(
            Arg::with_name("key")
                .long("key")
                .alias("private-key")
                .value_name("KEY_PATH")
                .help("Sets the private key")
                .long_help("Sets the private key. Must be used with --ca and --cert")
                .takes_value(true)
                .requires("ca"),
        )
        .get_matches();

    let pd: Vec<_> = matches.values_of("pd").unwrap().map(String::from).collect();
    let security = if let (Some(ca), Some(cert), Some(key)) = (
        matches.value_of("ca"),
        matches.value_of("cert"),
        matches.value_of("key"),
    ) {
        Some((PathBuf::from(ca), PathBuf::from(cert), PathBuf::from(key)))
    } else {
        None
    };
    (pd, security)
}
