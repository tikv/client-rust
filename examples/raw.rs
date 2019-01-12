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
use futures::future::Future;
use std::path::PathBuf;
use tikv_client::{raw::Client, Config, Key, KvPair, Result, Value};

const KEY: &str = "TiKV";
const VALUE: &str = "Rust";
const CUSTOM_CF: &str = "default";

fn main() -> Result<()> {
    let (pd, security) = parse_args();

    // Create a configuration to use for the example.
    // Optionally encrypt the traffic.
    let config = if let Some((ca, cert, key)) = security {
        Config::new(pd).with_security(ca, cert, key)
    } else {
        Config::new(pd)
    };

    // When we first create a client we recieve a `Connect` structure which must be resolved before
    // the client is actually connected and usable.
    let unconnnected_client = Client::new(&config);
    let client = unconnnected_client.wait()?;

    // Requests are created from the connected client. These calls return structures which
    // implement `Future`. This means the `Future` must be resolved before the action ever takes
    // place.
    //
    // Here we set the key `TiKV` to have the value `Rust` associated with it.
    let put_request = client.put(KEY, VALUE);
    put_request.wait()?; // Returns a `tikv_client::Error` on failure.
    println!("Put key \"{}\", value \"{}\".", KEY, VALUE);

    //
    // Unlike a standard Rust HashMap all calls take owned values. This is because under the hood
    // protobufs must take ownership of the data. If we only took a borrow we'd need to internally
    // clone it. This is against Rust API guidelines, so you must manage this yourself.
    //
    // Above, you saw we can use a `&'static str`, this is primarily for making examples short.
    // This type is practical to use for real things, and usage forces an internal copy.
    //
    // It is best to pass a `Vec<u8>` in terms of explictness and speed. `String`s and a few other
    // types are supported  as well, but it all ends up as `Vec<u8>` in the end.
    let key: String = String::from(KEY);
    let value: Value = client.get(key.clone()).wait()?;
    assert_eq!(value.as_ref(), VALUE.as_bytes());
    println!("Get key \"{:?}\" returned value \"{:?}\".", value, KEY);

    // You can also set the `ColumnFamily` used by the request.
    // This is *advanced usage* and should have some special considerations.
    client
        .delete(key.clone())
        .cf(CUSTOM_CF)
        .wait()
        .expect("Could not delete value");
    println!("Key: {:?} deleted", key);

    client
        .get(key)
        .cf(CUSTOM_CF)
        .wait()
        .expect_err("Get returned value for not existing key");

    let pairs: Vec<KvPair> = (1..3)
        .map(|i| KvPair::from((Key::from(format!("k{}", i)), Value::from(format!("v{}", i)))))
        .collect();
    client
        .batch_put(pairs.clone())
        .wait()
        .expect("Could not put pairs");

    let keys = vec![Key::from(b"k1".to_vec()), Key::from(b"k2".to_vec())];

    let values = client
        .batch_get(keys.clone())
        .cf(CUSTOM_CF)
        .wait()
        .expect("Could not get values");
    println!("Found values: {:?} for keys: {:?}", values, keys);

    let start: Key = b"k1".to_vec().into();
    let end: Key = b"k2".to_vec().into();
    client
        .scan(start.clone()..end.clone(), 10)
        .cf(CUSTOM_CF)
        .key_only()
        .wait()
        .expect("Could not scan");

    let ranges = vec![start.clone()..end.clone(), start.clone()..end.clone()];
    client
        .batch_scan(ranges, 10)
        .cf(CUSTOM_CF)
        .key_only()
        .wait()
        .expect("Could not batch scan");

    // Cleanly exit.
    Ok(())
}

fn parse_args() -> (Vec<String>, Option<(PathBuf, PathBuf, PathBuf)>) {
    let matches = App::new("Raw API Example of the Rust Client for TiKV")
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
