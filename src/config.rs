// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::path::PathBuf;
use std::time::Duration;

use serde::de;
use serde::ser;
use serde_derive::Deserialize;
use serde_derive::Serialize;

/// Config-time keyspace selection.
///
/// This is resolved into a request-time [`crate::request::Keyspace`] when creating a client.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub enum Keyspace {
    /// Use API V1 and do not add/remove any API V2 keyspace prefix.
    #[default]
    Disable,
    /// Use API V2 and let the client add/remove API V2 keyspace/key-mode prefixes.
    ///
    /// The keyspace ID is resolved via PD using the provided keyspace name.
    Enable { name: String },
    /// Use API V2 without adding or removing the API V2 keyspace/key-mode prefix.
    ///
    /// This mode is intended for **server-side embedding** use cases (e.g. embedding this client in
    /// `tikv-server`) where keys are already in API V2 "logical key bytes" form and must be passed
    /// through unchanged.
    #[cfg(feature = "apiv2-no-prefix")]
    ApiV2NoPrefix,
}

impl Keyspace {
    fn is_disable(&self) -> bool {
        matches!(self, Keyspace::Disable)
    }
}

// Why custom serde (but still lightweight)?
//
// `Config.keyspace` used to be `Option<String>`, and existing config files commonly encoded it as:
// - missing: disable keyspace (API V1)
// - string: keyspace name (resolved via PD; API V2)
//
// `Keyspace::ApiV2NoPrefix` introduces a third mode that can't be represented as a string, but we
// must keep the legacy formats working. We implement `Serialize`/`Deserialize` using
// `#[serde(untagged)]` helper enums so we can accept multiple shapes (string / map / unit) with
// minimal boilerplate, while keeping serialization legacy-friendly (string for named keyspaces and
// omitting disabled keyspace in `Config` via `skip_serializing_if`).

#[derive(Deserialize)]
#[serde(untagged)]
enum KeyspaceDe {
    Unit(()),
    Name(String),
    Map(KeyspaceDeMap),
}

#[derive(Deserialize)]
struct KeyspaceDeMap {
    mode: Option<String>,
    name: Option<String>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum KeyspaceSer<'a> {
    Unit(()),
    Name(&'a str),
    #[cfg(feature = "apiv2-no-prefix")]
    Map(KeyspaceSerMap<'a>),
}

#[derive(Serialize)]
#[cfg(feature = "apiv2-no-prefix")]
struct KeyspaceSerMap<'a> {
    mode: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
}

impl ser::Serialize for Keyspace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let repr = match self {
            // Prefer omitting the field at the `Config` layer; if serialized standalone, this is a
            // serde "unit" value (e.g. `null` in JSON), matching the legacy `Option<String>::None`
            // meaning "no keyspace configured".
            Keyspace::Disable => KeyspaceSer::Unit(()),
            Keyspace::Enable { name } => KeyspaceSer::Name(name),
            #[cfg(feature = "apiv2-no-prefix")]
            Keyspace::ApiV2NoPrefix => KeyspaceSer::Map(KeyspaceSerMap {
                mode: "api-v2-no-prefix",
                name: None,
            }),
        };
        serde::Serialize::serialize(&repr, serializer)
    }
}

impl<'de> de::Deserialize<'de> for Keyspace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let repr = <KeyspaceDe as serde::Deserialize>::deserialize(deserializer)?;
        match repr {
            KeyspaceDe::Unit(()) => Ok(Keyspace::Disable),
            KeyspaceDe::Name(name) => Ok(Keyspace::Enable { name }),
            KeyspaceDe::Map(KeyspaceDeMap { mode, name }) => match mode.as_deref() {
                None => {
                    if let Some(name) = name {
                        Ok(Keyspace::Enable { name })
                    } else {
                        Ok(Keyspace::Disable)
                    }
                }
                Some("disable") => Ok(Keyspace::Disable),
                Some("enable") => Ok(Keyspace::Enable {
                    name: name.ok_or_else(|| de::Error::missing_field("name"))?,
                }),
                #[cfg(feature = "apiv2-no-prefix")]
                Some("api-v2-no-prefix" | "apiv2-no-prefix") => Ok(Keyspace::ApiV2NoPrefix),
                #[cfg(not(feature = "apiv2-no-prefix"))]
                Some("api-v2-no-prefix" | "apiv2-no-prefix") => Err(de::Error::custom(
                    "keyspace mode \"api-v2-no-prefix\" requires feature \"apiv2-no-prefix\"",
                )),
                Some(other) => Err(de::Error::unknown_variant(
                    other,
                    &["disable", "enable", "api-v2-no-prefix"],
                )),
            },
        }
    }
}

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
    #[serde(default, skip_serializing_if = "Keyspace::is_disable")]
    pub keyspace: Keyspace,
}

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

