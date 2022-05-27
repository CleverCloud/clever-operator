//! # Configuration module
//!
//! This module provide utilities and helpers to interact with the configuration

use std::{
    convert::TryFrom,
    env::{self, VarError},
    path::PathBuf,
};

use clevercloud_sdk::{oauth10a::Credentials, PUBLIC_ENDPOINT};
use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// -----------------------------------------------------------------------------
// Constants

pub const OPERATOR_LISTEN: &str = "0.0.0.0:8000";

// -----------------------------------------------------------------------------
// Proxy structure

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Proxy {
    #[serde(rename = "http")]
    pub http: Option<String>,
    #[serde(rename = "https")]
    pub https: Option<String>,
    #[serde(rename = "no", default = "Default::default")]
    pub no: Vec<String>,
}

// -----------------------------------------------------------------------------
// Operator structure

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Operator {
    #[serde(rename = "listen")]
    pub listen: String,
}

// -----------------------------------------------------------------------------
// Api structure

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Api {
    #[serde(rename = "endpoint")]
    pub endpoint: String,
    #[serde(rename = "token")]
    pub token: String,
    #[serde(rename = "secret")]
    pub secret: String,
    #[serde(rename = "consumerKey")]
    pub consumer_key: String,
    #[serde(rename = "consumerSecret")]
    pub consumer_secret: String,
}

#[allow(clippy::from_over_into)]
impl Into<Credentials> for Api {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn into(self) -> Credentials {
        Credentials {
            token: self.token.to_owned(),
            secret: self.secret.to_owned(),
            consumer_key: self.consumer_key.to_owned(),
            consumer_secret: self.consumer_secret,
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
}

// -----------------------------------------------------------------------------
// Sentry structure

#[cfg(feature = "tracker")]
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct Sentry {
    #[serde(rename = "dsn")]
    pub dsn: Option<String>,
}

// -----------------------------------------------------------------------------
// Jaeger structure

#[cfg(feature = "trace")]
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct Jaeger {
    pub endpoint: String,
    pub user: Option<String>,
    pub password: Option<String>,
}

// -----------------------------------------------------------------------------
// Configuration structures

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Configuration {
    #[serde(rename = "proxy")]
    pub proxy: Option<Proxy>,
    #[serde(rename = "api")]
    pub api: Api,
    #[serde(rename = "operator")]
    pub operator: Operator,
    #[cfg(feature = "tracker")]
    #[serde(rename = "sentry", default = "Default::default")]
    pub sentry: Sentry,
    #[cfg(feature = "trace")]
    #[serde(rename = "jaeger")]
    pub jaeger: Option<Jaeger>,
}

impl TryFrom<PathBuf> for Configuration {
    type Error = Error;

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Config::builder()
            .set_default("api.endpoint", PUBLIC_ENDPOINT)
            .map_err(|err| Error::Default("api.endpoint".into(), err))?
            .set_default("api.token", "")
            .map_err(|err| Error::Default("api.token".into(), err))?
            .set_default("api.secret", "")
            .map_err(|err| Error::Default("api.secret".into(), err))?
            .set_default("api.consumerKey", "")
            .map_err(|err| Error::Default("api.consumerKey".into(), err))?
            .set_default("api.consumerSecret", "")
            .map_err(|err| Error::Default("api.consumerSecret".into(), err))?
            .set_default("operator.listen", OPERATOR_LISTEN)
            .map_err(|err| Error::Default("operator.listen".into(), err))?
            .add_source(Environment::with_prefix(
                &env!("CARGO_PKG_NAME").replace('-', "_"),
            ))
            .add_source(File::from(path).required(true))
            .build()
            .map_err(Error::Build)?
            .try_deserialize()
            .map_err(Error::Deserialize)
    }
}

impl Configuration {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    pub fn try_default() -> Result<Self, Error> {
        Config::builder()
            .set_default("api.endpoint", PUBLIC_ENDPOINT)
            .map_err(|err| Error::Default("api.endpoint".into(), err))?
            .set_default("api.token", "")
            .map_err(|err| Error::Default("api.token".into(), err))?
            .set_default("api.secret", "")
            .map_err(|err| Error::Default("api.secret".into(), err))?
            .set_default("api.consumerKey", "")
            .map_err(|err| Error::Default("api.consumerKey".into(), err))?
            .set_default("api.consumerSecret", "")
            .map_err(|err| Error::Default("api.consumerSecret".into(), err))?
            .set_default("operator.listen", OPERATOR_LISTEN)
            .map_err(|err| Error::Default("operator.listen".into(), err))?
            .add_source(Environment::with_prefix(
                &env!("CARGO_PKG_NAME").replace('-', "_"),
            ))
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

    /// Prints a message about missing value for configuration key
    #[cfg_attr(feature = "trace", tracing::instrument)]
    pub fn help(&self) {
        #[cfg(feature = "logging")]
        info!("Build with 'logging' feature flag");

        #[cfg(feature = "metrics")]
        info!("Build with 'metrics' feature flag");

        #[cfg(feature = "trace")]
        info!("Build with 'trace' feature flag");

        #[cfg(feature = "tracker")]
        info!("Build with 'tracker' feature flag");

        if self.api.consumer_key.is_empty() {
            warn!("Configuration key 'api.consumerKey' has an empty value");
        }

        if self.api.consumer_secret.is_empty() {
            warn!("Configuration key 'api.consumerSecret' has an empty value");
        }

        if self.api.token.is_empty() {
            warn!("Configuration key 'api.token' has an empty value");
        }

        if self.api.secret.is_empty() {
            warn!("Configuration key 'api.secret' has an empty value");
        }
    }
}
