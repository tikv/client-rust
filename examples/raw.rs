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

mod common;

use crate::common::parse_args;
use futures::future::Future;
use tikv_client::{raw::Client, Config, Key, KvPair, Result, Value};

const KEY: &str = "TiKV";
const VALUE: &str = "Rust";

fn main() -> Result<()> {
    // You can try running this example by passing your pd endpoints
    // (and SSL options if necessary) through command line arguments.
    let args = parse_args("raw");

    // Create a configuration to use for the example.
    // Optionally encrypt the traffic.
    let config = if let (Some(ca), Some(cert), Some(key)) = (args.ca, args.cert, args.key) {
        Config::new(args.pd).with_security(ca, cert, key)
    } else {
        Config::new(args.pd)
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
    let value: Value = client.get(key.clone()).wait()?.expect("value must exist");
    assert_eq!(value.as_ref(), VALUE.as_bytes());
    println!("Get key \"{:?}\" returned value \"{:?}\".", value, KEY);

    // You can also set the `ColumnFamily` used by the request.
    // This is *advanced usage* and should have some special considerations.
    client
        .delete(key.clone())
        .wait()
        .expect("Could not delete value");
    println!("Key: {:?} deleted", key);

    // Get returns None for non-existing key
    assert!(client.get(key).wait()?.is_none());

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
        .wait()
        .expect("Could not get values");
    println!("Found values: {:?} for keys: {:?}", values, keys);

    let start: Key = b"k1".to_vec().into();
    let end: Key = b"k2".to_vec().into();
    client
        .scan(start.clone()..end.clone(), 10)
        .key_only()
        .wait()
        .expect("Could not scan");

    // Cleanly exit.
    Ok(())
}
