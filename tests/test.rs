// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(async_await)]

#[cfg(feature = "integration-tests")]
mod integration;

#[cfg(feature = "proptest")]
use proptest::strategy::Strategy;

#[cfg(feature = "proptest")]
const PROPTEST_BATCH_SIZE_MAX: usize = 16;

#[cfg(feature = "proptest")]
pub fn arb_batch<T: core::fmt::Debug>(single_strategy: impl Strategy<Value = T>, max_batch_size: impl Into<Option<usize>>) -> impl Strategy<Value = Vec<T>> {
    let max_batch_size = max_batch_size.into().unwrap_or(PROPTEST_BATCH_SIZE_MAX);
    proptest::collection::vec(single_strategy, 0..max_batch_size)
}