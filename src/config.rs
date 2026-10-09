// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::path::PathBuf;
use std::time::Duration;

use serde_derive::Deserialize;
use serde_derive::Serialize;

/// A V3 keyspace identity.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct KeyspaceIdentity {
    pub namespace_id: u32,
    pub keyspace_id: u32,
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
    pub keyspace_identity: Option<KeyspaceIdentity>,
    pub keyspace_namespace_id: Option<u32>,
    pub keyspace_global_name_lookup: bool,
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
            keyspace_identity: None,
            keyspace_namespace_id: None,
            keyspace_global_name_lookup: false,
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
        self.keyspace_identity = None;
        self.keyspace_namespace_id = None;
        self.keyspace_global_name_lookup = false;
        self
    }

    /// Set namespace-scoped API V3 keyspace-name lookup for the client.
    #[must_use]
    pub fn with_keyspace_namespace_id(mut self, namespace_id: u32) -> Self {
        self.keyspace_identity = None;
        self.keyspace_namespace_id = Some(namespace_id);
        self.keyspace_global_name_lookup = false;
        self
    }

    /// Resolve the keyspace by globally unique name and use API V3 identity when PD returns one.
    ///
    /// This is intended for DB9-style deployments where keyspace names are globally unique even
    /// though PD's native API V3 uniqueness rule is namespace-scoped.
    #[must_use]
    pub fn with_keyspace_global_name_lookup(mut self, keyspace: &str) -> Self {
        self.keyspace = Some(keyspace.to_owned());
        self.keyspace_identity = None;
        self.keyspace_namespace_id = None;
        self.keyspace_global_name_lookup = true;
        self
    }

    /// Set the API V3 keyspace identity for the client.
    ///
    /// API V3 has no default keyspace. Both namespace id and keyspace id must be non-zero, and
    /// keyspace id must be less than `2^24`.
    #[must_use]
    pub fn with_keyspace_identity(mut self, namespace_id: u32, keyspace_id: u32) -> Self {
        self.keyspace = None;
        self.keyspace_namespace_id = None;
        self.keyspace_global_name_lookup = false;
        self.keyspace_identity = Some(KeyspaceIdentity {
            namespace_id,
            keyspace_id,
        });
        self
    }
}
