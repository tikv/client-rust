// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

// All integration tests are deliberately annotated by
//
// ```
// #[cfg_attr(not(feature = "integration-tests"), ignore)]
// ```
//
// This is so that integration tests are still compiled even if they aren't run.
// This helps avoid having broken integration tests even if they aren't run.

// If integration tests aren't being run, we still want to build them. We'll have unused things
// though. Allow that.
#![cfg_attr(not(feature = "integration-tests"), allow(dead_code))]

pub mod raw;
