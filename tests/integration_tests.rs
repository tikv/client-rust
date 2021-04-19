#![cfg(feature = "integration-tests")]

//! # Naming convention
//!
//! Test names should begin with one of the following:
//! 1. txn_
//! 2. raw_
//! 3. misc_
//!
//! If in the future Rust supports filter test by regex, we can remove the misc_ category.
//!
//! We make use of the convention to control the order of tests in CI, to force
//! some intialization (e.g. region split), and to allow transactional and raw
//! tests to coexist, since transactional requests have requirements on the
//! region boundaries.

mod common;
use common::{init, pd_addrs};
use futures::prelude::*;
use rand::{seq::IteratorRandom, thread_rng, Rng};
use serial_test::serial;
use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    iter,
};
use tikv_client::{
    Key, KvPair, RawClient, Result, Transaction, TransactionClient, TransactionOptions, Value,
};

// Parameters used in test
const NUM_PEOPLE: u32 = 100;
const NUM_TRNASFER: u32 = 100;

#[tokio::test]
#[serial]
async fn txn_get_timestamp() -> Result<()> {
    const COUNT: usize = 1 << 8; // use a small number to make test fast
    let client = TransactionClient::new(pd_addrs()).await?;

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
async fn txn_crud() -> Result<()> {
    init().await?;

    let client = TransactionClient::new(pd_addrs()).await?;
    let mut txn = client.begin_optimistic().await?;

    // Get non-existent keys
    assert!(txn.get("foo".to_owned()).await?.is_none());

    // batch_get do not return non-existent entries
    assert_eq!(
        txn.batch_get(vec!["foo".to_owned(), "bar".to_owned()])
            .await?
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
        .map(|pair| (pair.0, pair.1))
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
    let mut txn = client.begin_optimistic().await?;
    assert_eq!(
        txn.get("foo".to_owned()).await?,
        Some("bar".to_owned().into())
    );
    let batch_get_res: HashMap<Key, Value> = txn
        .batch_get(vec!["foo".to_owned(), "bar".to_owned()])
        .await?
        .map(|pair| (pair.0, pair.1))
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
    let mut snapshot = client.snapshot(
        client.current_timestamp().await?,
        // TODO needed because pessimistic does not check locks (#235)
        TransactionOptions::new_optimistic(),
    );
    let batch_get_res: HashMap<Key, Value> = snapshot
        .batch_get(vec!["foo".to_owned(), "bar".to_owned()])
        .await?
        .map(|pair| (pair.0, pair.1))
        .collect();
    assert_eq!(
        batch_get_res.get(&Key::from("foo".to_owned())),
        Some(Value::from("foo".to_owned())).as_ref()
    );
    assert_eq!(batch_get_res.get(&Key::from("bar".to_owned())), None);
    Ok(())
}

// Tests transactional insert and delete-your-writes cases
#[tokio::test]
#[serial]
async fn txn_insert_duplicate_keys() -> Result<()> {
    init().await?;

    let client = TransactionClient::new(pd_addrs()).await?;
    // Initialize TiKV store with {foo => bar}
    let mut txn = client.begin_optimistic().await?;
    txn.put("foo".to_owned(), "bar".to_owned()).await?;
    txn.commit().await?;
    // Try insert foo again
    let mut txn = client.begin_optimistic().await?;
    txn.insert("foo".to_owned(), "foo".to_owned()).await?;
    assert!(txn.commit().await.is_err());

    // Delete-your-writes
    let mut txn = client.begin_optimistic().await?;
    txn.insert("foo".to_owned(), "foo".to_owned()).await?;
    txn.delete("foo".to_owned()).await?;
    assert!(txn.commit().await.is_err());

    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_pessimistic() -> Result<()> {
    init().await?;

    let client = TransactionClient::new(pd_addrs()).await?;
    let mut txn = client.begin_pessimistic().await?;
    txn.put("foo".to_owned(), "foo".to_owned()).await.unwrap();

    let ttl = txn.send_heart_beat().await.unwrap();
    assert!(ttl > 0);

    txn.commit().await.unwrap();

    Ok(())
}

/// bank transfer mainly tests raw put and get
#[tokio::test]
#[serial]
async fn raw_bank_transfer() -> Result<()> {
    init().await?;
    let client = RawClient::new(pd_addrs()).await?;
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
        let transfer = rng.gen_range(0..alice_balance);
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
    Ok(())
}

/// Tests transactional API when there are multiple regions.
#[tokio::test]
#[serial]
async fn txn_write_million() -> Result<()> {
    const NUM_BITS_TXN: u32 = 4;
    const NUM_BITS_KEY_PER_TXN: u32 = 4;
    let interval = 2u32.pow(32 - NUM_BITS_TXN - NUM_BITS_KEY_PER_TXN);
    let value = "large_value".repeat(10);

    init().await?;
    let client = TransactionClient::new(pd_addrs()).await?;

    for i in 0..2u32.pow(NUM_BITS_TXN) {
        let mut cur = i * 2u32.pow(32 - NUM_BITS_TXN);
        let keys = iter::repeat_with(|| {
            let v = cur;
            cur = cur.overflowing_add(interval).0;
            v
        })
        .map(|u| u.to_be_bytes().to_vec())
        .take(2usize.pow(NUM_BITS_KEY_PER_TXN))
        .collect::<Vec<_>>();
        let mut txn = client.begin_optimistic().await?;
        for (k, v) in keys.iter().zip(iter::repeat(value.clone())) {
            txn.put(k.clone(), v).await?;
        }
        txn.commit().await?;

        let mut txn = client.begin_optimistic().await?;
        let res = txn.batch_get(keys).await?;
        assert_eq!(res.count(), 2usize.pow(NUM_BITS_KEY_PER_TXN));
        txn.commit().await?;
    }
    // test scan
    let limit = 2u32.pow(NUM_BITS_KEY_PER_TXN + NUM_BITS_TXN + 2); // large enough
    let mut snapshot = client.snapshot(
        client.current_timestamp().await?,
        TransactionOptions::default(),
    );
    let res = snapshot.scan(vec![].., limit).await?;
    assert_eq!(res.count(), 2usize.pow(NUM_BITS_KEY_PER_TXN + NUM_BITS_TXN));

    // scan by small range and combine them
    let mut rng = thread_rng();
    let mut keys = gen_u32_keys(200, &mut rng)
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();

    let mut sum = 0;

    // empty key to key[0]
    let mut snapshot = client.snapshot(
        client.current_timestamp().await?,
        TransactionOptions::default(),
    );
    let res = snapshot.scan(vec![]..keys[0].clone(), limit).await?;
    sum += res.count();

    // key[i] .. key[i+1]
    for i in 0..keys.len() - 1 {
        let res = snapshot
            .scan(keys[i].clone()..keys[i + 1].clone(), limit)
            .await?;
        sum += res.count();
    }

    // keys[last] to unbounded
    let res = snapshot.scan(keys[keys.len() - 1].clone().., limit).await?;
    sum += res.count();

    assert_eq!(sum, 2usize.pow(NUM_BITS_KEY_PER_TXN + NUM_BITS_TXN));

    // test batch_get and batch_get_for_update
    const SKIP_BITS: u32 = 0; // do not retrieve all because there's a limit of message size
    let mut cur = 0u32;
    let keys = iter::repeat_with(|| {
        let v = cur;
        cur = cur.overflowing_add(interval * 2u32.pow(SKIP_BITS)).0;
        v
    })
    .map(|u| u.to_be_bytes().to_vec())
    .take(2usize.pow(NUM_BITS_KEY_PER_TXN + NUM_BITS_TXN - SKIP_BITS))
    .collect::<Vec<_>>();

    let mut txn = client.begin_pessimistic().await?;
    let res = txn.batch_get(keys.clone()).await?.collect::<Vec<_>>();
    assert_eq!(res.len(), keys.len());

    let res = txn.batch_get_for_update(keys.clone()).await?;
    assert_eq!(res.len(), keys.len());

    txn.commit().await?;
    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_bank_transfer() -> Result<()> {
    init().await?;
    let client = TransactionClient::new(pd_addrs()).await?;
    let mut rng = thread_rng();

    let people = gen_u32_keys(NUM_PEOPLE, &mut rng);
    let mut txn = client
        .begin_with_options(TransactionOptions::new_optimistic())
        .await?;
    let mut sum: u32 = 0;
    for person in &people {
        let init = rng.gen::<u8>() as u32;
        sum += init as u32;
        txn.put(person.clone(), init.to_be_bytes().to_vec()).await?;
    }
    txn.commit().await?;

    // transfer
    for _ in 0..NUM_TRNASFER {
        let mut txn = client
            .begin_with_options(TransactionOptions::new_optimistic().use_async_commit())
            .await?;
        let chosen_people = people.iter().choose_multiple(&mut rng, 2);
        let alice = chosen_people[0];
        let mut alice_balance = get_txn_u32(&mut txn, alice.clone()).await?;
        let bob = chosen_people[1];
        let mut bob_balance = get_txn_u32(&mut txn, bob.clone()).await?;
        if alice_balance == 0 {
            txn.rollback().await?;
            continue;
        }
        let transfer = rng.gen_range(0..alice_balance);
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
    let mut txn = client.begin_optimistic().await?;
    for person in people.iter() {
        new_sum += get_txn_u32(&mut txn, person.clone()).await?;
    }
    assert_eq!(sum, new_sum);
    txn.commit().await?;
    Ok(())
}

#[tokio::test]
#[serial]
async fn raw_req() -> Result<()> {
    init().await?;
    let client = RawClient::new(pd_addrs()).await?;

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

    Ok(())
}

/// Only checks if we successfully update safepoint to PD.
#[tokio::test]
#[serial]
async fn txn_update_safepoint() -> Result<()> {
    init().await?;
    let client = TransactionClient::new(pd_addrs()).await?;
    let res = client.gc(client.current_timestamp().await?).await?;
    assert!(res);
    Ok(())
}

/// Tests raw API when there are multiple regions.
/// Write large volumes of data to enforce region splitting.
/// In order to test `scan`, data is uniformly inserted.
///
/// Ignoring this because we don't want to mess up transactional tests.
#[tokio::test]
#[serial]
#[ignore]
async fn raw_write_million() -> Result<()> {
    const NUM_BITS_TXN: u32 = 13;
    const NUM_BITS_KEY_PER_TXN: u32 = 4;
    let interval = 2u32.pow(32 - NUM_BITS_TXN - NUM_BITS_KEY_PER_TXN);

    init().await?;
    let client = RawClient::new(pd_addrs()).await?;

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
        client
            .batch_put(
                keys.iter()
                    .cloned()
                    .zip(iter::repeat(1u32.to_be_bytes().to_vec())),
            )
            .await?;

        let res = client.batch_get(keys).await?;
        assert_eq!(res.len(), 2usize.pow(NUM_BITS_KEY_PER_TXN));
    }

    // test scan
    let limit = 10;
    let res = client.scan(vec![].., limit).await?;
    assert_eq!(res.len(), limit as usize);

    // test batch_scan
    for batch_num in 1..4 {
        let _ = client
            .batch_scan(iter::repeat(vec![]..).take(batch_num), limit)
            .await?;
        // FIXME: `each_limit` parameter does no work as expected.
        // It limits the entries on each region of each rangqe, instead of each range.
        // assert_eq!(res.len(), limit as usize * batch_num);
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_pessimistic_rollback() -> Result<()> {
    init().await?;
    let client = TransactionClient::new_with_config(pd_addrs(), Default::default()).await?;
    let mut preload_txn = client.begin_optimistic().await?;
    let key1 = vec![1];
    let value = key1.clone();

    preload_txn.put(key1.clone(), value).await?;
    preload_txn.commit().await?;

    for _ in 0..100 {
        let mut txn = client.begin_pessimistic().await?;
        let result = txn.get_for_update(key1.clone()).await;
        txn.rollback().await?;
        result?;
    }

    // for _ in 0..100 {
    //     let mut txn = client.begin_pessimistic().await?;
    //     let result = txn
    //         .batch_get_for_update(vec![key1.clone(), key2.clone()])
    //         .await;
    //     txn.rollback().await?;
    //     let _ = result?;
    // }

    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_pessimistic_delete() -> Result<()> {
    init().await?;
    let client =
        TransactionClient::new_with_config(vec!["127.0.0.1:2379"], Default::default()).await?;

    // The transaction will lock the keys and must release the locks on commit, even when values are
    // not written to the DB.
    let mut txn = client.begin_pessimistic().await?;
    txn.put(vec![1], vec![42]).await?;
    txn.delete(vec![1]).await?;
    txn.insert(vec![2], vec![42]).await?;
    txn.delete(vec![2]).await?;
    txn.put(vec![3], vec![42]).await?;
    txn.commit().await?;

    // Check that the keys are not locked.
    let mut txn2 = client.begin_optimistic().await?;
    txn2.put(vec![1], vec![42]).await?;
    txn2.put(vec![2], vec![42]).await?;
    txn2.put(vec![3], vec![42]).await?;
    txn2.commit().await?;

    // As before, but rollback instead of commit.
    let mut txn = client.begin_pessimistic().await?;
    txn.put(vec![1], vec![42]).await?;
    txn.delete(vec![1]).await?;
    txn.delete(vec![2]).await?;
    txn.insert(vec![2], vec![42]).await?;
    txn.delete(vec![2]).await?;
    txn.put(vec![3], vec![42]).await?;
    txn.rollback().await?;

    let mut txn2 = client.begin_optimistic().await?;
    txn2.put(vec![1], vec![42]).await?;
    txn2.put(vec![2], vec![42]).await?;
    txn2.put(vec![3], vec![42]).await?;
    txn2.commit().await?;

    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_lock_keys() -> Result<()> {
    init().await?;
    let client = TransactionClient::new_with_config(pd_addrs(), Default::default()).await?;

    let k1 = b"key1".to_vec();
    let k2 = b"key2".to_vec();
    let v = b"some value".to_vec();

    // optimistic
    let mut t1 = client.begin_optimistic().await?;
    let mut t2 = client.begin_optimistic().await?;
    t1.lock_keys(vec![k1.clone(), k2.clone()]).await?;
    t2.put(k1.clone(), v.clone()).await?;
    t2.commit().await?;
    // must have commit conflict
    assert!(t1.commit().await.is_err());

    // pessimistic
    let k3 = b"key3".to_vec();
    let k4 = b"key4".to_vec();
    let mut t3 = client.begin_pessimistic().await?;
    let mut t4 = client.begin_pessimistic().await?;
    t3.lock_keys(vec![k3.clone(), k4.clone()]).await?;
    assert!(t4.lock_keys(vec![k3.clone(), k4.clone()]).await.is_err());

    t3.rollback().await?;
    t4.lock_keys(vec![k3.clone(), k4.clone()]).await?;
    t4.commit().await?;

    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_get_for_update() -> Result<()> {
    init().await?;
    let client = TransactionClient::new_with_config(pd_addrs(), Default::default()).await?;
    let key1 = "key".to_owned();
    let key2 = "another key".to_owned();
    let value1 = b"some value".to_owned();
    let value2 = b"another value".to_owned();
    let keys = vec![key1.clone(), key2.clone()];

    let mut t1 = client.begin_pessimistic().await?;
    let mut t2 = client.begin_pessimistic().await?;
    let mut t3 = client.begin_optimistic().await?;
    let mut t4 = client.begin_optimistic().await?;
    let mut t0 = client.begin_pessimistic().await?;
    t0.put(key1.clone(), value1).await?;
    t0.put(key2.clone(), value2).await?;
    t0.commit().await?;

    assert!(t1.get(key1.clone()).await?.is_none());
    assert!(t1.get_for_update(key1.clone()).await?.unwrap() == value1);
    t1.commit().await?;

    assert!(t2.batch_get(keys.clone()).await?.collect::<Vec<_>>().len() == 0);
    let res: HashMap<_, _> = t2
        .batch_get_for_update(keys.clone())
        .await?
        .into_iter()
        .map(From::from)
        .collect();
    t2.commit().await?;
    assert!(res.get(&key1.clone().into()).unwrap() == &value1);
    assert!(res.get(&key2.into()).unwrap() == &value2);

    assert!(t3.get_for_update(key1).await?.is_none());
    assert!(t3.commit().await.is_err());

    assert!(t4.batch_get_for_update(keys).await?.len() == 0);
    assert!(t4.commit().await.is_err());

    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_pessimistic_heartbeat() -> Result<()> {
    init().await?;

    let key1 = "key1".to_owned();
    let key2 = "key2".to_owned();
    let client = TransactionClient::new(pd_addrs()).await?;

    let mut heartbeat_txn = client
        .begin_with_options(TransactionOptions::new_pessimistic())
        .await?;
    heartbeat_txn.put(key1.clone(), "foo").await.unwrap();

    let mut txn_without_heartbeat = client
        .begin_with_options(TransactionOptions::new_pessimistic().no_auto_hearbeat())
        .await?;
    txn_without_heartbeat
        .put(key2.clone(), "fooo")
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;

    // use other txns to check these locks
    let mut t3 = client
        .begin_with_options(TransactionOptions::new_optimistic().no_resolve_locks())
        .await?;
    t3.put(key1.clone(), "gee").await?;
    assert!(t3.commit().await.is_err());
    let mut t4 = client.begin_optimistic().await?;
    t4.put(key2.clone(), "geee").await?;
    t4.commit().await?;

    assert!(heartbeat_txn.commit().await.is_ok());
    assert!(txn_without_heartbeat.commit().await.is_err());

    Ok(())
}
// helper function
async fn get_u32(client: &RawClient, key: Vec<u8>) -> Result<u32> {
    let x = client.get(key).await?.unwrap();
    let boxed_slice = x.into_boxed_slice();
    let array: Box<[u8; 4]> = boxed_slice
        .try_into()
        .expect("Value should not exceed u32 (4 * u8)");
    Ok(u32::from_be_bytes(*array))
}

// helper function
async fn get_txn_u32(txn: &mut Transaction, key: Vec<u8>) -> Result<u32> {
    let x = txn.get(key).await?.unwrap();
    let boxed_slice = x.into_boxed_slice();
    let array: Box<[u8; 4]> = boxed_slice
        .try_into()
        .expect("Value should not exceed u32 (4 * u8)");
    Ok(u32::from_be_bytes(*array))
}

// helper function
fn gen_u32_keys(num: u32, rng: &mut impl Rng) -> HashSet<Vec<u8>> {
    let mut set = HashSet::new();
    for _ in 0..num {
        set.insert(rng.gen::<u32>().to_be_bytes().to_vec());
    }
    set
}
