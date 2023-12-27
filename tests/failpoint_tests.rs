#![cfg(feature = "integration-tests")]

mod common;

use std::collections::HashSet;
use std::iter::FromIterator;
use std::thread;
use std::time::Duration;

use common::*;
use fail::FailScenario;
use log::info;
use rand::thread_rng;
use serial_test::serial;
use tikv_client::transaction::Client;
use tikv_client::transaction::HeartbeatOption;
use tikv_client::transaction::ResolveLocksOptions;
use tikv_client::Backoff;
use tikv_client::CheckLevel;
use tikv_client::Config;
use tikv_client::Result;
use tikv_client::RetryOptions;
use tikv_client::TransactionClient;
use tikv_client::TransactionOptions;

#[tokio::test]
#[serial]
async fn txn_optimistic_heartbeat() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();
    fail::cfg("after-prewrite", "sleep(6000)").unwrap();
    defer! {{
        fail::cfg("after-prewrite", "off").unwrap();
    }}

    let key1 = "key1".to_owned();
    let key2 = "key2".to_owned();
    let client =
        TransactionClient::new_with_config(pd_addrs(), Config::default().with_default_keyspace())
            .await?;

    // CheckLevel::Panic makes the case unstable, change to Warn level for now.
    // See https://github.com/tikv/client-rust/issues/389
    let mut heartbeat_txn = client
        .begin_with_options(
            TransactionOptions::new_optimistic()
                .heartbeat_option(HeartbeatOption::FixedTime(Duration::from_secs(1)))
                .drop_check(CheckLevel::Warn),
        )
        .await?;
    heartbeat_txn.put(key1.clone(), "foo").await.unwrap();

    let mut txn_without_heartbeat = client
        .begin_with_options(
            TransactionOptions::new_optimistic()
                .heartbeat_option(HeartbeatOption::NoHeartbeat)
                .drop_check(CheckLevel::Warn),
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
                .heartbeat_option(HeartbeatOption::NoHeartbeat)
                .drop_check(CheckLevel::Warn),
        )
        .await?;
    t3.put(key1.clone(), "gee").await?;
    assert!(t3.commit().await.is_err());

    let mut t4 = client
        .begin_with_options(TransactionOptions::new_optimistic().drop_check(CheckLevel::Warn))
        .await?;
    t4.put(key2.clone(), "geee").await?;
    t4.commit().await?;

    heartbeat_txn_handle.await.unwrap();
    txn_without_heartbeat_handle.await.unwrap();

    scenario.teardown();

    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_cleanup_locks_batch_size() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();
    let full_range = ..;

    fail::cfg("after-prewrite", "return").unwrap();
    fail::cfg("before-cleanup-locks", "return").unwrap();
    defer! {{
        fail::cfg("after-prewrite", "off").unwrap();
        fail::cfg("before-cleanup-locks", "off").unwrap();
    }}

    let client =
        TransactionClient::new_with_config(pd_addrs(), Config::default().with_default_keyspace())
            .await?;
    let keys = write_data(&client, true, true).await?;
    assert_eq!(count_locks(&client).await?, keys.len());

    let safepoint = client.current_timestamp().await?;
    let options = ResolveLocksOptions {
        async_commit_only: false,
        batch_size: 4,
    };
    let res = client
        .cleanup_locks(full_range, &safepoint, options)
        .await?;

    assert_eq!(res.resolved_locks, keys.len());
    assert_eq!(count_locks(&client).await?, keys.len());

    scenario.teardown();
    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_cleanup_async_commit_locks() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();
    let full_range = ..;

    // no commit
    {
        info!("test no commit");
        fail::cfg("after-prewrite", "return").unwrap();
        defer! {
            fail::cfg("after-prewrite", "off").unwrap()
        }

        let client = TransactionClient::new_with_config(
            pd_addrs(),
            Config::default().with_default_keyspace(),
        )
        .await?;
        let keys = write_data(&client, true, true).await?;
        assert_eq!(count_locks(&client).await?, keys.len());

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: true,
            ..Default::default()
        };
        client
            .cleanup_locks(full_range, &safepoint, options)
            .await?;

        must_committed(&client, keys).await;
        assert_eq!(count_locks(&client).await?, 0);
    }

    // partial commit
    {
        info!("test partial commit");
        let percent = 50;
        fail::cfg("before-commit-secondary", &format!("return({percent})")).unwrap();
        defer! {
            fail::cfg("before-commit-secondary", "off").unwrap()
        }

        let client = TransactionClient::new_with_config(
            pd_addrs(),
            Config::default().with_default_keyspace(),
        )
        .await?;
        let keys = write_data(&client, true, false).await?;
        thread::sleep(Duration::from_secs(1)); // Wait for async commit to complete.
        assert_eq!(count_locks(&client).await?, keys.len() * percent / 100);

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: true,
            ..Default::default()
        };
        client
            .cleanup_locks(full_range, &safepoint, options)
            .await?;

        must_committed(&client, keys).await;
        assert_eq!(count_locks(&client).await?, 0);
    }

    // all committed
    {
        info!("test all committed");
        let client = TransactionClient::new_with_config(
            pd_addrs(),
            Config::default().with_default_keyspace(),
        )
        .await?;
        let keys = write_data(&client, true, false).await?;

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: true,
            ..Default::default()
        };
        client
            .cleanup_locks(full_range, &safepoint, options)
            .await?;

        must_committed(&client, keys).await;
        assert_eq!(count_locks(&client).await?, 0);
    }

    // TODO: test rollback

    // TODO: test region error

    scenario.teardown();
    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_cleanup_range_async_commit_locks() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();
    info!("test range clean lock");
    fail::cfg("after-prewrite", "return").unwrap();
    defer! {
        fail::cfg("after-prewrite", "off").unwrap()
    }

    let client =
        TransactionClient::new_with_config(pd_addrs(), Config::default().with_default_keyspace())
            .await?;
    let keys = write_data(&client, true, true).await?;
    assert_eq!(count_locks(&client).await?, keys.len());

    info!("total keys' count {}", keys.len());
    let mut sorted_keys: Vec<Vec<u8>> = Vec::from_iter(keys.clone());
    sorted_keys.sort();
    let start_key = sorted_keys[1].clone();
    let end_key = sorted_keys[sorted_keys.len() - 2].clone();

    let safepoint = client.current_timestamp().await?;
    let options = ResolveLocksOptions {
        async_commit_only: true,
        ..Default::default()
    };
    let res = client
        .cleanup_locks(start_key..end_key, &safepoint, options)
        .await?;

    assert_eq!(res.resolved_locks, keys.len() - 3);

    // cleanup all locks to avoid affecting following cases.
    let options = ResolveLocksOptions {
        async_commit_only: false,
        ..Default::default()
    };
    client.cleanup_locks(.., &safepoint, options).await?;
    must_committed(&client, keys).await;
    assert_eq!(count_locks(&client).await?, 0);

    scenario.teardown();
    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_cleanup_2pc_locks() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();
    let full_range = ..;

    // no commit
    {
        info!("test no commit");
        fail::cfg("after-prewrite", "return").unwrap();
        defer! {
            fail::cfg("after-prewrite", "off").unwrap()
        }

        let client = TransactionClient::new_with_config(
            pd_addrs(),
            Config::default().with_default_keyspace(),
        )
        .await?;
        let keys = write_data(&client, false, true).await?;
        assert_eq!(count_locks(&client).await?, keys.len());

        let safepoint = client.current_timestamp().await?;
        {
            let options = ResolveLocksOptions {
                async_commit_only: true, // Skip 2pc locks.
                ..Default::default()
            };
            client
                .cleanup_locks(full_range, &safepoint, options)
                .await?;
            assert_eq!(count_locks(&client).await?, keys.len());
        }
        let options = ResolveLocksOptions {
            async_commit_only: false,
            ..Default::default()
        };
        client
            .cleanup_locks(full_range, &safepoint, options)
            .await?;

        must_rollbacked(&client, keys).await;
        assert_eq!(count_locks(&client).await?, 0);
    }

    // all committed
    {
        info!("test all committed");
        let client = TransactionClient::new_with_config(
            pd_addrs(),
            Config::default().with_default_keyspace(),
        )
        .await?;
        let keys = write_data(&client, false, false).await?;
        assert_eq!(count_locks(&client).await?, 0);

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: false,
            ..Default::default()
        };
        client
            .cleanup_locks(full_range, &safepoint, options)
            .await?;

        must_committed(&client, keys).await;
        assert_eq!(count_locks(&client).await?, 0);
    }

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

