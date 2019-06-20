// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

// All integration tests are deliberately annotated by
//
// ```
// #[test]
// #[cfg_attr(not(feature = "integration-tests"), ignore)]
// ```
//
// This is so that integration tests are still compiled even if they aren't run.
// This helps avoid having broken integration tests even if they aren't run.

mod raw;

use std::env::var;
const ENV_PD_ADDR: &str = "PD_ADDR";

pub fn pd_addr() -> Vec<String> {
    var(ENV_PD_ADDR)
        .expect(&format!("Expected {}:", ENV_PD_ADDR))
        .split(",")
        .map(From::from)
        .collect()
}
