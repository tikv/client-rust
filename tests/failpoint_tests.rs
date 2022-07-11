#![cfg(feature = "integration-tests")]

mod common;
use common::{init, pd_addrs};
use fail::FailScenario;
use serial_test::serial;
use std::time::Duration;
use tikv_client::{
    transaction::{ApiV1, HeartbeatOption},
    Result, TransactionClient, TransactionOptions,
};

#[tokio::test]
#[serial]
async fn txn_optimistic_heartbeat() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();
    fail::cfg("after-prewrite", "sleep(6000)").unwrap();

    let key1 = "key1".to_owned();
    let key2 = "key2".to_owned();
    let client = TransactionClient::new(pd_addrs(), ApiV1::default(), None).await?;

    let mut heartbeat_txn = client
        .begin_with_options(
            TransactionOptions::new_optimistic()
                .heartbeat_option(HeartbeatOption::FixedTime(Duration::from_secs(1))),
        )
        .await?;
    heartbeat_txn.put(key1.clone(), "foo").await.unwrap();

    let mut txn_without_heartbeat = client
        .begin_with_options(
            TransactionOptions::new_optimistic().heartbeat_option(HeartbeatOption::NoHeartbeat),
        )
        .await?;
    txn_without_heartbeat
        .put(key2.clone(), "fooo")
        .await
        .unwrap();

    let heartbeat_txn_handle = tokio::task::spawn_blocking(move || {
        assert!(futures::executor::block_on(heartbeat_txn.commit()).is_ok())
    });
    let txn_without_heartbeat_handle = tokio::task::spawn_blocking(move || {
        assert!(futures::executor::block_on(txn_without_heartbeat.commit()).is_err())
    });

    // inital TTL is 3 seconds, before which TTL is valid regardless of heartbeat.
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    fail::cfg("after-prewrite", "off").unwrap();

    // use other txns to check these locks
    let mut t3 = client
        .begin_with_options(
            TransactionOptions::new_optimistic()
                .no_resolve_locks()
                .heartbeat_option(HeartbeatOption::NoHeartbeat),
        )
        .await?;
    t3.put(key1.clone(), "gee").await?;
    assert!(t3.commit().await.is_err());

    let mut t4 = client.begin_optimistic().await?;
    t4.put(key2.clone(), "geee").await?;
    t4.commit().await?;

    heartbeat_txn_handle.await.unwrap();
    txn_without_heartbeat_handle.await.unwrap();

    scenario.teardown();

    Ok(())
}
