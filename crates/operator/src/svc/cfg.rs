//! # Configuration module
//!
//! This module provides utilities and helpers to interact with the configuration

use std::{
    convert::TryFrom,
    env::{self, VarError},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use clevercloud_sdk::{Credentials, oauth10a::url::Url};
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
    /// Credentials used to sign the requests, the variant is picked from the
    /// keys that are present, see [`Credentials`].
    #[serde(flatten)]
    pub credentials: Credentials,
}

impl Api {
    /// Falls back on `endpoint` when this section does not declare one.
    ///
    /// A per-namespace secret overrides the credentials of a namespace, not the
    /// installation they belong to: without this it would silently target the
    /// public api while the operator talks to another one.
    pub fn inherit_endpoint(&mut self, endpoint: Option<&str>) {
        if self.endpoint.is_none() {
            self.endpoint = endpoint.map(ToOwned::to_owned);
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
        // an absent endpoint means "inherit the global one", see
        // [`Api::inherit_endpoint`].
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

        // The clever-tools file only carries credentials, the endpoint can
        // solely come from the environment here.
        let endpoint = match endpoint_from_env() {
            Some(value) => Some(
                normalise_endpoint(&value)
                    .map_err(|err| Error::InvalidValue("api.endpoint", err))?,
            ),
            None => None,
        };

        Ok(Self {
            api: Api {
                endpoint,
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
// Tests

#[cfg(test)]
mod tests {
    use config::{Config, ConfigError, File, FileFormat};

    use super::{Configuration, Credentials, NamespaceConfiguration, OPERATOR_LISTEN};

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
        let mut api = deserialize(
            r#"
            [api]
            token = "token"
            "#,
        )
        .api;

        api.inherit_endpoint(Some("https://api.example.com"));
        assert_eq!(api.endpoint.as_deref(), Some("https://api.example.com"));

        let mut api = deserialize(
            r#"
            [api]
            endpoint = "https://api.namespace.example.com"
            token = "token"
            "#,
        )
        .api;

        api.inherit_endpoint(Some("https://api.example.com"));
        assert_eq!(
            api.endpoint.as_deref(),
            Some("https://api.namespace.example.com"),
            "an explicit endpoint must win over the inherited one"
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
            "#,
            r#"
            [api]
            token = "token"
            "#,
        ] {
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
