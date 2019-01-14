// Copyright 2019 The TiKV Project Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

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
