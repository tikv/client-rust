#![cfg(feature = "integration-tests")]

use failure::Fallible;
use futures::prelude::*;
use rand::{seq::IteratorRandom, thread_rng, Rng};
use serial_test::serial;
use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    env, iter,
};
use tikv_client::{
    ColumnFamily, Config, Key, KvPair, RawClient, Result, Transaction, TransactionClient, Value,
};

// Parameters used in test
const NUM_PEOPLE: u32 = 100;
const NUM_TRNASFER: u32 = 100;

/// Delete all entris in TiKV to leave a clean space for following tests.
async fn clear_tikv() -> Fallible<()> {
    let cfs = vec![
        ColumnFamily::Default,
        ColumnFamily::Lock,
        ColumnFamily::Write,
    ];
    for cf in cfs {
        let config = Config::new(pd_addrs());
        let raw_client = RawClient::new(config)
            .await?
            .with_key_only(true)
            .with_cf(cf);
        raw_client.delete_range(vec![]..).await?;
    }
    Fallible::Ok(())
}

#[tokio::test]
async fn get_timestamp() -> Fallible<()> {
    const COUNT: usize = 1 << 8; // use a small number to make test fast
    let config = Config::new(pd_addrs());
    let client = TransactionClient::new(config).await?;

    let mut versions = future::join_all((0..COUNT).map(|_| client.current_timestamp()))
        .await
        .into_iter()
        .map(|res| res.map(|ts| (ts.physical << 18) + ts.logical))
        .collect::<Result<Vec<_>>>()?;

    // Each version should be unique
    versions.sort_unstable();
    versions.dedup();
    assert_eq!(versions.len(), COUNT);
    Ok(())
}

// Tests transactional get, put, delete, batch_get
#[tokio::test]
#[serial]
async fn crud() -> Fallible<()> {
    clear_tikv().await?;
    let config = Config::new(pd_addrs());

    let client = TransactionClient::new(config).await?;
    let mut txn = client.begin().await?;
    // Get non-existent keys
    assert!(txn.get("foo".to_owned()).await?.is_none());
    assert_eq!(
        txn.batch_get(vec!["foo".to_owned(), "bar".to_owned()])
            .await?
            .filter(|(_, v)| v.is_some())
            .count(),
        0
    );

    txn.put("foo".to_owned(), "bar".to_owned()).await?;
    txn.put("bar".to_owned(), "foo".to_owned()).await?;
    // Read buffered values
    assert_eq!(
        txn.get("foo".to_owned()).await?,
        Some("bar".to_owned().into())
    );
    let batch_get_res: HashMap<Key, Value> = txn
        .batch_get(vec!["foo".to_owned(), "bar".to_owned()])
        .await?
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect();
    assert_eq!(
        batch_get_res.get(&Key::from("foo".to_owned())),
        Some(Value::from("bar".to_owned())).as_ref()
    );
    assert_eq!(
        batch_get_res.get(&Key::from("bar".to_owned())),
        Some(Value::from("foo".to_owned())).as_ref()
    );
    txn.commit().await?;

    // Read from TiKV then update and delete
    let mut txn = client.begin().await?;
    assert_eq!(
        txn.get("foo".to_owned()).await?,
        Some("bar".to_owned().into())
    );
    let batch_get_res: HashMap<Key, Value> = txn
        .batch_get(vec!["foo".to_owned(), "bar".to_owned()])
        .await?
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect();
    assert_eq!(
        batch_get_res.get(&Key::from("foo".to_owned())),
        Some(Value::from("bar".to_owned())).as_ref()
    );
    assert_eq!(
        batch_get_res.get(&Key::from("bar".to_owned())),
        Some(Value::from("foo".to_owned())).as_ref()
    );
    txn.put("foo".to_owned(), "foo".to_owned()).await?;
    txn.delete("bar".to_owned()).await?;
    txn.commit().await?;

    // Read again from TiKV
    let txn = client.begin().await?;
    let batch_get_res: HashMap<Key, Value> = txn
        .batch_get(vec!["foo".to_owned(), "bar".to_owned()])
        .await?
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect();
    assert_eq!(
        batch_get_res.get(&Key::from("foo".to_owned())),
        Some(Value::from("foo".to_owned())).as_ref()
    );
    assert_eq!(batch_get_res.get(&Key::from("bar".to_owned())), None);
    Fallible::Ok(())
}

