#![cfg(feature = "integration-tests")]
#![allow(clippy::result_large_err)]

mod common;

use std::collections::HashSet;
use std::iter::FromIterator;
use std::time::Duration;

use common::*;
use fail::FailScenario;
use log::info;
use rand::thread_rng;
use rand::Rng;
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

/// Mark a phase boundary in a long-running cleanup test.
///
/// These tests occasionally stall in CI until nextest's 600s cap kills them (#516),
/// and a killed test reports no elapsed timings — so the *entry* marker is the useful
/// one: the last `phase >` line in the log names the step that never finished. The
/// exit marker gives the duration when the test does complete, which is what tells a
/// reader whether a step is merely slow or genuinely stuck.
macro_rules! phase {
    ($name:expr, $body:expr) => {{
        info!("phase > {}", $name);
        let __start = std::time::Instant::now();
        let __out = $body;
        info!("phase < {} in {:?}", $name, __start.elapsed());
        __out
    }};
}

// Lock counting is scoped to each test's own keys, so that residual locks left by an
// earlier serial test — async-commit locks resolve in the background and drain at an
// unpredictable rate — can never inflate another test's count. That unbounded-scan-vs
// -draining-residue race was the source of the flaky lock over-counts (#525).
//
// Rather than filter a whole-keyspace scan, each multi-key test writes inside its own
// disjoint *band* of the u32 key space and scans only that band, so residual locks
// outside the band are never even fetched. Crucially the bands are a large fraction of
// the key space (not a single leading byte): under `MULTI_REGION` the harness splits the
// 4-byte key space into ~40 regions, so a band of `1/NUM_KEY_BANDS` of the space still
// spans several regions and the cross-region cleanup/retry paths stay exercised — a narrow
// single-byte prefix would collapse each test onto (usually) a single region.
const NUM_KEY_BANDS: u32 = 10;
const BAND_BATCH_SIZE: u32 = 0;
const BAND_ASYNC_NO_COMMIT: u32 = 1;
const BAND_ASYNC_PARTIAL: u32 = 2;
const BAND_ASYNC_ALL: u32 = 3;
const BAND_RANGE: u32 = 4;
const BAND_2PC_NO_COMMIT: u32 = 5;
const BAND_2PC_ALL: u32 = 6;

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

    fail::cfg("after-prewrite", "return").unwrap();
    fail::cfg("before-cleanup-locks", "return").unwrap();
    defer! {{
        fail::cfg("after-prewrite", "off").unwrap();
        fail::cfg("before-cleanup-locks", "off").unwrap();
    }}

    let client =
        TransactionClient::new_with_config(pd_addrs(), Config::default().with_default_keyspace())
            .await?;
    let keys = write_data(&client, BAND_BATCH_SIZE, true, true).await?;
    assert_eq!(
        count_locks_in_band(&client, BAND_BATCH_SIZE).await?,
        keys.len()
    );

    let safepoint = client.current_timestamp().await?;
    let options = ResolveLocksOptions {
        async_commit_only: false,
        batch_size: 4,
    };
    // Scope the cleanup to this test's band. `before-cleanup-locks` stubs the actual
    // resolution but still counts every *scanned* lock into `resolved_locks`, so a
    // whole-keyspace range would fold an earlier test's still-draining residue into the
    // count and make `resolved_locks > keys.len()` — the #525 flake.
    let (band_lo, band_hi) = band_range_bytes(BAND_BATCH_SIZE);
    let res = client
        .cleanup_locks(band_lo..band_hi, &safepoint, options)
        .await?;

    assert_eq!(res.resolved_locks, keys.len());
    // The stubbed cleanup left the locks held (this asserts they are). Band isolation
    // already keeps them out of any other test's scan, but resolve them for real before
    // returning anyway, so this test leaves its band clean instead of seeding
    // async-commit locks that drain in the background and add work to the next test's
    // whole-keyspace init cleanup.
    assert_eq!(
        count_locks_in_band(&client, BAND_BATCH_SIZE).await?,
        keys.len()
    );
    fail::cfg("before-cleanup-locks", "off").unwrap();
    let (band_lo, band_hi) = band_range_bytes(BAND_BATCH_SIZE);
    client
        .cleanup_locks(
            band_lo..band_hi,
            &safepoint,
            ResolveLocksOptions {
                async_commit_only: false,
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(count_locks_in_band(&client, BAND_BATCH_SIZE).await?, 0);

    scenario.teardown();
    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_cleanup_async_commit_locks() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();

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
        let keys = write_data(&client, BAND_ASYNC_NO_COMMIT, true, true).await?;
        assert_eq!(
            count_locks_in_band(&client, BAND_ASYNC_NO_COMMIT).await?,
            keys.len()
        );

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: true,
            ..Default::default()
        };
        let (band_lo, band_hi) = band_range_bytes(BAND_ASYNC_NO_COMMIT);
        client
            .cleanup_locks(band_lo..band_hi, &safepoint, options)
            .await?;

        must_committed(&client, keys).await;
        assert_eq!(count_locks_in_band(&client, BAND_ASYNC_NO_COMMIT).await?, 0);
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
        let keys = phase!(
            "async/partial: write_data",
            write_data(&client, BAND_ASYNC_PARTIAL, true, false).await?
        );
        // Wait for async commit to complete.
        let expected = keys.len() * percent / 100;
        let remaining = phase!(
            "async/partial: wait for locks to settle",
            wait_for_locks_count_in_band(&client, BAND_ASYNC_PARTIAL, expected).await?
        );
        assert_eq!(remaining, expected);

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: true,
            ..Default::default()
        };
        let (band_lo, band_hi) = band_range_bytes(BAND_ASYNC_PARTIAL);
        client
            .cleanup_locks(band_lo..band_hi, &safepoint, options)
            .await?;

        must_committed(&client, keys).await;
        assert_eq!(count_locks_in_band(&client, BAND_ASYNC_PARTIAL).await?, 0);
    }

    // all committed
    {
        info!("test all committed");
        let client = TransactionClient::new_with_config(
            pd_addrs(),
            Config::default().with_default_keyspace(),
        )
        .await?;
        let keys = write_data(&client, BAND_ASYNC_ALL, true, false).await?;

        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: true,
            ..Default::default()
        };
        let (band_lo, band_hi) = band_range_bytes(BAND_ASYNC_ALL);
        client
            .cleanup_locks(band_lo..band_hi, &safepoint, options)
            .await?;

        must_committed(&client, keys).await;
        assert_eq!(count_locks_in_band(&client, BAND_ASYNC_ALL).await?, 0);
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
    let keys = write_data(&client, BAND_RANGE, true, true).await?;
    assert_eq!(count_locks_in_band(&client, BAND_RANGE).await?, keys.len());

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
    client
        .cleanup_locks(start_key.clone()..end_key.clone(), &safepoint, options)
        .await?;
    // `cleanup_locks` will resolve primary locks as well. So just check the remaining locks in the range.
    let remaining =
        wait_for_locks_count_in_range(&client, start_key.clone(), end_key.clone(), 0).await?;
    assert_eq!(remaining, 0);

    // cleanup the rest of this test's band so `must_committed` sees every key committed.
    let (band_lo, band_hi) = band_range_bytes(BAND_RANGE);
    let options = ResolveLocksOptions {
        async_commit_only: false,
        ..Default::default()
    };
    client
        .cleanup_locks(band_lo..band_hi, &safepoint, options)
        .await?;
    must_committed(&client, keys).await;
    assert_eq!(count_locks_in_band(&client, BAND_RANGE).await?, 0);

    scenario.teardown();
    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_resolve_locks() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();

    fail::cfg("after-prewrite", "return").unwrap();
    defer! {{
        fail::cfg("after-prewrite", "off").unwrap();
    }}

    let client =
        TransactionClient::new_with_config(pd_addrs(), Config::default().with_default_keyspace())
            .await?;
    let key = b"resolve-locks-key".to_vec();
    let keys = HashSet::from_iter(vec![key.clone()]);
    let mut txn = client
        .begin_with_options(
            TransactionOptions::new_optimistic()
                .heartbeat_option(HeartbeatOption::NoHeartbeat)
                .drop_check(CheckLevel::Warn),
        )
        .await?;
    txn.put(key.clone(), b"value".to_vec()).await?;
    assert!(txn.commit().await.is_err());

    let safepoint = client.current_timestamp().await?;
    // Scan only this test's own key, so a residual lock from an earlier test cannot be
    // swept into `resolve_locks` below.
    let locks = client
        .scan_locks(&safepoint, key.clone()..=key.clone(), 1024)
        .await?;
    assert!(locks.iter().any(|lock| lock.key == key));

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let start_version = client.current_timestamp().await?;
    let live_locks = client
        .resolve_locks(locks, start_version, OPTIMISTIC_BACKOFF)
        .await?;
    assert!(live_locks.is_empty());
    assert_eq!(count_locks_of_key(&client, key.clone()).await?, 0);
    must_rollbacked(&client, keys).await;

    scenario.teardown();
    Ok(())
}

