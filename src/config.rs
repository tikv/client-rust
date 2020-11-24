// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use serde_derive::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

/// The configuration for either a `raw::Client` or a `transaction::Client`.
///
/// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for PD
/// must be provided, **not** the TiKV nodes.
///
/// It's important to **include more than one PD endpoint** (include all, if possible!)
/// This helps avoid having a *single point of failure*.
///
/// By default, this client will use an insecure connection over instead of one protected by
/// Transport Layer Security (TLS). Your deployment may have chosen to rely on security measures
/// such as a private network, or a VPN layer to provide secure transmission.
///
/// To use a TLS secured connection, use the `with_security` function to set the required
/// parameters.
///
/// TiKV does not currently offer encrypted storage (or encryption-at-rest).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub ca_path: Option<PathBuf>,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub timeout: Duration,
}

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

impl Default for Config {
    fn default() -> Self {
        Config {
            ca_path: None,
            cert_path: None,
            key_path: None,
            timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }
}

impl Config {
    /// Set the certificate authority, certificate, and key locations for the
    /// [`Config`](Config).
    ///
    /// By default, TiKV connections do not utilize transport layer security. Enable it by setting
    /// these values.
    ///
    /// # Examples
    /// ```rust
    /// # use tikv_client::Config;
    /// let config = Config::default().with_security("root.ca", "internal.cert", "internal.key");
    /// ```
    pub fn with_security(
        mut self,
        ca_path: impl Into<PathBuf>,
        cert_path: impl Into<PathBuf>,
        key_path: impl Into<PathBuf>,
    ) -> Self {
        self.ca_path = Some(ca_path.into());
        self.cert_path = Some(cert_path.into());
        self.key_path = Some(key_path.into());
        self
    }

    /// Set the timeout for the [`Config`](Config).
    ///
    ///
    /// # Examples
    /// ```rust
    /// # use tikv_client::Config;
    /// # use std::time::Duration;
    /// let config = Config::default().timeout(Duration::from_secs(10));
    /// ```
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}
