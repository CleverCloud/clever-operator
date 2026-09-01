//! # Configuration module
//!
//! This module provides utilities and helpers to interact with the configuration

use std::{
    convert::TryFrom,
    env::{self, VarError},
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use clevercloud_sdk::{
    Credentials,
    oauth10a::{reqwest, url::Url},
};
use config::{Config, ConfigError, File, FileFormat};
use serde::{Deserialize, Deserializer, Serialize};
use tracing::warn;

// -----------------------------------------------------------------------------
// Constants

pub const OPERATOR_LISTEN: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8000);

// -----------------------------------------------------------------------------
// Operator structure

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Operator {
    #[serde(rename = "listen")]
    pub listen: SocketAddr,
}

impl Default for Operator {
    fn default() -> Self {
        Self {
            listen: OPERATOR_LISTEN,
        }
    }
}

// -----------------------------------------------------------------------------
// ConfigurationError enum

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to load configuration, {0}")]
    Build(ConfigError),
    #[error("failed to deserialize configuration, {0}")]
    Deserialize(ConfigError),
    #[error("failed to set default for key '{0}', {1}")]
    Default(String, ConfigError),
    #[error("failed to retrieve environment variable '{0}', {1}")]
    EnvironmentVariable(&'static str, VarError),
    #[error("invalid value for configuration key '{0}', {1}")]
    InvalidValue(&'static str, String),
}

// -----------------------------------------------------------------------------
// Api structure

/// Returns the endpoint stripped from its trailing slashes, or a message
/// explaining why the value cannot be used.
///
/// The sdk joins paths to the endpoint as-is, so a trailing slash would send
/// every request to a doubled separator, and the http client can only reach
/// `http` and `https` urls.
fn normalise_endpoint(endpoint: &str) -> Result<String, String> {
    let url = Url::parse(endpoint).map_err(|err| format!("expected a valid url, {err}"))?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!(
            "expected an 'http' or 'https' url, found scheme '{}'",
            url.scheme()
        ));
    }

    if url.query().is_some() || url.fragment().is_some() {
        return Err("expected an url without query string nor fragment".to_string());
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err("expected an url without embedded credentials".to_string());
    }

    // Return the parsed form rather than the input, so nothing the parser
    // tolerates but the concatenation would break survives, and strip the
    // trailing slash the url normalisation adds back.
    Ok(url.as_str().trim_end_matches('/').to_string())
}