/// bank transfer mainly tests raw put and get
#[tokio::test]
#[serial]
async fn raw_bank_transfer() -> Fallible<()> {
    clear_tikv().await?;
    let config = Config::new(pd_addrs());
    let client = RawClient::new(config).await?;
    let mut rng = thread_rng();

    let people = gen_u32_keys(NUM_PEOPLE, &mut rng);
    let mut sum: u32 = 0;
    for person in &people {
        let init = rng.gen::<u8>() as u32;
        sum += init as u32;
        client
            .put(person.clone(), init.to_be_bytes().to_vec())
            .await?;
    }

    // transfer
    for _ in 0..NUM_TRNASFER {
        let chosen_people = people.iter().choose_multiple(&mut rng, 2);
        let alice = chosen_people[0];
        let mut alice_balance = get_u32(&client, alice.clone()).await?;
        let bob = chosen_people[1];
        let mut bob_balance = get_u32(&client, bob.clone()).await?;
        if alice_balance == 0 {
            continue;
        }
        let transfer = rng.gen_range(0, alice_balance);
        alice_balance -= transfer;
        bob_balance += transfer;
        client
            .put(alice.clone(), alice_balance.to_be_bytes().to_vec())
            .await?;
        client
            .put(bob.clone(), bob_balance.to_be_bytes().to_vec())
            .await?;
    }

    // check
    let mut new_sum = 0;
    for person in &people {
        new_sum += get_u32(&client, person.clone()).await?;
    }
    assert_eq!(sum, new_sum);
    Fallible::Ok(())
}

/// Tests transactional API when there are multiple regions.
/// Write large volumes of data to enforce region splitting.
/// In order to test `scan`, data is uniformly inserted.
#[tokio::test]
#[serial]
async fn txn_write_million() -> Fallible<()> {
    const NUM_BITS_TXN: u32 = 7;
    const NUM_BITS_KEY_PER_TXN: u32 = 3;
    let interval = 2u32.pow(32 - NUM_BITS_TXN - NUM_BITS_KEY_PER_TXN);

    clear_tikv().await?;
    let config = Config::new(pd_addrs());
    let client = TransactionClient::new(config).await?;

    for i in 0..2u32.pow(NUM_BITS_TXN) {
        let mut cur = i * 2u32.pow(32 - NUM_BITS_TXN);
        let keys = iter::repeat_with(|| {
            let v = cur;
            cur = cur.overflowing_add(interval).0;
            v
        })
        .map(|u| u.to_be_bytes().to_vec())
        .take(2usize.pow(NUM_BITS_KEY_PER_TXN))
        .collect::<Vec<_>>(); // each txn puts 2 ^ 12 keys. 12 = 25 - 13
        let mut txn = client.begin().await?;
        for (k, v) in keys.iter().zip(iter::repeat(1u32.to_be_bytes().to_vec())) {
            txn.put(k.clone(), v).await?;
        }
        txn.commit().await?;

        let mut txn = client.begin().await?;
        let res = txn.batch_get(keys).await?;
        assert_eq!(res.count(), 2usize.pow(NUM_BITS_KEY_PER_TXN));
        txn.commit().await?;
    }

    // test scan
    let limit = 2u32.pow(NUM_BITS_KEY_PER_TXN + NUM_BITS_TXN + 2); // large enough
    let txn = client.begin().await?;
    let res = txn.scan(vec![].., limit).await?;
    assert_eq!(res.count(), 2usize.pow(NUM_BITS_KEY_PER_TXN + NUM_BITS_TXN));

    // scan by small range and combine them
    let mut rng = thread_rng();
    let mut keys = gen_u32_keys(10, &mut rng)
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();

    let mut sum = 0;

    // empty key to key[0]
    let txn = client.begin().await?;
    let res = txn.scan(vec![]..keys[0].clone(), limit).await?;
    sum += res.count();

    // key[i] .. key[i+1]
    for i in 0..keys.len() - 1 {
        let res = txn
            .scan(keys[i].clone()..keys[i + 1].clone(), limit)
            .await?;
        sum += res.count();
    }

    // keys[last] to unbounded
    let res = txn.scan(keys[keys.len() - 1].clone().., limit).await?;
    sum += res.count();

    assert_eq!(sum, 2usize.pow(NUM_BITS_KEY_PER_TXN + NUM_BITS_TXN));

    Fallible::Ok(())
}

