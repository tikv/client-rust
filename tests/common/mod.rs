use std::env;
use tikv_client::{ColumnFamily, RawClient};

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

const ENV_PD_ADDRS: &str = "PD_ADDRS";

pub fn pd_addrs() -> Vec<String> {
    env::var(ENV_PD_ADDRS)
        .expect(&format!("Expected {}:", ENV_PD_ADDRS))
        .split(",")
        .map(From::from)
        .collect()
}
