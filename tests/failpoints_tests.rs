#![cfg(feature = "integration-tests")]

mod common;
use common::{clear_tikv, pd_addrs};
use serial_test::serial;
use tikv_client::{Result, TransactionClient, TransactionOptions};

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[serial]
async fn optimistic_heartbeat() -> Result<()> {
    clear_tikv().await;
    fail::cfg("after-prewrite", "sleep(10000)").unwrap();

    let key1 = "key1".to_owned();
    let key2 = "key2".to_owned();
    let client = TransactionClient::new(pd_addrs()).await?;

    let mut heartbeat_txn = client
        .begin_with_options(TransactionOptions::new_optimistic())
        .await?;
    heartbeat_txn.put(key1.clone(), "foo").await.unwrap();

    let mut txn_without_heartbeat = client
        .begin_with_options(TransactionOptions::new_optimistic().no_auto_hearbeat())
        .await?;
    txn_without_heartbeat
        .put(key2.clone(), "fooo")
        .await
        .unwrap();

    let heartbeat_txn_handle = tokio::spawn(async move {
        assert!(heartbeat_txn.commit().await.is_ok());
    });
    let txn_without_heartbeat_handle = tokio::spawn(async move {
        assert!(txn_without_heartbeat.commit().await.is_err());
    });

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    fail::cfg("after-prewrite", "off").unwrap();

    // use other txns to check these locks
    let mut t3 = client
        .begin_with_options(
            TransactionOptions::new_optimistic()
                .no_resolve_locks()
                .no_auto_hearbeat(),
        )
        .await?;
    t3.put(key1.clone(), "gee").await?;
    assert!(t3.commit().await.is_err());

    let mut t4 = client.begin_optimistic().await?;
    t4.put(key2.clone(), "geee").await?;
    t4.commit().await?;

    heartbeat_txn_handle.await.unwrap();
    txn_without_heartbeat_handle.await.unwrap();

    Ok(())
}