impl Default for Config {
    fn default() -> Self {
        Config {
            ca_path: None,
            cert_path: None,
            key_path: None,
            timeout: DEFAULT_REQUEST_TIMEOUT,
            keyspace: Keyspace::default(),
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
        self.keyspace = Keyspace::Enable {
            name: keyspace.to_owned(),
        };
        self
    }

    /// Use API V2 without adding or removing the API V2 keyspace/key-mode prefix.
    ///
    /// This is intended for **server-side embedding** use cases (e.g. embedding this client in
    /// `tikv-server`) where keys are already in API V2 "logical key bytes" form and must be passed
    /// through unchanged.
    #[cfg(feature = "apiv2-no-prefix")]
    #[must_use]
    pub fn with_api_v2_no_prefix(mut self) -> Self {
        self.keyspace = Keyspace::ApiV2NoPrefix;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_toml_deserialize_cases() {
        // Parse TOML into `Config` (covers `Keyspace` deserialization via the field).
        let cases: Vec<(&str, Keyspace)> = vec![
            ("", Keyspace::Disable),
            (
                "keyspace = \"DEFAULT\"\n",
                Keyspace::Enable {
                    name: "DEFAULT".to_owned(),
                },
            ),
            ("keyspace = { mode = \"disable\" }\n", Keyspace::Disable),
            (
                "keyspace = { mode = \"enable\", name = \"DEFAULT\" }\n",
                Keyspace::Enable {
                    name: "DEFAULT".to_owned(),
                },
            ),
            // Backward/forward compatibility: allow a structured keyspace table without `mode`.
            (
                "keyspace = { name = \"DEFAULT\" }\n",
                Keyspace::Enable {
                    name: "DEFAULT".to_owned(),
                },
            ),
            // Empty table behaves like disabled.
            ("keyspace = {}\n", Keyspace::Disable),
        ];

        #[cfg(feature = "apiv2-no-prefix")]
        let cases = {
            let mut cases = cases;
            cases.push((
                "keyspace = { mode = \"api-v2-no-prefix\" }\n",
                Keyspace::ApiV2NoPrefix,
            ));
            cases
        };

        for (input, expected_keyspace) in cases {
            let cfg: Config = toml::from_str(input).unwrap();
            assert_eq!(cfg.keyspace, expected_keyspace);
        }

        // Invalid structured forms.
        let err = toml::from_str::<Config>("keyspace = { mode = \"enable\" }\n")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("missing field") || err.contains("missing_field"),
            "unexpected error: {err}"
        );

        #[cfg(not(feature = "apiv2-no-prefix"))]
        {
            let err = toml::from_str::<Config>("keyspace = { mode = \"api-v2-no-prefix\" }\n")
                .unwrap_err()
                .to_string();
            assert!(err.contains("requires feature"), "unexpected error: {err}");
        }
    }

    #[test]
    fn test_config_toml_serialize_cases() {
        let cases: Vec<(Config, &str)> = vec![
            (Config::default(), "[timeout]\nsecs = 2\nnanos = 0\n"),
            (
                Config::default().with_keyspace("DEFAULT"),
                "keyspace = \"DEFAULT\"\n\n[timeout]\nsecs = 2\nnanos = 0\n",
            ),
        ];

        #[cfg(feature = "apiv2-no-prefix")]
        let cases = {
            let mut cases = cases;
            cases.push((
                Config::default().with_api_v2_no_prefix(),
                "[timeout]\nsecs = 2\nnanos = 0\n\n[keyspace]\nmode = \"api-v2-no-prefix\"\n",
            ));
            cases
        };

        for (cfg, expected) in cases {
            let out = toml::to_string(&cfg).unwrap();
            assert_eq!(out, expected);
        }
    }
}
