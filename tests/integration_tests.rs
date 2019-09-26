#![cfg(feature = "integration-tests")]

use failure::Fallible;
use futures::executor::{block_on, ThreadPool};
use futures::prelude::*;
use std::collections::HashMap;
use std::env;
use tikv_client::{Config, Key, Result, TransactionClient, Value};

#[test]
fn get_timestamp() -> Fallible<()> {
    const COUNT: usize = 1 << 16;
    let mut pool = ThreadPool::new()?;
    let config = Config::new(pd_addrs());
    let fut = async {
        let client = TransactionClient::new(config).await?;
        Result::Ok(future::join_all((0..COUNT).map(|_| client.current_timestamp())).await)
    };
    // Calculate each version of retrieved timestamp
    let mut versions = pool
        .run(fut)?
        .into_iter()
        .map(|res| res.map(|ts| (ts.physical << 18) + ts.logical))
        .collect::<Result<Vec<_>>>()?;

    // Each version should be unique
    versions.sort_unstable();
    versions.dedup();
    assert_eq!(versions.len(), COUNT);
    Ok(())
}

#[test]
fn crud() -> Fallible<()> {
    let config = Config::new(pd_addrs());
    block_on(async move {
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

        txn.set("foo".to_owned(), "bar".to_owned()).await?;
        txn.set("bar".to_owned(), "foo".to_owned()).await?;
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
        txn.set("foo".to_owned(), "foo".to_owned()).await?;
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
    })
}

const ENV_PD_ADDRS: &str = "PD_ADDRS";

fn pd_addrs() -> Vec<String> {
    env::var(ENV_PD_ADDRS)
        .expect(&format!("Expected {}:", ENV_PD_ADDRS))
        .split(",")
        .map(From::from)
        .collect()
}
