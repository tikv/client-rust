// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{arb_batch, integration::pd_addr};
use futures::executor::block_on;
use proptest::{arbitrary::any, proptest};
use tikv_client::{raw::Client, Config, KvPair, Value};

proptest! {
    #[test]
    fn point(
        pair in any::<KvPair>(),
    ) {
        let client = block_on(Client::connect(Config::new(pd_addr()))).unwrap();

        block_on(
            client.put(pair.key().clone(), pair.value().clone())
        ).unwrap();

        let out_value = block_on(
            client.get(pair.key().clone())
        ).unwrap();
        assert_eq!(Some(Value::from(pair.value().clone())), out_value);

        block_on(
            client.delete(pair.key().clone())
        ).unwrap();
    }
}

proptest! {
    #[test]
    fn batch(
        kvs in arb_batch(any::<KvPair>(), None),
    ) {
        let client = block_on(Client::connect(Config::new(pd_addr()))).unwrap();
        let keys = kvs.iter().map(|kv| kv.key()).cloned().collect::<Vec<_>>();

        block_on(
            client.batch_put(kvs.clone())
        ).unwrap();

        let out_value = block_on(
            client.batch_get(keys.clone())
        ).unwrap();
        assert_eq!(kvs, out_value);

        block_on(
            client.batch_delete(keys.clone())
        ).unwrap();
    }
}

proptest! {
    #[test]
    fn scan(
        kvs in arb_batch(any::<KvPair>(), None),
    ) {
        let client = block_on(Client::connect(Config::new(pd_addr()))).unwrap();
        let mut keys = kvs.iter().map(|kv| kv.key()).cloned().collect::<Vec<_>>();
        keys.sort();
        // If we aren't getting values this is an empty vector, so use dummy values.
        let start_key = keys.iter().cloned().next().unwrap_or(vec![0].into());
        let end_key = keys.iter().cloned().last().unwrap_or(vec![0].into());

        block_on(
            client.batch_put(kvs.clone())
        ).unwrap();

        let out_value = block_on(
            client.scan(start_key..=end_key, 10240) // Magic max number is TiKV intrinsic
        ).unwrap();

        // Since TiKV returns empty keys as tombstones in scans, we just check we can find all the items.
        assert!(kvs.iter().all(|kv| {
            out_value.iter().find(|out| kv == *out).is_some()
        }));

        block_on(
            client.batch_delete(keys.clone())
        ).unwrap();
    }
}
