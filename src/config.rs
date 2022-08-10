// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use serde_derive::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
use tikv_client_store::KvClientConfig;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
/// The configuration for either a [`RawClient`](crate::RawClient) or a
/// [`TransactionClient`](crate::TransactionClient).
///
/// See also [`TransactionOptions`](crate::TransactionOptions) which provides more ways to configure
/// requests.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub ca_path: Option<PathBuf>,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub timeout: Duration,
    pub kv_config: KvClientConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ca_path: None,
            cert_path: None,
            key_path: None,
            timeout: DEFAULT_REQUEST_TIMEOUT,
            kv_config: KvClientConfig::default(),
        }
    }
}

impl Config {
    /// Set the certificate authority, certificate, and key locations for clients.
    ///
    /// By default, this client will use an insecure connection over instead of one protected by
    /// Transport Layer Security (TLS). Your deployment may have chosen to rely on security measures
    /// such as a private network, or a VPN layer to provide secure transmission.
    ///
    /// To use a TLS secured connection, use the `with_security` function to set the required
    /// parameters.
    ///
    /// TiKV does not currently offer encrypted storage (or encryption-at-rest).
    ///
    /// # Examples
    /// ```rust
    /// # use tikv_client::Config;
    /// let config = Config::default().with_security("root.ca", "internal.cert", "internal.key");
    /// ```
    #[must_use]
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

    /// Set the timeout for clients.
    ///
    /// The timeout is used for all requests when using or connecting to a TiKV cluster (including
    /// PD nodes). If the request does not complete within timeout, the request is cancelled and
    /// an error returned to the user.
    ///
    /// The default timeout is two seconds.
    ///
    /// # Examples
    /// ```rust
    /// # use tikv_client::Config;
    /// # use std::time::Duration;
    /// let config = Config::default().with_timeout(Duration::from_secs(10));
    /// ```
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    // TODO: add more config options for tivk client config
    pub fn with_kv_timeout(mut self, timeout: u64) -> Self {
        self.kv_config.request_timeout = timeout;
        self
    }

    pub fn with_kv_completion_queue_size(mut self, size: usize) -> Self {
        self.kv_config.completion_queue_size = size;
        self
    }

    pub fn with_kv_grpc_keepalive_time(mut self, time: u64) -> Self {
        self.kv_config.grpc_keepalive_time = time;
        self
    }

    pub fn with_kv_grpc_keepalive_timeout(mut self, timeout: u64) -> Self {
        self.kv_config.grpc_keepalive_timeout = timeout;
        self
    }

    pub fn with_kv_allow_batch(mut self, allow_batch: bool) -> Self {
        self.kv_config.allow_batch = allow_batch;
        self
    }

    pub fn with_kv_overload_threshold(mut self, threshold: usize) -> Self {
        self.kv_config.overload_threshold = threshold;
        self
    }

    pub fn with_kv_max_batch_wait_time(mut self, wait: u64) -> Self {
        self.kv_config.max_batch_wait_time = wait;
        self
    }

    pub fn with_kv_max_batch_size(mut self, size: usize) -> Self {
        self.kv_config.max_batch_size = size;
        self
    }
}
