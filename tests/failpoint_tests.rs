#![cfg(feature = "integration-tests")]

mod common;
use common::{init, pd_addrs};
use fail::FailScenario;
use serial_test::serial;
use slog::{Drain, Logger};
use std::time::Duration;
use tikv_client::{
    transaction::HeartbeatOption, Result, TimestampExt, TransactionClient, TransactionOptions,
};
use tikv_client_proto::pdpb::Timestamp;
use tokio::task::JoinHandle;

#[tokio::test]
#[serial]
async fn txn_optimistic_heartbeat() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();
    fail::cfg("after-prewrite", "sleep(6000)").unwrap();

    let key1 = "key1".to_owned();
    let key2 = "key2".to_owned();
    let client = TransactionClient::new(pd_addrs(), None).await?;

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

#[tokio::test]
#[serial]
async fn txn_status() -> Result<()> {
    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
    let logger = Logger::root(
        slog_term::FullFormat::new(plain)
            .build()
            .filter_level(slog::Level::Debug)
            .fuse(),
        slog::o!(),
    );

    init().await?;
    let scenario = FailScenario::setup();
    fail::cfg("after-prewrite", "sleep(6000)").unwrap();

    let key1 = b"key1".to_vec();
    let val1 = b"val1".to_vec();
    let key2 = b"key2".to_vec();
    let val2 = b"val2".to_vec();
    let client = TransactionClient::new(pd_addrs(), Some(logger)).await?;
    let mut txn1 = client
        .begin_with_options(
            TransactionOptions::new_optimistic()
                .use_async_commit()
                .drop_check(tikv_client::CheckLevel::Warn),
        )
        .await?;

    txn1.put(key1.clone(), val1.clone()).await?;
    txn1.put(key2.clone(), val2.clone()).await?;
    // let txn1_handle: JoinHandle<Result<Option<Timestamp>>> =
    //     tokio::task::spawn(async move { txn1.commit().await });
    let txn1_handle: JoinHandle<Result<Option<Timestamp>>> =
        tokio::task::spawn_blocking(move || futures::executor::block_on(txn1.commit()));

    tokio::time::sleep(Duration::from_secs(1)).await;
    let safepoint = client.current_timestamp().await?;
    client.cleanup_async_commit_locks(safepoint).await?;

    fail::cfg("after-prewrite", "off").unwrap();
    let commit_ts = txn1_handle.await?.unwrap().unwrap();
    println!(
        "commit_ts:{}, physical:{}",
        commit_ts.version(),
        commit_ts.get_physical()
    );

    // check
    {
        let mut txn = client.begin_optimistic().await?;
        assert_eq!(txn.get(key1).await?.unwrap(), val1);
        assert_eq!(txn.get(key2).await?.unwrap(), val2);
        txn.commit().await?;
    }

    scenario.teardown();
    Ok(())
}
