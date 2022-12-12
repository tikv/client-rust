#![cfg(feature = "integration-tests")]

mod common;

use common::*;
use fail::FailScenario;
use rand::thread_rng;
use serial_test::serial;
use slog::info;
use std::{collections::HashSet, time::Duration};
use tikv_client::{
    transaction::{Client, HeartbeatOption, ResolveLocksOptions},
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

async fn must_committed(client: &TransactionClient, keys: HashSet<Vec<u8>>) {
    let ts = client.current_timestamp().await.unwrap();
    let mut snapshot = client.snapshot(ts, TransactionOptions::default());
    for key in keys {
        let val = snapshot.get(key.clone()).await.unwrap();
        assert_eq!(Some(key), val);
    }
}

async fn must_rollbacked(client: &TransactionClient, keys: HashSet<Vec<u8>>) {
    let ts = client.current_timestamp().await.unwrap();
    let mut snapshot = client.snapshot(ts, TransactionOptions::default());
    for key in keys {
        let val = snapshot.get(key.clone()).await.unwrap();
        assert_eq!(None, val);
    }
}

async fn must_have_locks(client: &TransactionClient, count: usize) {
    let ts = client.current_timestamp().await.unwrap();
    let locks = client.scan_locks(&ts, vec![]).await.unwrap();
    assert_eq!(locks.len(), count);
}

async fn must_no_lock(client: &TransactionClient) {
    (must_have_locks(client, 0)).await;
}

const TXN_COUNT: usize = 16;
const KEY_COUNT: usize = 64;

async fn write_data(
    client: &Client,
    async_commit: bool,
    commit_error: bool,
) -> Result<HashSet<Vec<u8>>> {
    let mut rng = thread_rng();
    let keys = gen_u32_keys((TXN_COUNT * KEY_COUNT) as u32, &mut rng);
    let mut txns = Vec::with_capacity(TXN_COUNT);

    let mut options =
        TransactionOptions::new_optimistic().drop_check(tikv_client::CheckLevel::Warn);
    if async_commit {
        options = options.use_async_commit();
    }

    for _ in 0..TXN_COUNT {
        let txn = client.begin_with_options(options.clone()).await?;
        txns.push(txn);
    }

    for (i, key) in keys.iter().enumerate() {
        txns[i % TXN_COUNT]
            .put(key.to_owned(), key.to_owned())
            .await?;
    }

    for txn in &mut txns {
        let res = txn.commit().await;
        assert_eq!(res.is_err(), commit_error, "error: {:?}", res);
    }
    Ok(keys)
}

#[tokio::test]
#[serial]
async fn txn_cleanup_async_commit_locks() -> Result<()> {
    let logger = new_logger(slog::Level::Info);

    init().await?;
    let scenario = FailScenario::setup();

    // no commit
    {
        info!(logger, "test no commit");
        fail::cfg("after-prewrite", "return").unwrap();
        defer! {
            fail::cfg("after-prewrite", "off").unwrap()
        }

        let client = TransactionClient::new(pd_addrs(), Some(logger.clone())).await?;
        let keys = write_data(&client, true, true).await?;
        must_have_locks(&client, keys.len()).await;

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: true,
            ..Default::default()
        };
        client.cleanup_locks(&safepoint, options).await?;

        must_committed(&client, keys).await;
        must_no_lock(&client).await;
    }

    // partial commit
    {
        info!(logger, "test partial commit");
        let percent = 50;
        fail::cfg("before-commit-secondary", &format!("return({})", percent)).unwrap();
        defer! {
            fail::cfg("before-commit-secondary", "off").unwrap()
        }

        let client = TransactionClient::new(pd_addrs(), Some(logger.clone())).await?;
        let keys = write_data(&client, true, false).await?;
        must_have_locks(&client, keys.len() * percent / 100).await;

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: true,
            ..Default::default()
        };
        client.cleanup_locks(&safepoint, options).await?;

        must_committed(&client, keys).await;
        must_no_lock(&client).await;
    }

    // all committed
    {
        info!(logger, "test all committed");
        let client = TransactionClient::new(pd_addrs(), Some(logger.clone())).await?;
        let keys = write_data(&client, true, false).await?;
        must_no_lock(&client).await;

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: true,
            ..Default::default()
        };
        client.cleanup_locks(&safepoint, options).await?;

        must_committed(&client, keys).await;
        must_no_lock(&client).await;
    }

    scenario.teardown();
    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_cleanup_2pc_locks() -> Result<()> {
    let logger = new_logger(slog::Level::Info);

    init().await?;
    let scenario = FailScenario::setup();

    // no commit
    {
        info!(logger, "test no commit");
        fail::cfg("after-prewrite", "return").unwrap();
        defer! {
            fail::cfg("after-prewrite", "off").unwrap()
        }

        let client = TransactionClient::new(pd_addrs(), Some(logger.clone())).await?;
        let keys = write_data(&client, false, true).await?;
        must_have_locks(&client, keys.len()).await;

        let safepoint = client.current_timestamp().await?;
        {
            let options = ResolveLocksOptions {
                async_commit_only: true, // Skip 2pc locks.
                ..Default::default()
            };
            client.cleanup_locks(&safepoint, options).await?;
            must_have_locks(&client, keys.len()).await;
        }
        let options = ResolveLocksOptions {
            async_commit_only: false,
            ..Default::default()
        };
        client.cleanup_locks(&safepoint, options).await?;

        must_rollbacked(&client, keys).await;
        must_no_lock(&client).await;
    }

    // all committed
    {
        info!(logger, "test all committed");
        let client = TransactionClient::new(pd_addrs(), Some(logger.clone())).await?;
        let keys = write_data(&client, false, false).await?;
        must_no_lock(&client).await;

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: false,
            ..Default::default()
        };
        client.cleanup_locks(&safepoint, options).await?;

        must_committed(&client, keys).await;
        must_no_lock(&client).await;
    }

    scenario.teardown();
    Ok(())
}
