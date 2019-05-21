// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

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
