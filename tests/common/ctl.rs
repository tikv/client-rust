// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.
//! The module provides some utility functions to control and get information
//! from PD, using its HTTP API.

use super::pd_addrs;
use tikv_client_common::{Error, Result};

pub async fn get_region_count() -> Result<u64> {
    let res = reqwest::get(format!("http://{}/pd/api/v1/regions", pd_addrs()[0]))
        .await
        .map_err(|e| Error::StringError(e.to_string()))?;

    let body = res
        .text()
        .await
        .map_err(|e| Error::StringError(e.to_string()))?;
    let value: serde_json::Value = serde_json::from_str(body.as_ref()).unwrap();
    value["count"]
        .as_u64()
        .ok_or_else(|| Error::StringError("pd region count does not return an integer".to_owned()))
}
