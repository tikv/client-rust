// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(async_await)]

#[cfg(feature = "property-testing")]
use core::fmt::Debug;
#[cfg(feature = "property-testing")]
use proptest::{
    self,
    strategy::{Strategy, ValueTree},
};
#[cfg(feature = "integration-tests")]
use criterion::criterion_main;
#[cfg(feature = "property-testing")]
mod integration;

use std::env::var;

pub const ENV_PD_ADDR: &str = "PD_ADDR";

pub fn pd_addrs() -> Vec<String> {
    var(ENV_PD_ADDR)
        .expect(&format!("Expected {}:", ENV_PD_ADDR))
        .split(",")
        .map(From::from)
        .collect()
}

#[cfg(feature = "property-testing")]
pub fn generate<T: Debug>(strat: impl Strategy<Value = T>) -> T {
    strat
        .new_tree(&mut proptest::test_runner::TestRunner::new(
            proptest::test_runner::Config::default(),
        ))
        .unwrap()
        .current()
}

#[cfg(feature = "property-testing")]
const PROPTEST_BATCH_SIZE_MAX: usize = 16;

#[cfg(feature = "property-testing")]
pub fn arb_batch<T: core::fmt::Debug>(
    single_strategy: impl Strategy<Value = T>,
    max_batch_size: impl Into<Option<usize>>,
) -> impl Strategy<Value = Vec<T>> {
    let max_batch_size = max_batch_size.into().unwrap_or(PROPTEST_BATCH_SIZE_MAX);
    proptest::collection::vec(single_strategy, 0..max_batch_size)
}

#[cfg(feature = "integration-tests")]
criterion_main!(integration::raw::suite);

#[cfg(not(feature = "integration-tests"))]
fn main() {
    unimplemented!("Try adding the `integration-tests` feature with the `PD_ADDR` ENV variable.");
}