// Regression test for #545: a pessimistic transaction whose commit fails after
// prewrite has placed its 2PC lock must have that lock cleared by `rollback()`.
// Previously the terminal pessimistic rollback sent `PessimisticRollback`, which
// only removes `LockType::Pessimistic` locks, so the prewrite lock was left
// behind (and `rollback()` still returned `Ok`).
#[tokio::test]
#[serial]
async fn txn_pessimistic_rollback_clears_prewrite_locks() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();

    fail::cfg("after-prewrite", "return").unwrap();
    defer! {{
        fail::cfg("after-prewrite", "off").unwrap();
    }}

    let client =
        TransactionClient::new_with_config(pd_addrs(), Config::default().with_default_keyspace())
            .await?;
    let key = b"pessimistic-rollback-prewrite-key".to_vec();

    let mut txn = client
        .begin_with_options(
            TransactionOptions::new_pessimistic()
                .heartbeat_option(HeartbeatOption::NoHeartbeat)
                .drop_check(CheckLevel::Warn),
        )
        .await?;
    txn.get_for_update(key.clone()).await?;
    txn.put(key.clone(), b"value".to_vec()).await?;
    // The commit fails after prewrite has placed the (2PC) lock.
    assert!(txn.commit().await.is_err());
    // rollback() must clear that prewrite lock.
    txn.rollback().await?;
    assert_eq!(count_locks_of_key(&client, key).await?, 0);

    scenario.teardown();
    Ok(())
}