#[tokio::test]
#[serial]
async fn txn_bank_transfer() -> Fallible<()> {
    clear_tikv().await?;
    let config = Config::new(pd_addrs());
    let client = TransactionClient::new(config).await?;
    let mut rng = thread_rng();

    let people = gen_u32_keys(NUM_PEOPLE, &mut rng);
    let mut txn = client.begin().await?;
    let mut sum: u32 = 0;
    for person in &people {
        let init = rng.gen::<u8>() as u32;
        sum += init as u32;
        txn.put(person.clone(), init.to_be_bytes().to_vec()).await?;
    }
    txn.commit().await?;

    // transfer
    for _ in 0..NUM_TRNASFER {
        let mut txn = client.begin().await?;
        let chosen_people = people.iter().choose_multiple(&mut rng, 2);
        let alice = chosen_people[0];
        let mut alice_balance = get_txn_u32(&txn, alice.clone()).await?;
        let bob = chosen_people[1];
        let mut bob_balance = get_txn_u32(&txn, bob.clone()).await?;
        if alice_balance == 0 {
            continue;
        }
        let transfer = rng.gen_range(0, alice_balance);
        alice_balance -= transfer;
        bob_balance += transfer;
        txn.put(alice.clone(), alice_balance.to_be_bytes().to_vec())
            .await?;
        txn.put(bob.clone(), bob_balance.to_be_bytes().to_vec())
            .await?;
        txn.commit().await?;
    }

    // check
    let mut new_sum = 0;
    let mut txn = client.begin().await?;
    for person in people.iter() {
        new_sum += get_txn_u32(&txn, person.clone()).await?;
    }
    assert_eq!(sum, new_sum);
    txn.commit().await?;
    Fallible::Ok(())
}

