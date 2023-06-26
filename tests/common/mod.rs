#![allow(dead_code)]

mod ctl;

use futures_timer::Delay;
use log::{info, warn};
use rand::Rng;
use slog::Drain;
use std::{collections::HashSet, convert::TryInto, env, time::Duration};
use tikv_client::{ColumnFamily, Key, RawClient, Result, Transaction, TransactionClient};

const ENV_PD_ADDRS: &str = "PD_ADDRS";
const ENV_ENABLE_MULIT_REGION: &str = "MULTI_REGION";
const REGION_SPLIT_TIME_LIMIT: Duration = Duration::from_secs(15);

// Delete all entries in TiKV to leave a clean space for following tests.
pub async fn clear_tikv() {
    let cfs = vec![
        ColumnFamily::Default,
        ColumnFamily::Lock,
        ColumnFamily::Write,
    ];
    // DEFAULT_REGION_BACKOFF is not long enough for CI environment. So set a longer backoff.
    let backoff = tikv_client::Backoff::no_jitter_backoff(100, 30000, 20);
    for cf in cfs {
        let raw_client = RawClient::new(pd_addrs(), None).await.unwrap().with_cf(cf);
        raw_client
            .delete_range_opt(vec![].., backoff.clone())
            .await
            .unwrap();
    }
}

// To test with multiple regions, prewrite some data. Tests that hope to test
// with multiple regions should use keys in the corresponding ranges.
pub async fn init() -> Result<()> {
    if env::var(ENV_ENABLE_MULIT_REGION).is_ok() {
        // 1000 keys: 0..1000
        let keys_1 = std::iter::successors(Some(0u32), |x| Some(x + 1))
            .take(1000)
            .map(|x| x.to_be_bytes().to_vec());
        // 1024 keys: 0..u32::MAX
        let count = 1024;
        let step = u32::MAX / count;
        let keys_2 = std::iter::successors(Some(0u32), |x| Some(x + step))
            .take(count as usize - 1)
            .map(|x| x.to_be_bytes().to_vec());

        // about 43 regions with above keys.
        ensure_region_split(keys_1.chain(keys_2), 40).await?;
    }

    clear_tikv().await;
    let region_cnt = ctl::get_region_count().await?;
    // print log for debug convenience
    println!("init finish with {region_cnt} regions");
    Ok(())
}

async fn ensure_region_split(
    keys: impl IntoIterator<Item = impl Into<Key>>,
    region_count: u32,
) -> Result<()> {
    if ctl::get_region_count().await? as u32 >= region_count {
        return Ok(());
    }

    // 1. write plenty transactional keys
    // 2. wait until regions split

    let client = TransactionClient::new(pd_addrs(), None).await?;
    let mut txn = client.begin_optimistic().await?;
    for key in keys.into_iter() {
        txn.put(key.into(), vec![0, 0, 0, 0]).await?;
    }
    txn.commit().await?;
    let mut txn = client.begin_optimistic().await?;
    let _ = txn.scan(vec![].., 2048).await?;
    txn.commit().await?;

    info!("splitting regions...");
    let start_time = std::time::Instant::now();
    loop {
        if ctl::get_region_count().await? as u32 >= region_count {
            break;
        }
        if start_time.elapsed() > REGION_SPLIT_TIME_LIMIT {
            warn!("Stop splitting regions: time limit exceeded");
            break;
        }
        Delay::new(Duration::from_millis(200)).await;
    }

    Ok(())
}

pub fn pd_addrs() -> Vec<String> {
    env::var(ENV_PD_ADDRS)
        .unwrap_or_else(|_| {
            info!(
                "Environment variable {} is not found. Using {:?} as default.",
                ENV_PD_ADDRS, "127.0.0.1:2379"
            );
            "127.0.0.1:2379".to_owned()
        })
        .split(',')
        .map(From::from)
        .collect()
}

pub fn new_logger(level: slog::Level) -> slog::Logger {
    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
    slog::Logger::root(
        slog_term::FullFormat::new(plain)
            .build()
            .filter_level(level)
            .fuse(),
        slog::o!(),
    )
}

// helper function
pub async fn get_u32(client: &RawClient, key: Vec<u8>) -> Result<u32> {
    let x = client.get(key).await?.unwrap();
    let boxed_slice = x.into_boxed_slice();
    let array: Box<[u8; 4]> = boxed_slice
        .try_into()
        .expect("Value should not exceed u32 (4 * u8)");
    Ok(u32::from_be_bytes(*array))
}

// helper function
pub async fn get_txn_u32(txn: &mut Transaction, key: Vec<u8>) -> Result<u32> {
    let x = txn.get(key).await?.unwrap();
    let boxed_slice = x.into_boxed_slice();
    let array: Box<[u8; 4]> = boxed_slice
        .try_into()
        .expect("Value should not exceed u32 (4 * u8)");
    Ok(u32::from_be_bytes(*array))
}

// helper function
pub fn gen_u32_keys(num: u32, rng: &mut impl Rng) -> HashSet<Vec<u8>> {
    let mut set = HashSet::new();
    for _ in 0..num {
        set.insert(rng.gen::<u32>().to_be_bytes().to_vec());
    }
    set
}

/// Copied from https://github.com/tikv/tikv/blob/d86a449d7f5b656cef28576f166e73291f501d77/components/tikv_util/src/macros.rs#L55
/// Simulates Go's defer.
///
/// Please note that, different from go, this defer is bound to scope.
/// When exiting the scope, its deferred calls are executed in last-in-first-out
/// order.
#[macro_export]
macro_rules! defer {
    ($t:expr) => {
        let __ctx = $crate::DeferContext::new(|| $t);
    };
}

/// Invokes the wrapped closure when dropped.
pub struct DeferContext<T: FnOnce()> {
    t: Option<T>,
}

impl<T: FnOnce()> DeferContext<T> {
    pub fn new(t: T) -> DeferContext<T> {
        DeferContext { t: Some(t) }
    }
}

impl<T: FnOnce()> Drop for DeferContext<T> {
    fn drop(&mut self) {
        self.t.take().unwrap()()
    }
}
