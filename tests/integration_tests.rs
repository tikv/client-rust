#![cfg(feature = "integration-tests")]

use failure::Fallible;
use futures::prelude::*;
use rand::{seq::IteratorRandom, thread_rng, Rng};
use serial_test::serial;
use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    env,
};
use tikv_client::{Config, Key, RawClient, Result, TransactionClient, Value};

/// The number of keys that we scan in one iteration.
const SCAN_BATCH_SIZE: u32 = 10000;
const NUM_PEOPLE: u32 = 100;
const NUM_TRNASFER: u32 = 100;

/// Delete all entris in TiKV to leave a clean space for following tests.
/// TiKV does not provide an elegant way to do this, so it is finished by scanning and deletions.
async fn clear_tikv() -> Fallible<()> {
    delete_all_raw().await?;
    delete_all_txn().await?;
    Fallible::Ok(())
}

async fn delete_all_raw() -> Fallible<()> {
    let config = Config::new(pd_addrs());
    let raw_client = RawClient::new(config).await?.with_key_only(true);
    loop {
        println!("in delete raw loop");
        let keys = raw_client
            .scan(vec![]..vec![], SCAN_BATCH_SIZE)
            .await
            .expect("delete_all failed scanning all keys")
            .into_iter()
            .map(|pair| pair.into_key())
            .collect::<Vec<_>>();

        if keys.is_empty() {
            break;
        }

        raw_client
            .batch_delete(keys)
            .await
            .expect("delete_all failed batch_delete");
    }

    Fallible::Ok(())
}

async fn delete_all_txn() -> Fallible<()> {
    let config = Config::new(pd_addrs());
    let txn_client = TransactionClient::new(config).await?;
    let mut txn = txn_client.begin().await?;

    loop {
        println!("in delete txn loop");
        let mut keys = txn
            .scan("".to_owned().."".to_owned(), SCAN_BATCH_SIZE, true)
            .await?
            .peekable();

        if keys.peek().is_none() {
            break;
        }

        // FIXME: scan does not use local buffer? anyone else?

        for kv in keys {
            println!("{:?}", kv.key());
            txn.delete(kv.into_key()).await?;
        }
    }

    txn.commit().await?;
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

    let people = gen_people(NUM_PEOPLE, &mut rng);
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
        let mut alice_balance = get_account(&client, alice.clone()).await?;
        let bob = chosen_people[1];
        let mut bob_balance = get_account(&client, bob.clone()).await?;
        if alice_balance > bob_balance {
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
    }

    // check
    let mut new_sum = 0;
    for person in &people {
        new_sum += get_account(&client, person.clone()).await?;
    }
    assert_eq!(sum, new_sum);
    Fallible::Ok(())
}

// helper function
async fn get_account(client: &RawClient, person: Vec<u8>) -> Fallible<u32> {
    let x = client.get(person).await?.unwrap();
    let boxed_slice = x.into_boxed_slice();
    let array: Box<[u8; 4]> = boxed_slice
        .try_into()
        .expect("Value should not exceed u32 (4 * u8)");
    Fallible::Ok(u32::from_be_bytes(*array))
}

// helper function
fn gen_people(num: u32, rng: &mut impl Rng) -> HashSet<Vec<u8>> {
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