// Regression test for #545 (retry path): `rollback()` may be re-entered from
// `StartedRollback` after a failed first attempt. The "already started
// committing" fact must survive that transition so the retry still uses
// `BatchRollback`; if it were recomputed from the (now `StartedRollback`)
// status, a pessimistic txn would fall back to `PessimisticRollback` and leak
// the prewrite lock again.
#[tokio::test]
#[serial]
async fn txn_pessimistic_rollback_retry_clears_prewrite_locks() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();

    // Commit fails after prewrite places the 2PC lock.
    fail::cfg("after-prewrite", "return").unwrap();
    // The first rollback attempt fails; the retry must still clear the lock.
    fail::cfg("before-rollback", "1*return").unwrap();
    defer! {{
        fail::cfg("after-prewrite", "off").unwrap();
        fail::cfg("before-rollback", "off").unwrap();
    }}

    let client =
        TransactionClient::new_with_config(pd_addrs(), Config::default().with_default_keyspace())
            .await?;
    let key = b"pessimistic-rollback-retry-key".to_vec();

    let mut txn = client
        .begin_with_options(
            TransactionOptions::new_pessimistic()
                .heartbeat_option(HeartbeatOption::NoHeartbeat)
                .drop_check(CheckLevel::Warn),
        )
        .await?;
    txn.get_for_update(key.clone()).await?;
    txn.put(key.clone(), b"value".to_vec()).await?;
    // The commit fails after prewrite has placed the (2PC) lock.
    assert!(txn.commit().await.is_err());
    // First rollback fails at the failpoint; status stays `StartedRollback`.
    assert!(txn.rollback().await.is_err());
    // The retry must persist `prewritten` and use `BatchRollback` to clear the
    // 2PC lock — recomputing it from status here would fall back to
    // `PessimisticRollback` and leave the lock behind.
    txn.rollback().await?;
    assert_eq!(count_locks_of_key(&client, key).await?, 0);

    scenario.teardown();
    Ok(())
}

