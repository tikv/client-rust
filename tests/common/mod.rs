mod ctl;

use futures_timer::Delay;
use log::{info, warn};
use std::{env, time::Duration};
use tikv_client::{ColumnFamily, Key, RawClient, Result, TransactionClient};

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
    for cf in cfs {
        let raw_client = RawClient::new(pd_addrs()).await.unwrap().with_cf(cf);
        raw_client.delete_range(vec![]..).await.unwrap();
    }
}

// To test with multiple regions, prewrite some data. Tests that hope to test
// with multiple regions should use keys in the corresponding ranges.
pub async fn init() -> Result<()> {
    // ignore SetLoggerError
    let _ = simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .init();

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

        ensure_region_split(keys_1.chain(keys_2), 100).await?;
    }

    clear_tikv().await;
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

    let client = TransactionClient::new(pd_addrs()).await?;
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
        .expect(&format!("Expected {}:", ENV_PD_ADDRS))
        .split(",")
        .map(From::from)
        .collect()
}