#[tokio::test]
#[serial]
async fn raw_req() -> Fallible<()> {
    clear_tikv().await?;
    let config = Config::new(pd_addrs());
    let client = RawClient::new(config).await?;

    // empty; get non-existent key
    let res = client.get("k1".to_owned()).await;
    assert_eq!(res?, None);

    // empty; put then batch_get
    client.put("k1".to_owned(), "v1".to_owned()).await?;
    client.put("k2".to_owned(), "v2".to_owned()).await?;

    let res = client
        .batch_get(vec!["k1".to_owned(), "k2".to_owned(), "k3".to_owned()])
        .await?;
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].1, "v1".as_bytes());
    assert_eq!(res[1].1, "v2".as_bytes());

    // k1,k2; batch_put then batch_get
    let _ = client
        .batch_put(vec![
            ("k3".to_owned(), "v3".to_owned()),
            ("k4".to_owned(), "v4".to_owned()),
        ])
        .await?;

    let res = client
        .batch_get(vec!["k4".to_owned(), "k3".to_owned()])
        .await?;
    assert_eq!(res[0], KvPair::new("k3".to_owned(), "v3"));
    assert_eq!(res[1], KvPair::new("k4".to_owned(), "v4"));

    // k1,k2,k3,k4; delete then get
    let res = client.delete("k3".to_owned()).await;
    assert!(res.is_ok());

    let res = client.get("k3".to_owned()).await?;
    assert_eq!(res, None);

    // k1,k2,k4; batch_delete then batch_get
    let res = client
        .batch_delete(vec![
            "k1".to_owned(),
            "k2".to_owned(),
            "k3".to_owned(),
            "k4".to_owned(),
        ])
        .await;
    assert!(res.is_ok());

    let res = client
        .batch_get(vec![
            "k1".to_owned(),
            "k2".to_owned(),
            "k3".to_owned(),
            "k4".to_owned(),
        ])
        .await?;
    assert_eq!(res.len(), 0);

    // empty; batch_put then scan
    let _ = client
        .batch_put(vec![
            ("k3".to_owned(), "v3".to_owned()),
            ("k5".to_owned(), "v5".to_owned()),
            ("k1".to_owned(), "v1".to_owned()),
            ("k2".to_owned(), "v2".to_owned()),
            ("k4".to_owned(), "v4".to_owned()),
        ])
        .await?;

    let res = client.scan("k2".to_owned()..="k5".to_owned(), 5).await?;
    assert_eq!(res.len(), 4);
    assert_eq!(res[0].1, "v2".as_bytes());
    assert_eq!(res[1].1, "v3".as_bytes());
    assert_eq!(res[2].1, "v4".as_bytes());
    assert_eq!(res[3].1, "v5".as_bytes());

    let res = client.scan("k2".to_owned().."k5".to_owned(), 2).await?;
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].1, "v2".as_bytes());
    assert_eq!(res[1].1, "v3".as_bytes());

    let res = client.scan("k1".to_owned().., 20).await?;
    assert_eq!(res.len(), 5);
    assert_eq!(res[0].1, "v1".as_bytes());
    assert_eq!(res[1].1, "v2".as_bytes());
    assert_eq!(res[2].1, "v3".as_bytes());
    assert_eq!(res[3].1, "v4".as_bytes());
    assert_eq!(res[4].1, "v5".as_bytes());

    let res = client
        .batch_scan(
            vec![
                "".to_owned().."k1".to_owned(),
                "k1".to_owned().."k2".to_owned(),
                "k2".to_owned().."k3".to_owned(),
                "k3".to_owned().."k4".to_owned(),
                "k4".to_owned().."k5".to_owned(),
            ],
            2,
        )
        .await?;
    assert_eq!(res.len(), 4);

    let res = client
        .batch_scan(
            vec![
                "".to_owned()..="k3".to_owned(),
                "k2".to_owned()..="k5".to_owned(),
            ],
            4,
        )
        .await?;
    assert_eq!(res.len(), 7);
    assert_eq!(res[0].1, "v1".as_bytes());
    assert_eq!(res[1].1, "v2".as_bytes());
    assert_eq!(res[2].1, "v3".as_bytes());
    assert_eq!(res[3].1, "v2".as_bytes());
    assert_eq!(res[4].1, "v3".as_bytes());
    assert_eq!(res[5].1, "v4".as_bytes());
    assert_eq!(res[6].1, "v5".as_bytes());

    Fallible::Ok(())
}

// helper function
async fn get_u32(client: &RawClient, key: Vec<u8>) -> Fallible<u32> {
    let x = client.get(key).await?.unwrap();
    let boxed_slice = x.into_boxed_slice();
    let array: Box<[u8; 4]> = boxed_slice
        .try_into()
        .expect("Value should not exceed u32 (4 * u8)");
    Fallible::Ok(u32::from_be_bytes(*array))
}

// helper function
async fn get_txn_u32(txn: &Transaction, key: Vec<u8>) -> Fallible<u32> {
    let x = txn.get(key).await?.unwrap();
    let boxed_slice = x.into_boxed_slice();
    let array: Box<[u8; 4]> = boxed_slice
        .try_into()
        .expect("Value should not exceed u32 (4 * u8)");
    Fallible::Ok(u32::from_be_bytes(*array))
}

// helper function
fn gen_u32_keys(num: u32, rng: &mut impl Rng) -> HashSet<Vec<u8>> {
    let mut set = HashSet::new();
    for _ in 0..num {
        set.insert(rng.gen::<u32>().to_be_bytes().to_vec());
    }
    set
}

const ENV_PD_ADDRS: &str = "PD_ADDRS";

fn pd_addrs() -> Vec<String> {
    env::var(ENV_PD_ADDRS)
        .expect(&format!("Expected {}:", ENV_PD_ADDRS))
        .split(",")
        .map(From::from)
        .collect()
}