async fn count_locks(client: &TransactionClient) -> Result<usize> {
    let ts = client.current_timestamp().await.unwrap();
    let locks = client.scan_locks(&ts, .., 1024).await?;
    // De-duplicated as `scan_locks` will return duplicated locks due to retry on region changes.
    let locks_set: HashSet<Vec<u8>> = HashSet::from_iter(locks.into_iter().map(|l| l.key));
    Ok(locks_set.len())
}

// Note: too many transactions or keys will make CI unstable due to timeout.
const TXN_COUNT: usize = 16;
const KEY_COUNT: usize = 32;
const REGION_BACKOFF: Backoff = Backoff::no_jitter_backoff(2, 5000, 20);
const OPTIMISTIC_BACKOFF: Backoff = Backoff::no_jitter_backoff(2, 500, 10);

async fn write_data(
    client: &Client,
    async_commit: bool,
    commit_error: bool,
) -> Result<HashSet<Vec<u8>>> {
    let mut rng = thread_rng();
    let keys = gen_u32_keys((TXN_COUNT * KEY_COUNT) as u32, &mut rng);
    let mut txns = Vec::with_capacity(TXN_COUNT);

    let mut options = TransactionOptions::new_optimistic()
        .retry_options(RetryOptions {
            region_backoff: REGION_BACKOFF,
            lock_backoff: OPTIMISTIC_BACKOFF,
        })
        .drop_check(CheckLevel::Warn);
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
        assert_eq!(res.is_err(), commit_error, "error: {res:?}");
    }
    Ok(keys)
}
