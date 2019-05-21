// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

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

    // When we first create a client we receive a `Connect` structure which must be resolved before
    // the client is actually connected and usable.
    let unconnnected_client = Client::new(config);
    let client = unconnnected_client.wait()?;

    // Requests are created from the connected client. These calls return structures which
    // implement `Future`. This means the `Future` must be resolved before the action ever takes
    // place.
    //
    // Here we set the key `TiKV` to have the value `Rust` associated with it.
    let put_request = client.put(KEY, VALUE);
    put_request.wait()?; // Returns a `tikv_client::Error` on failure.
    println!("Put key {:?}, value {:?}.", KEY, VALUE);

    // Unlike a standard Rust HashMap all calls take owned values. This is because under the hood
    // protobufs must take ownership of the data. If we only took a borrow we'd need to internally
    // clone it. This is against Rust API guidelines, so you must manage this yourself.
    //
    // Above, you saw we can use a `&'static str`, this is primarily for making examples short.
    // This type is practical to use for real things, and usage forces an internal copy.
    //
    // It is best to pass a `Vec<u8>` in terms of explictness and speed. `String`s and a few other
    // types are supported as well, but it all ends up as `Vec<u8>` in the end.
    let value: Option<Value> = client.get(KEY).wait()?;
    assert_eq!(value, Some(Value::from(VALUE)));
    println!("Get key {:?} returned value {:?}.", Key::from(KEY), value);

    // You can also set the `ColumnFamily` used by the request.
    // This is *advanced usage* and should have some special considerations.
    client.delete(KEY).wait().expect("Could not delete value");
    println!("Key: {:?} deleted", Key::from(KEY));

    // Here we check if the key has been deleted from the key-value store.
    let value: Option<Value> = client
        .get(KEY)
        .wait()
        .expect("Could not get just deleted entry");
    assert!(value.is_none());

    // You can ask to write multiple key-values at the same time, it is much more
    // performant because it is passed in one request to the key-value store.
    let pairs = vec![
        KvPair::from(("k1", "v1")),
        KvPair::from(("k2", "v2")),
        KvPair::from(("k3", "v3")),
    ];
    client.batch_put(pairs).wait().expect("Could not put pairs");

    // Same thing when you want to retrieve multiple values.
    let keys = vec![Key::from("k1"), Key::from("k2")];
    let values = client
        .batch_get(keys.clone())
        .wait()
        .expect("Could not get values");
    println!("Found values: {:?} for keys: {:?}", values, keys);

    // Scanning a range of keys is also possible giving it two bounds
    // it will returns all entries between these two.
    let start = "k1";
    let end = "k2";
    let pairs = client
        .scan(start..=end, 10)
        .key_only()
        .wait()
        .expect("Could not scan");

    let keys: Vec<_> = pairs.into_iter().map(|p| p.key().clone()).collect();
    assert_eq!(&keys, &[Key::from("k1"), Key::from("k2")]);
    println!("Scaning from {:?} to {:?} gives: {:?}", start, end, keys);

    // Cleanly exit.
    Ok(())
}
