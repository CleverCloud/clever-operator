//! # Configuration module
//!
//! This module provides utilities and helpers to interact with the configuration

use std::{
    convert::TryFrom,
    env::{self, VarError},
    net::SocketAddr,
    path::PathBuf,
};

use clevercloud_sdk::{oauth10a::Credentials, PUBLIC_ENDPOINT};
use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use tracing::warn;

// -----------------------------------------------------------------------------
// Constants

pub const OPERATOR_LISTEN: &str = "0.0.0.0:8000";

// -----------------------------------------------------------------------------
// Operator structure

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Operator {
    #[serde(rename = "listen")]
    pub listen: SocketAddr,
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
}

// -----------------------------------------------------------------------------
// NamespaceConfiguration structures

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct NamespaceConfiguration {
    #[serde(rename = "api")]
    pub api: Credentials,
}

impl TryFrom<PathBuf> for NamespaceConfiguration {
    type Error = Error;

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Config::builder()
            // -----------------------------------------------------------------
            // Api
            .set_default(
                "api.endpoint",
                env::var("CLEVER_OPERATOR_API_ENDPOINT")
                    .unwrap_or_else(|_err| PUBLIC_ENDPOINT.to_string()),
            )
            .map_err(|err| Error::Default("api.endpoint".into(), err))?
            .set_default(
                "api.token",
                env::var("CLEVER_OPERATOR_API_TOKEN").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.token".into(), err))?
            .set_default(
                "api.secret",
                env::var("CLEVER_OPERATOR_API_SECRET").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.secret".into(), err))?
            .set_default(
                "api.consumerKey",
                env::var("CLEVER_OPERATOR_API_CONSUMER_KEY").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.consumerKey".into(), err))?
            .set_default(
                "api.consumerSecret",
                env::var("CLEVER_OPERATOR_API_CONSUMER_SECRET")
                    .unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.consumerSecret".into(), err))?
            // -----------------------------------------------------------------
            // Files
            .add_source(File::from(path).required(true))
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
    pub api: Credentials,
    #[serde(rename = "operator")]
    pub operator: Operator,
}

impl TryFrom<PathBuf> for Configuration {
    type Error = Error;

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Config::builder()
            // -----------------------------------------------------------------
            // Api
            .set_default(
                "api.endpoint",
                env::var("CLEVER_OPERATOR_API_ENDPOINT")
                    .unwrap_or_else(|_err| PUBLIC_ENDPOINT.to_string()),
            )
            .map_err(|err| Error::Default("api.endpoint".into(), err))?
            .set_default(
                "api.token",
                env::var("CLEVER_OPERATOR_API_TOKEN").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.token".into(), err))?
            .set_default(
                "api.secret",
                env::var("CLEVER_OPERATOR_API_SECRET").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.secret".into(), err))?
            .set_default(
                "api.consumerKey",
                env::var("CLEVER_OPERATOR_API_CONSUMER_KEY").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.consumerKey".into(), err))?
            .set_default(
                "api.consumerSecret",
                env::var("CLEVER_OPERATOR_API_CONSUMER_SECRET")
                    .unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.consumerSecret".into(), err))?
            // -----------------------------------------------------------------
            // Operator
            .set_default(
                "operator.listen",
                env::var("CLEVER_OPERATOR_OPERATOR_LISTEN")
                    .unwrap_or_else(|_err| OPERATOR_LISTEN.to_string()),
            )
            .map_err(|err| Error::Default("operator.listen".into(), err))?
            // -----------------------------------------------------------------
            // Sentry
            .set_default(
                "sentry.dsn",
                env::var("CLEVER_OPERATOR_SENTRY_DSN")
                    .map(Some)
                    .unwrap_or_else(|_err| None),
            )
            .map_err(|err| Error::Default("sentry.dsn".into(), err))?
            // -----------------------------------------------------------------
            // Jaeger
            .set_default(
                "jaeger.endpoint",
                env::var("CLEVER_OPERATOR_JAEGER_ENDPOINT").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("jaeger.endpoint".into(), err))?
            .set_default(
                "jaeger.user",
                env::var("CLEVER_OPERATOR_JAEGER_USER")
                    .map(Some)
                    .unwrap_or_else(|_err| None),
            )
            .map_err(|err| Error::Default("jaeger.user".into(), err))?
            .set_default(
                "jaeger.password",
                env::var("CLEVER_OPERATOR_JAEGER_PASSWORD")
                    .map(Some)
                    .unwrap_or_else(|_err| None),
            )
            .map_err(|err| Error::Default("jaeger.password".into(), err))?
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
    pub fn try_default() -> Result<Self, Error> {
        Config::builder()
            // -----------------------------------------------------------------
            // Api
            .set_default(
                "api.endpoint",
                env::var("CLEVER_OPERATOR_API_ENDPOINT")
                    .unwrap_or_else(|_err| PUBLIC_ENDPOINT.to_string()),
            )
            .map_err(|err| Error::Default("api.endpoint".into(), err))?
            .set_default(
                "api.token",
                env::var("CLEVER_OPERATOR_API_TOKEN").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.token".into(), err))?
            .set_default(
                "api.secret",
                env::var("CLEVER_OPERATOR_API_SECRET").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.secret".into(), err))?
            .set_default(
                "api.consumerKey",
                env::var("CLEVER_OPERATOR_API_CONSUMER_KEY").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.consumerKey".into(), err))?
            .set_default(
                "api.consumerSecret",
                env::var("CLEVER_OPERATOR_API_CONSUMER_SECRET")
                    .unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("api.consumerSecret".into(), err))?
            // -----------------------------------------------------------------
            // Operator
            .set_default(
                "operator.listen",
                env::var("CLEVER_OPERATOR_OPERATOR_LISTEN")
                    .unwrap_or_else(|_err| OPERATOR_LISTEN.to_string()),
            )
            .map_err(|err| Error::Default("operator.listen".into(), err))?
            // -----------------------------------------------------------------
            // Sentry
            .set_default(
                "sentry.dsn",
                env::var("CLEVER_OPERATOR_SENTRY_DSN")
                    .map(Some)
                    .unwrap_or_else(|_err| None),
            )
            .map_err(|err| Error::Default("sentry.dsn".into(), err))?
            // -----------------------------------------------------------------
            // Jaeger
            .set_default(
                "jaeger.endpoint",
                env::var("CLEVER_OPERATOR_JAEGER_ENDPOINT").unwrap_or_else(|_err| "".to_string()),
            )
            .map_err(|err| Error::Default("jaeger.endpoint".into(), err))?
            .set_default(
                "jaeger.user",
                env::var("CLEVER_OPERATOR_JAEGER_USER")
                    .map(Some)
                    .unwrap_or_else(|_err| None),
            )
            .map_err(|err| Error::Default("jaeger.user".into(), err))?
            .set_default(
                "jaeger.password",
                env::var("CLEVER_OPERATOR_JAEGER_PASSWORD")
                    .map(Some)
                    .unwrap_or_else(|_err| None),
            )
            .map_err(|err| Error::Default("jaeger.password".into(), err))?
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

        match &self.api {
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
