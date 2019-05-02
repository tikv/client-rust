// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

const NUM_TEST_KEYS: u32 = 100;
use crate::integration_tests::pd_addr;
use tikv_client::{raw::Client, Config, Key, KvPair, Value};

fn generate_key(id: i32) -> Key {
    format!("testkey_{}", id).into_bytes().into()
}

fn generate_value(id: i32) -> Value {
    format!("testvalue_{}", id).into_bytes().into()
}

async fn wipe_all(client: &Client) {
    let test_key_start = generate_key(0);
    let test_key_end = generate_key(NUM_TEST_KEYS as i32 - 1);
    await!(client.delete_range(test_key_start..test_key_end)).expect("Could not delete test keys");
}

async fn connect() -> Client {
    let client = await!(Client::new(Config::new(pd_addr()))).expect("Could not connect to tikv");
    await!(wipe_all(&client));
    client
}

async fn test_empty(client: &Client) {
    let test_key_start = generate_key(0);
    let test_key_end = generate_key(NUM_TEST_KEYS as i32 - 1);

    assert!(
        await!(client.scan(test_key_start..test_key_end, NUM_TEST_KEYS))
            .expect("Could not scan")
            .is_empty()
    );
}

async fn test_existence<'a>(
    client: &'a Client,
    existing_pairs: &'a [KvPair],
    not_existing_keys: Vec<Key>,
) {
    let test_key_start = generate_key(0);
    let test_key_end = generate_key(NUM_TEST_KEYS as i32 - 1);

    for pair in existing_pairs.iter().map(Clone::clone) {
        let (key, value) = pair.into_inner();
        assert_eq!(
            await!(client.get(key))
                .expect("Could not get value")
                .expect("key doesn't exist"),
            value.clone(),
        );
    }

    for key in not_existing_keys.clone().into_iter() {
        let r = await!(client.get(key)).expect("Cound not get value");
        assert!(r.is_none());
    }

    let mut existing_keys = Vec::with_capacity(existing_pairs.len());
    let mut existing_key_only_pairs = Vec::with_capacity(existing_pairs.len());
    for pair in existing_pairs.iter() {
        let key = pair.key().clone();
        existing_keys.push(key.clone());
        existing_key_only_pairs.push(KvPair::new(key, Value::default()));
    }

    let mut all_keys = existing_keys.clone();
    all_keys.extend_from_slice(&not_existing_keys);

    assert_eq!(
        await!(client.batch_get(all_keys)).expect("Could not get value in batch"),
        existing_pairs,
    );

    assert_eq!(
        await!(client.batch_get(not_existing_keys)).expect("Could not get value in batch"),
        Vec::new(),
    );

    assert_eq!(
        await!(client.scan(test_key_start.clone()..test_key_end.clone(), NUM_TEST_KEYS))
            .expect("Could not scan"),
        existing_pairs,
    );

    assert_eq!(
        await!(client
            .scan(test_key_start.clone()..test_key_end.clone(), NUM_TEST_KEYS)
            .key_only())
        .expect("Could not scan"),
        existing_key_only_pairs,
    );
}

#[runtime::test(runtime_tokio::Tokio)]
async fn basic_raw_test() {
    let client = await!(connect());

    await!(test_empty(&client));

    assert!(await!(client.put(generate_key(0), generate_value(0))).is_ok());
    let existing = &[KvPair::new(generate_key(0), generate_value(0))];
    await!(test_existence(
        &client,
        existing,
        vec![generate_key(1), generate_key(2)],
    ));

    let empty_pairs = Vec::new();
    assert!(await!(client.delete(generate_key(0))).is_ok());
    await!(test_existence(
        &client,
        &empty_pairs,
        vec![generate_key(0), generate_key(1), generate_key(2)],
    ));

    let pairs: Vec<KvPair> = (0..10)
        .map(|i| KvPair::new(generate_key(i), generate_value(i)))
        .collect();
    assert!(await!(client.batch_put(pairs.clone())).is_ok());
    await!(test_existence(
        &client,
        &pairs,
        vec![generate_key(10), generate_key(11), generate_key(12)],
    ));

    let keys: Vec<Key> = vec![generate_key(8), generate_key(9)];
    assert!(await!(client.batch_delete(keys)).is_ok());
    let mut pairs = pairs;
    pairs.truncate(8);
    await!(test_existence(
        &client,
        &pairs,
        vec![generate_key(8), generate_key(9), generate_key(10)],
    ));

    await!(wipe_all(&client));
    await!(test_existence(
        &client,
        &empty_pairs,
        pairs.into_iter().map(|x| x.into_inner().0).collect(),
    ));
}
