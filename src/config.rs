// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use serde_derive::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

/// The configuration for either a [`raw::Client`](raw/struct.Client.html) or a
/// [`transaction::Client`](transaction/struct.Client.html).
///
/// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for PD
/// must be provided, **not** the TiKV nodes.
///
/// It's important to **include more than one PD endpoint** (include all, if possible!)
/// This helps avoid having a *single point of failure*.
///
/// By default, this client will use an insecure connection over instead of one protected by
/// Transport Layer Security (TLS). Your deployment may have chosen to rely on security measures
/// such as a private network, or a VPN layer to provid secure transmission.
///
/// To use a TLS secured connection, use the `with_security` function to set the required
/// parameters.
///
/// TiKV does not currently offer encrypted storage (or encryption-at-rest).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub(crate) pd_endpoints: Vec<String>,
    pub(crate) ca_path: Option<PathBuf>,
    pub(crate) cert_path: Option<PathBuf>,
    pub(crate) key_path: Option<PathBuf>,
    pub(crate) timeout: Duration,
}

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

impl Config {
    /// Create a new [`Config`](struct.Config.html) which coordinates with the given PD endpoints.
    ///
    /// It's important to **include more than one PD endpoint** (include all, if possible!)
    /// This helps avoid having a *single point of failure*.
    ///
    /// ```rust
    /// # use tikv_client::Config;
    /// let config = Config::new(vec!["192.168.0.100:2379", "192.168.0.101:2379"]);
    /// ```
    pub fn new(pd_endpoints: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Config {
            pd_endpoints: pd_endpoints.into_iter().map(Into::into).collect(),
            ca_path: None,
            cert_path: None,
            key_path: None,
            timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// Set the certificate authority, certificate, and key locations for the
    /// [`Config`](struct.Config.html).
    ///
    /// By default, TiKV connections do not utilize transport layer security. Enable it by setting
    /// these values.
    ///
    /// ```rust
    /// # use tikv_client::Config;
    /// let config = Config::new(vec!["192.168.0.100:2379", "192.168.0.101:2379"])
    ///     .with_security("root.ca", "internal.cert", "internal.key");
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

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}
