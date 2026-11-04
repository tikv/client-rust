// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::path::PathBuf;
use std::time::Duration;

use serde_derive::Deserialize;
use serde_derive::Serialize;

pub use crate::proto::kvrpcpb::{RequestOrigin, TxnProtocolVersion};

/// Per-client policy inherited by transactions and lock resolvers.
/// Supports `0` (legacy) and `1` (structured incompatibility handling), defaulting
/// to `1`. Shared-lock version `2` is unsupported. Direct callers must choose a
/// supported version; transactional client construction validates [`Config`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxnRequestPolicy {
    /// Default product identity for auditing, applied when the request origin is `Unknown`.
    pub origin: RequestOrigin,
    /// Configured version; each destination may select a lower compatible version.
    pub protocol_version: TxnProtocolVersion,
}

impl TxnRequestPolicy {
    /// Default transactional request origin (`Unknown`) and protocol version (`1`).
    pub const DEFAULT: Self = Self {
        origin: RequestOrigin::Unknown,
        protocol_version: TxnProtocolVersion::TxnVerSupportIncompatibleErrorHandling,
    };
}

impl Default for TxnRequestPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The configuration for either a [`RawClient`](crate::RawClient) or a
/// [`TransactionClient`](crate::TransactionClient).
///
/// See also [`TransactionOptions`](crate::TransactionOptions) which provides more ways to configure
/// requests.
///
/// This struct is marked `#[non_exhaustive]` to allow adding new configuration options in the
/// future without breaking downstream code. Construct it via [`Config::default`] and then use the
/// `with_*` methods (or field assignment) to customize it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct Config {
    pub ca_path: Option<PathBuf>,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub timeout: Duration,
    pub grpc_max_decoding_message_size: usize,
    pub keyspace: Option<String>,
    /// Product origin for transactional requests, encoded as a protobuf enum value.
    /// Defaults to [`RequestOrigin::Unknown`].
    pub request_origin: i32,
    /// Configured transaction protocol version, encoded as a protobuf enum value.
    /// Defaults to `1`; transactional client construction accepts only `0` and `1`.
    pub txn_protocol_version: i32,
}

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_GRPC_MAX_DECODING_MESSAGE_SIZE: usize = 4 * 1024 * 1024; // 4MB

impl Default for Config {
    fn default() -> Self {
        Config {
            ca_path: None,
            cert_path: None,
            key_path: None,
            timeout: DEFAULT_REQUEST_TIMEOUT,
            grpc_max_decoding_message_size: DEFAULT_GRPC_MAX_DECODING_MESSAGE_SIZE,
            keyspace: None,
            request_origin: TxnRequestPolicy::DEFAULT.origin as i32,
            txn_protocol_version: TxnRequestPolicy::DEFAULT.protocol_version as i32,
        }
    }
}

impl Config {
    /// Set the transactional request origin; defaults to [`RequestOrigin::Unknown`].
    #[must_use]
    pub fn with_request_origin(mut self, origin: RequestOrigin) -> Self {
        self.request_origin = origin as i32;
        self
    }

    /// Set the transaction protocol version; defaults to `1`.
    /// Transactional client construction validates this setting; the setter does not.
    /// See [`TxnRequestPolicy`] for supported versions.
    #[must_use]
    pub fn with_txn_protocol_version(mut self, version: TxnProtocolVersion) -> Self {
        self.txn_protocol_version = version as i32;
        self
    }

    pub(crate) fn txn_request_policy(&self) -> crate::Result<TxnRequestPolicy> {
        let origin = RequestOrigin::try_from(self.request_origin).map_err(|_| {
            crate::Error::StringError(format!("unknown request origin: {}", self.request_origin))
        })?;
        let protocol_version =
            TxnProtocolVersion::try_from(self.txn_protocol_version).map_err(|_| {
                crate::Error::StringError(format!(
                    "unknown transaction protocol version: {}",
                    self.txn_protocol_version
                ))
            })?;
        if protocol_version == TxnProtocolVersion::TxnVerSupportSharedLock {
            return Err(crate::Error::StringError(
                "shared-lock transaction protocol version is not supported".to_owned(),
            ));
        }
        Ok(TxnRequestPolicy {
            origin,
            protocol_version,
        })
    }

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

    /// Set the maximum decoding message size for gRPC.
    #[must_use]
    pub fn with_grpc_max_decoding_message_size(mut self, size: usize) -> Self {
        self.grpc_max_decoding_message_size = size;
        self
    }

    /// Set to use default keyspace.
    ///
    /// Server should enable `storage.api-version = 2` to use this feature.
    #[must_use]
    pub fn with_default_keyspace(self) -> Self {
        self.with_keyspace("DEFAULT")
    }

    /// Set the use keyspace for the client.
    ///
    /// Server should enable `storage.api-version = 2` to use this feature.
    #[must_use]
    pub fn with_keyspace(mut self, keyspace: &str) -> Self {
        self.keyspace = Some(keyspace.to_owned());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_policy_defaults_and_configured_values_are_preserved() {
        assert_eq!(
            Config::default().txn_request_policy().unwrap(),
            TxnRequestPolicy::DEFAULT
        );
        let config = Config::default()
            .with_request_origin(RequestOrigin::TiFlash)
            .with_txn_protocol_version(TxnProtocolVersion::TxnVerLegacy);
        assert_eq!(
            config.txn_request_policy().unwrap(),
            TxnRequestPolicy {
                origin: RequestOrigin::TiFlash,
                protocol_version: TxnProtocolVersion::TxnVerLegacy,
            }
        );
        let restored: Config =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(
            restored.txn_request_policy().unwrap(),
            config.txn_request_policy().unwrap()
        );
    }

    #[test]
    fn transaction_policy_rejects_unsupported_versions_and_unknown_origins() {
        for version in [-1, 2, 42] {
            let mut config = Config::default();
            config.txn_protocol_version = version;
            assert!(config.txn_request_policy().is_err());
        }
        let mut config = Config::default();
        config.request_origin = 42;
        assert!(config.txn_request_policy().is_err());
    }
}
