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

use futures::future::Future;
use std::path::PathBuf;
use tikv_client::{raw::Client, Config, Key, Result, Value};

const KEY: &str = "TiKV";
const VALUE: &str = "Rust";
const CUSTOM_CF: &str = "custom_cf";

fn main() -> Result<()> {
    // Create a configuration to use for the example.
    // Optionally encrypt the traffic.
    let config = Config::new(vec![
        "192.168.0.101:3379", // Avoid a single point of failure,
        "192.168.0.100:3379", // use at least two PD endpoints.
    ])
    .with_security(
        PathBuf::from("/path/to/ca.pem"),
        PathBuf::from("/path/to/client.pem"),
        PathBuf::from("/path/to/client-key.pem"),
    );

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
    let put_result: () = put_request.wait()?; // Returns a `tikv_client::Error` on failure.
    println!("Put key \"{}\", value \"{}\".", KEY, VALUE);

    //
    // Unlike a standard Rust HashMap all calls take owned values. This is because under the hood
    // protobufs must take ownership of the data. If we only took a borrow we'd need to internally // clone it. This is against Rust API guidelines, so you must manage this yourself.
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
    let req = client
        .delete(key.clone())
        .cf(CUSTOM_CF)
        .wait()
        .expect("Could not delete value");
    println!("Key: {:?} deleted", key);

    client
        .get(key)
        .cf("test_cf")
        .wait()
        .expect_err("Get returned value for not existing key");

    let keys = vec![Key::from(b"k1".to_vec()), Key::from(b"k2".to_vec())];

    let values = client
        .batch_get(keys.clone())
        .cf("test_cf")
        .wait()
        .expect("Could not get values");
    println!("Found values: {:?} for keys: {:?}", values, keys);

    let start: Key = b"k1".to_vec().into();
    let end: Key = b"k2".to_vec().into();
    client
        .scan(start.clone()..end.clone(), 10)
        .cf("test_cf")
        .key_only()
        .wait()
        .expect("Could not scan");

    let ranges = vec![start.clone()..end.clone(), start.clone()..end.clone()];
    client
        .batch_scan(ranges, 10)
        .cf("test_cf")
        .key_only()
        .wait()
        .expect("Could not batch scan");

    // Cleanly exit.
    Ok(())
}