/// Returns the endpoint given by the environment, treating a blank value as
/// unset: an empty variable is how a `ConfigMap` or a chart expresses "not
/// configured", it must not be a start-up failure.
fn endpoint_from_env() -> Option<String> {
    env::var("CLEVER_OPERATOR_API_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn deserialize_endpoint<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        None => Ok(None),
        Some(endpoint) => normalise_endpoint(&endpoint)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// Returns the message carried by a reqwest error.
///
/// Its own `Display` is only the kind of the error, e.g. "builder error", the
/// reason lives in its source.
fn reason(err: &reqwest::Error) -> String {
    match std::error::Error::source(err) {
        Some(source) => format!("{err}, {source}"),
        None => err.to_string(),
    }
}

/// Returns the certificate authorities held by the pem bundle at `path`, or a
/// message explaining why the file cannot be used.
///
/// The http client is built with the Mozilla root bundle compiled into it: it
/// reads neither the system trust store nor `SSL_CERT_FILE`, so the authority of
/// a self-hosted installation has to be handed over explicitly.
pub fn read_ca_bundle(path: &Path) -> Result<Vec<reqwest::Certificate>, String> {
    let content =
        fs::read(path).map_err(|err| format!("failed to read '{}', {err}", path.display()))?;

    let certificates = reqwest::Certificate::from_pem_bundle(&content).map_err(|err| {
        format!(
            "failed to parse '{}' as a pem bundle, {}",
            path.display(),
            reason(&err)
        )
    })?;

    // Anything the parser does not recognise as a certificate is skipped rather
    // than reported: an empty result is how a file holding no pem block at all,
    // or only a private key, comes back. Accepting it would leave the authority
    // untrusted while everything looks configured.
    if certificates.is_empty() {
        return Err(format!(
            "expected at least one pem certificate in '{}'",
            path.display()
        ));
    }

    Ok(certificates)
}

/// Returns the certificate authority bundle given by the environment, treating a
/// blank value as unset, for the same reason as [`endpoint_from_env`].
fn ca_bundle_from_env() -> Option<String> {
    env::var("CLEVER_OPERATOR_API_CA_BUNDLE")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Rejects a blank value, which is not a path and would otherwise be reported as
/// a missing file.
///
/// The bundle itself is read by [`Configuration::validate`], not here: the path
/// it holds is the one of the pod, and the machine encoding a manifest with
/// `configmap generate` or `secret generate` has no reason to carry that file.
fn deserialize_ca_bundle<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        None => Ok(None),
        Some(path) if path.trim().is_empty() => Err(serde::de::Error::custom(
            "expected a path to a pem certificate bundle, found an empty value",
        )),
        Some(path) => Ok(Some(PathBuf::from(path))),
    }
}

/// How to reach the Clever Cloud api: where to send the requests and how to
/// authenticate them.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Api {
    /// Base url of the Clever Cloud api, without any trailing slash, e.g.
    /// `https://api.clever-cloud.com`.
    ///
    /// It is optional on purpose. When it is not set, the sdk falls back on the
    /// public api that matches the kind of credentials, which is the api bridge
    /// for a bare bearer token and [`clevercloud_sdk::PUBLIC_ENDPOINT`]
    /// otherwise.
    #[serde(
        rename = "endpoint",
        default,
        deserialize_with = "deserialize_endpoint",
        skip_serializing_if = "Option::is_none"
    )]
    pub endpoint: Option<String>,
    /// Path to a pem file holding the certificate authorities to trust on top of
    /// the ones compiled into the http client.
    ///
    /// A self-hosted installation is commonly served by a certificate signed by
    /// a private authority, which the client would otherwise refuse. It is a
    /// path and not inline pem so it can be mounted from a `ConfigMap` or a
    /// `Secret`.
    #[serde(
        rename = "ca_bundle",
        default,
        deserialize_with = "deserialize_ca_bundle",
        skip_serializing_if = "Option::is_none"
    )]
    pub ca_bundle: Option<PathBuf>,
    /// Credentials used to sign the requests, the variant is picked from the
    /// keys that are present, see [`Credentials`].
    #[serde(flatten)]
    pub credentials: Credentials,
}

impl Api {
    /// Falls back on `api` for every key this section does not declare.
    ///
    /// A per-namespace secret overrides the credentials of a namespace, not the
    /// installation they belong to: without this it would silently target the
    /// public api while the operator talks to another one, and lose the
    /// certificate authorities that installation is served with.
    pub fn inherit_from(&mut self, api: &Api) {
        if self.endpoint.is_none() {
            self.endpoint.clone_from(&api.endpoint);
        }

        if self.ca_bundle.is_none() {
            self.ca_bundle.clone_from(&api.ca_bundle);
        }
    }
}

// -----------------------------------------------------------------------------
// NamespaceConfiguration structures

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct NamespaceConfiguration {
    #[serde(rename = "api")]
    pub api: Api,
}

impl TryFrom<&str> for NamespaceConfiguration {
    type Error = Error;

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn try_from(content: &str) -> Result<Self, Self::Error> {
        // No default is set here on purpose: the secret carries the credentials
        // of the namespace on its own, so the same content resolves to the same
        // kind of credentials as it would in the global configuration file, and
        // an absent endpoint or certificate authority bundle means "inherit
        // the global one", see [`Api::inherit_from`].
        Config::builder()
            .add_source(File::from_str(content, FileFormat::Toml))
            .build()
            .map_err(Error::Build)?
            .try_deserialize()
            .map_err(Error::Deserialize)
    }
}

// -----------------------------------------------------------------------------
// Configuration structures

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Configuration {
    #[serde(rename = "api")]
    pub api: Api,
    #[serde(rename = "operator")]
    pub operator: Operator,
}

impl TryFrom<PathBuf> for Configuration {
    type Error = Error;

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let mut builder = Config::builder();

        // -----------------------------------------------------------------
        // Api
        if let Ok(value) = env::var("CLEVER_OPERATOR_API_TOKEN") {
            builder = builder
                .set_default("api.token", value)
                .map_err(|err| Error::Default("api.token".into(), err))?;
        }

        if let Ok(value) = env::var("CLEVER_OPERATOR_API_SECRET") {
            builder = builder
                .set_default("api.secret", value)
                .map_err(|err| Error::Default("api.secret".into(), err))?;
        }

