// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::path::PathBuf;
use std::time::Duration;

use crate::{Error, Result};
use serde_derive::Deserialize;
use serde_derive::Serialize;

/// Identifies the process issuing transaction RPCs for compatibility auditing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RequestOrigin {
    #[default]
    Unknown,
    TiDb,
    TiCdc,
    Br,
    TiFlash,
}

impl RequestOrigin {
    pub(crate) fn as_proto(self) -> i32 {
        use crate::proto::kvrpcpb::RequestOrigin as ProtoOrigin;
        match self {
            Self::Unknown => ProtoOrigin::Unknown as i32,
            Self::TiDb => ProtoOrigin::TiDb as i32,
            Self::TiCdc => ProtoOrigin::TiCdc as i32,
            Self::Br => ProtoOrigin::Br as i32,
            Self::TiFlash => ProtoOrigin::TiFlash as i32,
        }
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
    #[serde(skip)]
    pub(crate) default_txn_protocol_version: u32,
    #[serde(skip)]
    pub(crate) request_origin: RequestOrigin,
}

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_GRPC_MAX_DECODING_MESSAGE_SIZE: usize = 4 * 1024 * 1024; // 4MB
pub(crate) const MAX_SUPPORTED_TXN_PROTOCOL_VERSION: u32 =
    crate::proto::kvrpcpb::TxnProtocolVersion::TxnVerSupportIncompatibleErrorHandling as u32;
pub(crate) const DEFAULT_TXN_PROTOCOL_VERSION: u32 = MAX_SUPPORTED_TXN_PROTOCOL_VERSION;

impl Default for Config {
    fn default() -> Self {
        Config {
            ca_path: None,
            cert_path: None,
            key_path: None,
            timeout: DEFAULT_REQUEST_TIMEOUT,
            grpc_max_decoding_message_size: DEFAULT_GRPC_MAX_DECODING_MESSAGE_SIZE,
            keyspace: None,
            default_txn_protocol_version: DEFAULT_TXN_PROTOCOL_VERSION,
            request_origin: RequestOrigin::Unknown,
        }
    }
}

impl Config {
    /// Set the highest transaction protocol version this client can understand.
    pub fn with_default_txn_protocol_version(mut self, version: u32) -> Result<Self> {
        let max = MAX_SUPPORTED_TXN_PROTOCOL_VERSION;
        if version > max {
            return Err(Error::StringError(format!(
                "unsupported transaction protocol version {version}; maximum is {max}"
            )));
        }
        self.default_txn_protocol_version = version;
        Ok(self)
    }

    /// Set the process origin attached to transaction RPCs.
    #[must_use]
    pub fn with_request_origin(mut self, origin: RequestOrigin) -> Self {
        self.request_origin = origin;
        self
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
    fn transaction_protocol_version_is_bounded() {
        assert_eq!(Config::default().default_txn_protocol_version, 1);
        assert_eq!(
            Config::default()
                .with_default_txn_protocol_version(0)
                .unwrap()
                .default_txn_protocol_version,
            0
        );
        assert!(Config::default()
            .with_default_txn_protocol_version(2)
            .is_err());
    }

    #[test]
    fn compatibility_fields_are_not_deserialized() {
        let config: Config = serde_json::from_str(
            r#"{"default-txn-protocol-version":0,"request-origin":"TiFlash"}"#,
        )
        .unwrap();
        assert_eq!(config.default_txn_protocol_version, 1);
        assert_eq!(config.request_origin, RequestOrigin::Unknown);
    }
}
