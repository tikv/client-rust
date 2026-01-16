#![cfg(feature = "integration-tests")]

//! Tests for SyncTransactionClient
//!
//! These tests mirror the async TransactionClient tests but use the synchronous API.

mod common;
use common::*;
use serial_test::serial;
use std::collections::HashMap;
use tikv_client::Config;
use tikv_client::Key;
use tikv_client::Result;
use tikv_client::SyncTransactionClient;
use tikv_client::TransactionOptions;
use tikv_client::Value;

/// Helper to initialize and return a sync client
fn sync_client() -> Result<SyncTransactionClient> {
    SyncTransactionClient::new_with_config(pd_addrs(), Config::default().with_default_keyspace())
}

#[test]
#[serial]
fn sync_txn_get_timestamp() -> Result<()> {
    const COUNT: usize = 1 << 8;
    let client = sync_client()?;

    let mut versions = (0..COUNT)
        .map(|_| client.current_timestamp())
        .map(|res| res.map(|ts| (ts.physical << 18) + ts.logical))
        .collect::<Result<Vec<_>>>()?;

    // Each version should be unique
    versions.sort_unstable();
    versions.dedup();
    assert_eq!(versions.len(), COUNT);
    Ok(())
}

#[test]
#[serial]
fn sync_txn_crud() -> Result<()> {
    init_sync()?;

    let client = sync_client()?;
    let mut txn = client.begin_optimistic()?;

    // Get non-existent keys
    assert!(txn.get("foo".to_owned())?.is_none());

    // batch_get do not return non-existent entries
    assert_eq!(
        txn.batch_get(vec!["foo".to_owned(), "bar".to_owned()])?
            .count(),
        0
    );

    txn.put("foo".to_owned(), "bar".to_owned())?;
    txn.put("bar".to_owned(), "foo".to_owned())?;

    // Read buffered values
    assert_eq!(txn.get("foo".to_owned())?, Some("bar".to_owned().into()));

    let batch_get_res: HashMap<Key, Value> = txn
        .batch_get(vec!["foo".to_owned(), "bar".to_owned()])?
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

    txn.commit()?;

    // Verify the values were committed
    let mut txn = client.begin_optimistic()?;
    assert_eq!(txn.get("foo".to_owned())?, Some("bar".to_owned().into()));

    // Test delete
    txn.delete("foo".to_owned())?;
    assert!(txn.get("foo".to_owned())?.is_none());
    txn.commit()?;

    // Verify deletion
    let mut txn = client.begin_optimistic()?;
    assert!(txn.get("foo".to_owned())?.is_none());
    txn.rollback()?;

    Ok(())
}

#[test]
#[serial]
fn sync_txn_begin_pessimistic() -> Result<()> {
    init_sync()?;

    let client = sync_client()?;
    let mut txn = client.begin_pessimistic()?;

    txn.put("pessimistic_key".to_owned(), "value".to_owned())?;
    assert_eq!(
        txn.get("pessimistic_key".to_owned())?,
        Some("value".to_owned().into())
    );

    txn.commit()?;

    // Verify committed
    let mut txn = client.begin_optimistic()?;
    assert_eq!(
        txn.get("pessimistic_key".to_owned())?,
        Some("value".to_owned().into())
    );
    txn.rollback()?;

    Ok(())
}

#[test]
#[serial]
fn sync_txn_snapshot() -> Result<()> {
    init_sync()?;

    let client = sync_client()?;

    // Write some data
    let mut txn = client.begin_optimistic()?;
    txn.put("snapshot_key".to_owned(), "initial".to_owned())?;
    txn.commit()?;

    // Get snapshot at current timestamp
    let ts = client.current_timestamp()?;
    let mut snapshot = client.snapshot(ts, TransactionOptions::new_optimistic());

    // Verify snapshot reads initial value
    assert_eq!(
        snapshot.get("snapshot_key".to_owned())?,
        Some("initial".to_owned().into())
    );

    // Modify the value
    let mut txn = client.begin_optimistic()?;
    txn.put("snapshot_key".to_owned(), "updated".to_owned())?;
    txn.commit()?;

    // Snapshot still reads old value
    assert_eq!(
        snapshot.get("snapshot_key".to_owned())?,
        Some("initial".to_owned().into())
    );

    // New transaction reads updated value
    let mut txn = client.begin_optimistic()?;
    assert_eq!(
        txn.get("snapshot_key".to_owned())?,
        Some("updated".to_owned().into())
    );
    txn.rollback()?;

    Ok(())
}

#[test]
#[serial]
fn sync_txn_begin_with_options() -> Result<()> {
    init_sync()?;

    let client = sync_client()?;
    let options = TransactionOptions::new_optimistic();
    let mut txn = client.begin_with_options(options)?;

    txn.put("options_key".to_owned(), "value".to_owned())?;
    txn.commit()?;

    // Verify
    let mut txn = client.begin_optimistic()?;
    assert_eq!(
        txn.get("options_key".to_owned())?,
        Some("value".to_owned().into())
    );
    txn.rollback()?;

    Ok(())
}

#[test]
#[serial]
fn sync_txn_rollback() -> Result<()> {
    init_sync()?;

    let client = sync_client()?;

    // Write initial value
    let mut txn = client.begin_optimistic()?;
    txn.put("rollback_key".to_owned(), "initial".to_owned())?;
    txn.commit()?;

    // Start new transaction and modify
    let mut txn = client.begin_optimistic()?;
    txn.put("rollback_key".to_owned(), "modified".to_owned())?;

    // Rollback instead of commit
    txn.rollback()?;

    // Verify value is still "initial"
    let mut txn = client.begin_optimistic()?;
    assert_eq!(
        txn.get("rollback_key".to_owned())?,
        Some("initial".to_owned().into())
    );
    txn.rollback()?;

    Ok(())
}

#[test]
#[serial]
fn sync_txn_clone_client() -> Result<()> {
    init_sync()?;

    let client1 = sync_client()?;
    let client2 = client1.clone();

    // Both clients should work independently
    let mut txn1 = client1.begin_optimistic()?;
    let mut txn2 = client2.begin_optimistic()?;

    txn1.put("clone_key1".to_owned(), "value1".to_owned())?;
    txn2.put("clone_key2".to_owned(), "value2".to_owned())?;

    txn1.commit()?;
    txn2.commit()?;

    // Verify both writes succeeded
    let mut txn = client1.begin_optimistic()?;
    assert_eq!(
        txn.get("clone_key1".to_owned())?,
        Some("value1".to_owned().into())
    );
    assert_eq!(
        txn.get("clone_key2".to_owned())?,
        Some("value2".to_owned().into())
    );
    txn.rollback()?;

    Ok(())
}