        if let Ok(value) = env::var("CLEVER_OPERATOR_API_CONSUMER_KEY") {
            builder = builder
                .set_default("api.consumer-key", value)
                .map_err(|err| Error::Default("api.consumer-key".into(), err))?;
        }

        if let Ok(value) = env::var("CLEVER_OPERATOR_API_CONSUMER_SECRET") {
            builder = builder
                .set_default("api.consumer-secret", value)
                .map_err(|err| Error::Default("api.consumer-secret".into(), err))?;
        }

        if let Some(value) = endpoint_from_env() {
            builder = builder
                .set_default("api.endpoint", value)
                .map_err(|err| Error::Default("api.endpoint".into(), err))?;
        }

        if let Some(value) = ca_bundle_from_env() {
            builder = builder
                .set_default("api.ca_bundle", value)
                .map_err(|err| Error::Default("api.ca_bundle".into(), err))?;
        }

        builder
            // -----------------------------------------------------------------
            // Operator
            .set_default(
                "operator.listen",
                env::var("CLEVER_OPERATOR_OPERATOR_LISTEN")
                    .unwrap_or_else(|_err| OPERATOR_LISTEN.to_string()),
            )
            .map_err(|err| Error::Default("operator.listen".into(), err))?
            // -----------------------------------------------------------------
            // Files
            .add_source(File::from(path).required(true))
            .build()
            .map_err(Error::Build)?
            .try_deserialize()
            .map_err(Error::Deserialize)
    }
}

impl Configuration {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn try_from_clever_tools() -> Result<Self, Error> {
        let mut builder = Config::builder();

        // -----------------------------------------------------------------
        // Api
        if let Ok(value) = env::var("CLEVER_OPERATOR_API_TOKEN") {
            builder = builder
                .set_default("api.token", value)
                .map_err(|err| Error::Default("api.token".into(), err))?;
        }

        if let Ok(value) = env::var("CLEVER_OPERATOR_API_SECRET") {
            builder = builder
                .set_default("api.secret", value)
                .map_err(|err| Error::Default("api.secret".into(), err))?;
        }

        if let Ok(value) = env::var("CLEVER_OPERATOR_API_CONSUMER_KEY") {
            builder = builder
                .set_default("api.consumer-key", value)
                .map_err(|err| Error::Default("api.consumer-key".into(), err))?;
        }

        if let Ok(value) = env::var("CLEVER_OPERATOR_API_CONSUMER_SECRET") {
            builder = builder
                .set_default("api.consumer-secret", value)
                .map_err(|err| Error::Default("api.consumer-secret".into(), err))?;
        }

        let credentials: Credentials = builder
            // -----------------------------------------------------------------
            // Operator
            .set_default(
                "operator.listen",
                env::var("CLEVER_OPERATOR_OPERATOR_LISTEN")
                    .unwrap_or_else(|_err| OPERATOR_LISTEN.to_string()),
            )
            .map_err(|err| Error::Default("operator.listen".into(), err))?
            // -----------------------------------------------------------------
            // Files
            .add_source(
                File::from(PathBuf::from(format!(
                    "{}/.config/clever-cloud/clever-tools",
                    env::var("HOME").map_err(|err| Error::EnvironmentVariable("HOME", err))?,
                )))
                .required(false),
            )
            .build()
            .map_err(Error::Build)?
            .try_deserialize()
            .map_err(Error::Deserialize)?;

        // The clever-tools file only carries credentials, the endpoint and the
        // certificate authority bundle can solely come from the environment
        // here.
        let endpoint = match endpoint_from_env() {
            Some(value) => Some(
                normalise_endpoint(&value)
                    .map_err(|err| Error::InvalidValue("api.endpoint", err))?,
            ),
            None => None,
        };

        let ca_bundle = ca_bundle_from_env().map(PathBuf::from);

