#![cfg(feature = "integration-tests")]

mod common;
use common::{clear_tikv, pd_addrs};
use serial_test::serial;
use tikv_client::{Result, TransactionClient, TransactionOptions};

#[tokio::test]
#[serial]
async fn optimistic_heartbeat() -> Result<()> {
    clear_tikv().await;

    let key1 = "key1".to_owned();
    let key2 = "key2".to_owned();
    let client = TransactionClient::new(pd_addrs()).await?;

    let mut heartbeat_txn = client
        .begin_with_options(TransactionOptions::new_optimistic())
        .await?;
    heartbeat_txn.put(key1, "foo").await.unwrap();

    let mut txn_without_heartbeat = client
        .begin_with_options(TransactionOptions::new_optimistic().no_heart_beat())
        .await?;
    txn_without_heartbeat.put(key2, "fooo").await.unwrap();

    fail::cfg("after-prewrite", "sleep(30000)").unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;

    // use other txns to check these locks
    let mut t3 = client
        .begin_with_options(TransactionOptions::new_optimistic().no_resolve_locks().no_heart_beat())
        .await?;
    t3.put("key1".to_owned(), "gee").await?;
    assert!(t3.commit().await.is_err());
    let mut t4 = client.begin_optimistic().await?;
    t4.put("key2".to_owned(), "geee").await?;
    t4.commit().await?;

    assert!(heartbeat_txn.commit().await.is_ok());
    assert!(txn_without_heartbeat.commit().await.is_err());

    Ok(())
}