#[tokio::test]
#[serial]
async fn txn_cleanup_2pc_locks() -> Result<()> {
    init().await?;
    let scenario = FailScenario::setup();

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
        let keys = phase!(
            "2pc/no-commit: write_data",
            write_data(&client, BAND_2PC_NO_COMMIT, false, true).await?
        );
        phase!("2pc/no-commit: count locks", {
            assert_eq!(
                count_locks_in_band(&client, BAND_2PC_NO_COMMIT).await?,
                keys.len()
            );
        });

        let (band_lo, band_hi) = band_range_bytes(BAND_2PC_NO_COMMIT);
        let safepoint = client.current_timestamp().await?;
        {
            let options = ResolveLocksOptions {
                async_commit_only: true, // Skip 2pc locks.
                ..Default::default()
            };
            phase!("2pc/no-commit: cleanup_locks(async_commit_only)", {
                client
                    .cleanup_locks(band_lo.clone()..band_hi.clone(), &safepoint, options)
                    .await?;
            });
            assert_eq!(
                count_locks_in_band(&client, BAND_2PC_NO_COMMIT).await?,
                keys.len()
            );
        }
        let options = ResolveLocksOptions {
            async_commit_only: false,
            ..Default::default()
        };
        phase!("2pc/no-commit: cleanup_locks(all)", {
            client
                .cleanup_locks(band_lo..band_hi, &safepoint, options)
                .await?;
        });

        phase!(
            "2pc/no-commit: must_rollbacked",
            must_rollbacked(&client, keys).await
        );
        assert_eq!(count_locks_in_band(&client, BAND_2PC_NO_COMMIT).await?, 0);
    }

    // all committed
    {
        info!("test all committed");
        let client = TransactionClient::new_with_config(
            pd_addrs(),
            Config::default().with_default_keyspace(),
        )
        .await?;
        let keys = phase!(
            "2pc/all-committed: write_data",
            write_data(&client, BAND_2PC_ALL, false, false).await?
        );
        phase!("2pc/all-committed: wait for locks to drain", {
            assert_eq!(
                wait_for_locks_count_in_band(&client, BAND_2PC_ALL, 0).await?,
                0
            );
        });

        let (band_lo, band_hi) = band_range_bytes(BAND_2PC_ALL);
        let safepoint = client.current_timestamp().await?;
        let options = ResolveLocksOptions {
            async_commit_only: false,
            ..Default::default()
        };
        phase!("2pc/all-committed: cleanup_locks(all)", {
            client
                .cleanup_locks(band_lo..band_hi, &safepoint, options)
                .await?;
        });

        phase!(
            "2pc/all-committed: must_committed",
            must_committed(&client, keys).await
        );
        assert_eq!(count_locks_in_band(&client, BAND_2PC_ALL).await?, 0);
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

/// Count the de-duplicated locks whose key falls in `[start_key, end_key)`.
///
/// The scan is bounded to the range on the server, so locks outside it — for example
/// async-commit residue still draining from an earlier serial test — are never fetched
/// and cannot inflate the count.
async fn count_locks_in_range(
    client: &TransactionClient,
    start_key: Vec<u8>,
    end_key: Vec<u8>,
) -> Result<usize> {
    let ts = client.current_timestamp().await.unwrap();
    let locks = client.scan_locks(&ts, start_key..end_key, 65536).await?;
    // De-duplicated as `scan_locks` will return duplicated locks due to retry on region changes.
    let locks_set: HashSet<Vec<u8>> = locks.into_iter().map(|l| l.key).collect();
    Ok(locks_set.len())
}

/// Poll until exactly `expected` locks remain in `[start_key, end_key)`, giving up after
/// ~15s and returning whatever the final scan observes.
async fn wait_for_locks_count_in_range(
    client: &TransactionClient,
    start_key: Vec<u8>,
    end_key: Vec<u8>,
    expected: usize,
) -> Result<usize> {
    for _ in 0..30 {
        let remaining = count_locks_in_range(client, start_key.clone(), end_key.clone()).await?;
        if remaining == expected {
            return Ok(expected);
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    count_locks_in_range(client, start_key, end_key).await
}

/// The half-open `[lo, hi)` u32 range owned by `band`. The last band absorbs the remainder
/// of the (integer) division so the bands together tile the whole key space.
fn band_bounds(band: u32) -> (u32, u32) {
    let span = u32::MAX / NUM_KEY_BANDS;
    let lo = band * span;
    let hi = if band == NUM_KEY_BANDS - 1 {
        u32::MAX
    } else {
        lo + span
    };
    (lo, hi)
}

/// `band_bounds` as big-endian key bytes, ready to hand to a range scan or `cleanup_locks`.
fn band_range_bytes(band: u32) -> (Vec<u8>, Vec<u8>) {
    let (lo, hi) = band_bounds(band);
    (lo.to_be_bytes().to_vec(), hi.to_be_bytes().to_vec())
}

/// Count the de-duplicated locks held within a test's own key band.
async fn count_locks_in_band(client: &TransactionClient, band: u32) -> Result<usize> {
    let (lo, hi) = band_range_bytes(band);
    count_locks_in_range(client, lo, hi).await
}

/// Poll until exactly `expected` locks remain within a test's own key band.
async fn wait_for_locks_count_in_band(
    client: &TransactionClient,
    band: u32,
    expected: usize,
) -> Result<usize> {
    let (lo, hi) = band_range_bytes(band);
    wait_for_locks_count_in_range(client, lo, hi, expected).await
}

/// Count whether a single `key` currently holds a lock (0 or 1), scanning only that key.
///
/// Used by the single-key tests; residual locks on any other key are never fetched.
async fn count_locks_of_key(client: &TransactionClient, key: Vec<u8>) -> Result<usize> {
    let ts = client.current_timestamp().await.unwrap();
    let locks = client.scan_locks(&ts, key.clone()..=key, 1024).await?;
    let locks_set: HashSet<Vec<u8>> = locks.into_iter().map(|l| l.key).collect();
    Ok(locks_set.len())
}

// Note: too many transactions or keys will make CI unstable due to timeout.
const TXN_COUNT: usize = 16;
const KEY_COUNT: usize = 32;
const REGION_BACKOFF: Backoff = Backoff::no_jitter_backoff(2, 5000, 20);
const OPTIMISTIC_BACKOFF: Backoff = Backoff::no_jitter_backoff(2, 500, 10);

async fn write_data(
    client: &Client,
    band: u32,
    async_commit: bool,
    commit_error: bool,
) -> Result<HashSet<Vec<u8>>> {
    let mut rng = thread_rng();
    // Generate keys inside this case's band so its locks stay within `[lo, hi)` and never
    // collide with another (serial) case's keys, while remaining spread across the band's
    // regions under `MULTI_REGION`.
    let (lo, hi) = band_bounds(band);
    let mut keys: HashSet<Vec<u8>> = HashSet::new();
    while keys.len() < TXN_COUNT * KEY_COUNT {
        keys.insert(rng.gen_range(lo..hi).to_be_bytes().to_vec());
    }
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