        Ok(Self {
            api: Api {
                endpoint,
                ca_bundle,
                credentials,
            },
            operator: Operator::default(),
        })
    }

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn try_default() -> Result<Self, Error> {
        let mut builder = Config::builder();

        // -----------------------------------------------------------------
        // Api
        if let Ok(value) = env::var("CLEVER_OPERATOR_API_TOKEN") {
            builder = builder
                .set_default("api.token", value)
                .map_err(|err| Error::Default("api.token".into(), err))?;
        }

        if let Ok(value) = env::var("CLEVER_OPERATOR_API_SECRET") {
            builder = builder
                .set_default("api.secret", value)
                .map_err(|err| Error::Default("api.secret".into(), err))?;
        }

        if let Ok(value) = env::var("CLEVER_OPERATOR_API_CONSUMER_KEY") {
            builder = builder
                .set_default("api.consumer-key", value)
                .map_err(|err| Error::Default("api.consumer-key".into(), err))?;
        }

        if let Ok(value) = env::var("CLEVER_OPERATOR_API_CONSUMER_SECRET") {
            builder = builder
                .set_default("api.consumer-secret", value)
                .map_err(|err| Error::Default("api.consumer-secret".into(), err))?;
        }

        if let Some(value) = endpoint_from_env() {
            builder = builder
                .set_default("api.endpoint", value)
                .map_err(|err| Error::Default("api.endpoint".into(), err))?;
        }

        if let Some(value) = ca_bundle_from_env() {
            builder = builder
                .set_default("api.ca_bundle", value)
                .map_err(|err| Error::Default("api.ca_bundle".into(), err))?;
        }

        builder
            // -----------------------------------------------------------------
            // Operator
            .set_default(
                "operator.listen",
                env::var("CLEVER_OPERATOR_OPERATOR_LISTEN")
                    .unwrap_or_else(|_err| OPERATOR_LISTEN.to_string()),
            )
            .map_err(|err| Error::Default("operator.listen".into(), err))?
            // -----------------------------------------------------------------
            // Files
            .add_source(
                File::from(PathBuf::from(format!(
                    "/usr/share/{}/config",
                    env!("CARGO_PKG_NAME")
                )))
                .required(false),
            )
            .add_source(
                File::from(PathBuf::from(format!(
                    "/etc/{}/config",
                    env!("CARGO_PKG_NAME")
                )))
                .required(false),
            )
            .add_source(
                File::from(PathBuf::from(format!(
                    "{}/.config/{}/config",
                    env::var("HOME").map_err(|err| Error::EnvironmentVariable("HOME", err))?,
                    env!("CARGO_PKG_NAME")
                )))
                .required(false),
            )
            .add_source(
                File::from(PathBuf::from(format!(
                    "{}/.local/share/{}/config",
                    env::var("HOME").map_err(|err| Error::EnvironmentVariable("HOME", err))?,
                    env!("CARGO_PKG_NAME")
                )))
                .required(false),
            )
            .add_source(File::from(PathBuf::from("config")).required(false))
            .build()
            .map_err(Error::Build)?
            .try_deserialize()
            .map_err(Error::Deserialize)
    }

