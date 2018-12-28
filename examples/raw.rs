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
use tikv_client::*;

fn main() {
    let config = Config::new(vec!["127.0.0.1:2379"]).with_security(
        PathBuf::from("/path/to/ca.pem"),
        PathBuf::from("/path/to/client.pem"),
        PathBuf::from("/path/to/client-key.pem"),
    );
    let raw = raw::Client::new(&config)
        .wait()
        .expect("Could not connect to tikv");

    let key: Key = b"Company".to_vec().into();
    let value: Value = b"PingCAP".to_vec().into();

    raw.put(key.clone(), value.clone())
        .cf("test_cf")
        .wait()
        .expect("Could not put kv pair to tikv");
    println!("Successfully put {:?}:{:?} to tikv", key, value);

    let value = raw
        .get(&key)
        .cf("test_cf")
        .wait()
        .expect("Could not get value");
    println!("Found val: {:?} for key: {:?}", value, key);

    raw.delete(&key)
        .cf("test_cf")
        .wait()
        .expect("Could not delete value");
    println!("Key: {:?} deleted", key);

    raw.get(&key)
        .cf("test_cf")
        .wait()
        .expect_err("Get returned value for not existing key");

    let keys = vec![b"k1".to_vec().into(), b"k2".to_vec().into()];

    let values = raw
        .batch_get(&keys)
        .cf("test_cf")
        .wait()
        .expect("Could not get values");
    println!("Found values: {:?} for keys: {:?}", values, keys);

    let start: Key = b"k1".to_vec().into();
    let end: Key = b"k2".to_vec().into();
    raw.scan(&start..&end, 10)
        .cf("test_cf")
        .key_only()
        .wait()
        .expect("Could not scan");

    let ranges = [&start..&end, &start..&end];
    raw.batch_scan(&ranges, 10)
        .cf("test_cf")
        .key_only()
        .wait()
        .expect("Could not batch scan");
}
