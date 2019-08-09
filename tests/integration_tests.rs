#![cfg(feature = "integration-tests")]
#![feature(async_await)]

use failure::Fallible;
use futures::executor::ThreadPool;
use futures::prelude::*;
use std::env;
use tikv_client::{Config, Result, TransactionClient};

#[test]
fn get_timestamp() -> Fallible<()> {
    const COUNT: usize = 1 << 16;
    let mut pool = ThreadPool::new()?;
    let config = Config::new(pd_addrs());
    let fut = async {
        let client = TransactionClient::connect(config).await?;
        Result::Ok(future::join_all((0..COUNT).map(|_| client.current_timestamp())).await)
    };
    // Calculate each version of retrieved timestamp
    let mut versions = pool
        .run(fut)?
        .into_iter()
        .map(|res| res.map(|ts| (ts.physical << 18) + ts.logical))
        .collect::<Result<Vec<_>>>()?;

    // Each version should be unique
    versions.sort_unstable();
    versions.dedup();
    assert_eq!(versions.len(), COUNT);
    Ok(())
}

const ENV_PD_ADDRS: &str = "PD_ADDRS";

fn pd_addrs() -> Vec<String> {
    env::var(ENV_PD_ADDRS)
        .expect(&format!("Expected {}:", ENV_PD_ADDRS))
        .split(",")
        .map(From::from)
        .collect()
}