    /// Checks the values the parser deliberately leaves alone, so an unusable
    /// one is reported before anything runs rather than on every reconciliation
    /// with an opaque error.
    ///
    /// Only reading the certificate authority bundle falls in that category: it
    /// needs the file to be there, which is true of the pod but not of the
    /// machine encoding a manifest with `configmap generate` or `secret
    /// generate`. The daemon goes through it when it builds its client and
    /// refuses to start on failure, `--check` reports the same thing without
    /// starting anything.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(path) = &self.api.ca_bundle {
            read_ca_bundle(path).map_err(|err| Error::InvalidValue("api.ca_bundle", err))?;
        }

        Ok(())
    }

    /// Prints a message about missing value for a configuration key
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn help(&self) {
        #[cfg(feature = "logging")]
        tracing::info!(feature = "logging", "Build with feature flag");

        #[cfg(feature = "metrics")]
        tracing::info!(feature = "metrics", "Build with feature flag");

        #[cfg(feature = "tracing")]
        tracing::info!(feature = "tracing", "Build with feature flag");

        match &self.api.credentials {
            Credentials::OAuth1 {
                consumer_key,
                consumer_secret,
                token,
                secret,
            } => {
                if consumer_key.is_empty() {
                    warn!(
                        key = "api.consumer-key",
                        "Configuration key has an empty value"
                    );
                }

                if consumer_secret.is_empty() {
                    warn!(
                        key = "api.consumer-secret",
                        "Configuration key has an empty value"
                    );
                }

                if token.is_empty() {
                    warn!(key = "api.token", "Configuration key has an empty value");
                }

                if secret.is_empty() {
                    warn!(key = "api.secret", "Configuration key has an empty value");
                }
            }
            Credentials::Basic { username, password } => {
                if username.is_empty() {
                    warn!(key = "api.username", "Configuration key has an empty value");
                }

                if password.is_empty() {
                    warn!(key = "api.password", "Configuration key has an empty value");
                }
            }
            Credentials::Bearer { token } => {
                if token.is_empty() {
                    warn!(key = "api.token", "Configuration key has an empty value");
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Test fixtures

/// A self-signed certificate authority, and the helper dropping it on the disk.
///
/// The bundle is read while the configuration is parsed, so both this module and
/// the client one need a real file to point at.
#[cfg(test)]
pub(crate) const CA_BUNDLE: &str = "-----BEGIN CERTIFICATE-----
MIIDHzCCAgegAwIBAgIUD4ReiGbRHedBZdoEEiPyKxBOsjowDQYJKoZIhvcNAQEL
BQAwHzEdMBsGA1UEAwwUQ2xldmVyIENsb3VkIFRlc3QgQ0EwHhcNMjYwOTAxMTMx
NjM0WhcNMzYwODI5MTMxNjM0WjAfMR0wGwYDVQQDDBRDbGV2ZXIgQ2xvdWQgVGVz
dCBDQTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAKbTuKeZw4r/zU55
jF8lTE6v/BTcflLCoGHYgdoagi/42SZNlZ9ZiYLiFMsUaH8s8YvlvnThCMlpN/C7
SssMycS4EwafROPBuCnReT7hWiRRYe8j7vPCWhyqtXTbhG8Y3H5AzHnl+jx52wwm
GTXbchWEthFVvBmqaUbLz5gGdoso1hqEinG0t2tE+WkT6N/b2ahvRw6XbA5ms1AK
LmmvNjbdfn6SjVssCQCYOVfsMLNW7c0sM5uZ55PdahQWqCGt3Wcrox1ZJQkRzwxr
hXjEP7rRFdMJRwlCdzyUntVDo5AL3FcMpY5vrudyPJJibn3kgnhc60+1AGAP2/iz
1k4vJbkCAwEAAaNTMFEwHQYDVR0OBBYEFBYrp2tVEBWU3bxFTqLPSQ3kdgrdMB8G
A1UdIwQYMBaAFBYrp2tVEBWU3bxFTqLPSQ3kdgrdMA8GA1UdEwEB/wQFMAMBAf8w
DQYJKoZIhvcNAQELBQADggEBACqLLWomeFK9hMlh6WoiJ29gGvbwSowVSaMsFOcD
qqDfhvMP/53mEquCAFnW9VLVrH8bqwQOkMdlB717rKqX+8Rr4bItylOArA7r45Jf
qRQbA6MI3W0QSs1D2lfq3nnOazCfSsvrMex86VbzhGwUrNgUv+dF32JuvfVl+j2c
rzupXiAdXcrHnOP5fJn+UEmAr8YsKZE8eetugy/EEFnouo751f8jNpFf99kWpdu1
qhHts40nDiapL9mZRcZIBM55qH89Q5SlKGb9To04yS12oeWtkw/dQZlXrOe3xudt
AyiEApAKuHSk56yMbOXEqaznhICXitMKCBNlW2c3emwpBUk=
-----END CERTIFICATE-----
";

/// Writes `content` in a file of the temporary directory named after `name` and
/// returns its path. `name` must be unique across the tests of the crate.
#[cfg(test)]
pub(crate) fn write_test_file(name: &str, content: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("{}-{name}", env!("CARGO_PKG_NAME")));

    fs::write(&path, content).expect("to write the test file");

    path
}

/// Returns the path of a file that does not exist, named after `name`.
#[cfg(test)]
pub(crate) fn missing_test_file(name: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("{}-{name}", env!("CARGO_PKG_NAME")));

    let _ = fs::remove_file(&path);

    path
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use std::path::Path;

    use config::{Config, ConfigError, File, FileFormat};

    use super::{
        CA_BUNDLE, Configuration, Credentials, NamespaceConfiguration, OPERATOR_LISTEN,
        missing_test_file, write_test_file,
    };

    /// Deserializes a toml payload the same way the file sources do, without
    /// reading the environment so the tests stay independent from it.
    fn try_deserialize(content: &str) -> Result<Configuration, ConfigError> {
        Config::builder()
            .set_default("operator.listen", OPERATOR_LISTEN.to_string())
            .expect("to set a default for the listen address")
            .add_source(File::from_str(content, FileFormat::Toml))
            .build()
            .expect("to build the configuration")
            .try_deserialize()
    }

    fn deserialize(content: &str) -> Configuration {
        try_deserialize(content).expect("to deserialize the configuration")
    }

    #[test]
    fn endpoint_is_none_when_the_key_is_absent() {
        let config = deserialize(
            r#"
            [api]
            token = "token"
            secret = "secret"
            "#,
        );

        assert_eq!(config.api.endpoint, None);
        assert_eq!(
            config.api.credentials,
            Credentials::OAuth1 {
                token: "token".into(),
                secret: "secret".into(),
                consumer_key: clevercloud_sdk::DEFAULT_CONSUMER_KEY.into(),
                consumer_secret: clevercloud_sdk::DEFAULT_CONSUMER_SECRET.into(),
            }
        );
    }

    #[test]
    fn endpoint_is_read_from_the_api_section() {
        let config = deserialize(
            r#"
            [api]
            endpoint = "https://api.example.com"
            token = "token"
            secret = "secret"
            consumer-key = "consumer-key"
            consumer-secret = "consumer-secret"
            "#,
        );

        assert_eq!(
            config.api.endpoint.as_deref(),
            Some("https://api.example.com")
        );
        assert_eq!(
            config.api.credentials,
            Credentials::OAuth1 {
                token: "token".into(),
                secret: "secret".into(),
                consumer_key: "consumer-key".into(),
                consumer_secret: "consumer-secret".into(),
            }
        );
    }

    /// The endpoint key must not take part in the untagged deserialization of
    /// the credentials, a lone token still means the oauthless backend.
    #[test]
    fn endpoint_does_not_shadow_the_bearer_credentials() {
        let config = deserialize(
            r#"
            [api]
            endpoint = "https://api-bridge.example.com"
            token = "token"
            "#,
        );

        assert_eq!(
            config.api.credentials,
            Credentials::Bearer {
                token: "token".into()
            }
        );
    }

    /// The sdk joins paths to the endpoint as-is, so what is stored must be the
    /// parsed url: a trailing slash would double the separator, and anything the
    /// parser merely tolerates would reach the request builder untouched.
    #[test]
    fn endpoints_are_normalised_through_the_parsed_url() {
        for (raw, expected) in [
            ("https://api.example.com//", "https://api.example.com"),
            ("https://api.example.com ", "https://api.example.com"),
            (" https://api.example.com/", "https://api.example.com"),
            (
                "https://api.example.com/base/",
                "https://api.example.com/base",
            ),
            ("HTTPS://API.EXAMPLE.COM", "https://api.example.com"),
        ] {
            let config = deserialize(&format!(
                r#"
                [api]
                endpoint = "{raw}"
                token = "token"
                "#
            ));

            assert_eq!(
                config.api.endpoint.as_deref(),
                Some(expected),
                "unexpected normalisation of '{raw}'"
            );
        }
    }

    /// A per-namespace secret overrides the credentials of a namespace, not the
    /// installation they belong to.
    #[test]
    fn namespace_endpoint_is_inherited_unless_overridden() {
        let global = deserialize(
            r#"
            [api]
            endpoint = "https://api.example.com"
            token = "token"
            "#,
        )
        .api;

        let mut api = deserialize(
            r#"
            [api]
            token = "token"
            "#,
        )
        .api;

        api.inherit_from(&global);
        assert_eq!(api.endpoint.as_deref(), Some("https://api.example.com"));

        let mut api = deserialize(
            r#"
            [api]
            endpoint = "https://api.namespace.example.com"
            token = "token"
            "#,
        )
        .api;

        api.inherit_from(&global);
        assert_eq!(
            api.endpoint.as_deref(),
            Some("https://api.namespace.example.com"),
            "an explicit endpoint must win over the inherited one"
        );
    }

    /// The bundle follows the endpoint: it describes the installation, not the
    /// credentials the secret carries.
    #[test]
    fn namespace_ca_bundle_is_inherited_unless_overridden() {
        let global = deserialize(
            r#"
            [api]
            ca_bundle = "/etc/ssl/installation.pem"
            token = "token"
            "#,
        )
        .api;

        let mut api = deserialize(
            r#"
            [api]
            token = "token"
            "#,
        )
        .api;

        api.inherit_from(&global);
        assert_eq!(
            api.ca_bundle.as_deref(),
            Some(Path::new("/etc/ssl/installation.pem"))
        );

        let mut api = deserialize(
            r#"
            [api]
            ca_bundle = "/etc/ssl/namespace.pem"
            token = "token"
            "#,
        )
        .api;

        api.inherit_from(&global);
        assert_eq!(
            api.ca_bundle.as_deref(),
            Some(Path::new("/etc/ssl/namespace.pem")),
            "an explicit bundle must win over the inherited one"
        );
    }

    /// The bundle sits next to the flattened credentials: it must be consumed as
    /// a key of its own and must not take part in their untagged
    /// deserialization, exactly as the endpoint does not.
    #[test]
    fn ca_bundle_does_not_shadow_the_flattened_credentials() {
        let path = Some(Path::new("/etc/ssl/installation.pem"));

        let config = deserialize(
            r#"
            [api]
            ca_bundle = "/etc/ssl/installation.pem"
            token = "token"
            "#,
        );

        assert_eq!(config.api.ca_bundle.as_deref(), path);
        assert_eq!(
            config.api.credentials,
            Credentials::Bearer {
                token: "token".into()
            },
            "a lone token must still mean the oauthless auth backend"
        );

        let config = deserialize(
            r#"
            [api]
            endpoint = "https://api.example.com"
            ca_bundle = "/etc/ssl/installation.pem"
            token = "token"
            secret = "secret"
            "#,
        );

        assert_eq!(config.api.ca_bundle.as_deref(), path);
        assert_eq!(
            config.api.credentials,
            Credentials::OAuth1 {
                token: "token".into(),
                secret: "secret".into(),
                consumer_key: clevercloud_sdk::DEFAULT_CONSUMER_KEY.into(),
                consumer_secret: clevercloud_sdk::DEFAULT_CONSUMER_SECRET.into(),
            },
            "a token and a secret must still mean oauth1"
        );

        // The same content, read as the per-namespace secret is.
        let configuration = NamespaceConfiguration::try_from(
            r#"
            [api]
            ca_bundle = "/etc/ssl/installation.pem"
            token = "token"
            "#,
        )
        .expect("to parse the namespace configuration");

        assert_eq!(configuration.api.ca_bundle.as_deref(), path);
        assert_eq!(
            configuration.api.credentials,
            Credentials::Bearer {
                token: "token".into()
            }
        );
    }

    #[test]
    fn ca_bundle_is_none_when_the_key_is_absent() {
        let config = deserialize(
            r#"
            [api]
            token = "token"
            secret = "secret"
            "#,
        );

        assert_eq!(config.api.ca_bundle, None);
    }

    /// A blank value is not a path, and would otherwise be reported as a missing
    /// file. It is the only thing about the bundle the parser can tell.
    #[test]
    fn a_blank_ca_bundle_is_rejected() {
        assert!(
            try_deserialize(
                r#"
                [api]
                ca_bundle = ""
                token = "token"
                "#,
            )
            .is_err(),
            "an empty bundle path should have been rejected"
        );
    }

    /// A bundle the http client could not build upon must be reported before
    /// anything runs, rather than on every reconciliation with an opaque
    /// handshake error, and the message must name the file at fault.
    ///
    /// This is what `--check` goes through, and what the daemon goes through
    /// when it builds its client.
    #[test]
    fn unusable_ca_bundles_are_rejected_by_the_check() {
        // A file the parser recognises no certificate in comes back empty
        // instead of failing, which would leave the authority untrusted.
        let cases = [
            ("absent", missing_test_file("cfg-absent-ca-bundle.pem")),
            ("empty", write_test_file("cfg-empty-ca-bundle.pem", "")),
            (
                "text",
                write_test_file("cfg-text-ca-bundle.pem", "this is not a certificate\n"),
            ),
            (
                "truncated",
                write_test_file(
                    "cfg-truncated-ca-bundle.pem",
                    "-----BEGIN CERTIFICATE-----\nnot base64 !!!\n-----END CERTIFICATE-----\n",
                ),
            ),
            (
                "key",
                write_test_file(
                    "cfg-key-ca-bundle.pem",
                    "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----\n",
                ),
            ),
        ];

        for (case, path) in cases {
            let config = deserialize(&format!(
                r#"
                [api]
                ca_bundle = "{}"
                token = "token"
                "#,
                path.display()
            ));

            let err = config
                .validate()
                .expect_err(&format!("bundle '{case}' should have been rejected"));

            assert!(
                err.to_string().contains(&path.display().to_string()),
                "the error of '{case}' does not name the file at fault: {err}"
            );
        }
    }

    /// A configuration the parser accepted and whose bundle is readable must
    /// pass the check, and one without a bundle must not need a file at all.
    #[test]
    fn usable_configurations_pass_the_check() {
        let path = write_test_file("cfg-usable-ca-bundle.pem", CA_BUNDLE);

        deserialize(&format!(
            r#"
            [api]
            ca_bundle = "{}"
            token = "token"
            "#,
            path.display()
        ))
        .validate()
        .expect("the configuration to be healthy");

        deserialize(
            r#"
            [api]
            token = "token"
            "#,
        )
        .validate()
        .expect("a configuration without a bundle to be healthy");
    }

    /// Encoding a manifest with `configmap generate` or `secret generate` must
    /// keep working from a machine that does not hold the bundle: the path it
    /// carries is the one of the pod.
    #[test]
    fn a_bundle_the_machine_does_not_hold_still_parses() {
        let config = deserialize(
            r#"
            [api]
            ca_bundle = "/etc/clever-kubernetes-operator-ca/ca-bundle.pem"
            token = "token"
            "#,
        );

        assert_eq!(
            config.api.ca_bundle.as_deref(),
            Some(Path::new(
                "/etc/clever-kubernetes-operator-ca/ca-bundle.pem"
            ))
        );
        assert!(
            toml::to_string(&config).is_ok(),
            "the configuration must still be encodable"
        );
    }

    /// A value the http client could not reach must be rejected at load time,
    /// rather than failing on every reconciliation with an opaque error.
    #[test]
    fn unusable_endpoints_are_rejected() {
        for endpoint in [
            "api.example.com:8080",
            "ftp://api.example.com",
            "not an url",
            "https://api.example.com/?token=leaked",
            "https://api.example.com/#fragment",
            "https://user:password@api.example.com",
        ] {
            let content = format!(
                r#"
                [api]
                endpoint = "{endpoint}"
                token = "token"
                "#
            );

            assert!(
                try_deserialize(&content).is_err(),
                "endpoint '{endpoint}' should have been rejected"
            );
        }
    }

    /// `configmap generate` and `secret generate` encode the configuration back
    /// to toml, both the endpoint and the flattened credentials must survive it.
    #[test]
    fn configuration_round_trips_through_toml() {
        for content in [
            r#"
            [api]
            endpoint = "https://api.example.com"
            token = "token"
            secret = "secret"
            "#
            .to_string(),
            r#"
            [api]
            token = "token"
            "#
            .to_string(),
            r#"
            [api]
            endpoint = "https://api.example.com"
            ca_bundle = "/etc/ssl/installation.pem"
            token = "token"
            secret = "secret"
            "#
            .to_string(),
        ] {
            let content = content.as_str();
            let config = deserialize(content);
            let encoded = toml::to_string(&config).expect("to encode the configuration as toml");

            assert_eq!(deserialize(&encoded), config, "encoded as: {encoded}");
        }
    }

    /// A lone token means the oauthless auth backend, exactly as it does in the
    /// global configuration file.
    #[test]
    fn a_lone_token_is_a_bearer_credential() {
        let configuration = NamespaceConfiguration::try_from(
            r#"
            [api]
            token = "token"
            "#,
        )
        .expect("to parse the namespace configuration");

        assert_eq!(
            configuration.api.credentials,
            Credentials::Bearer {
                token: "token".to_string()
            }
        );
    }

    #[test]
    fn a_token_and_a_secret_are_oauth1_credentials() {
        let configuration = NamespaceConfiguration::try_from(
            r#"
            [api]
            token = "token"
            secret = "secret"
            "#,
        )
        .expect("to parse the namespace configuration");

        assert_eq!(
            configuration.api.credentials,
            Credentials::OAuth1 {
                token: "token".to_string(),
                secret: "secret".to_string(),
                consumer_key: clevercloud_sdk::DEFAULT_CONSUMER_KEY.to_string(),
                consumer_secret: clevercloud_sdk::DEFAULT_CONSUMER_SECRET.to_string(),
            }
        );
    }
}
