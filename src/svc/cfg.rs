//! # Configuration module
//!
//! This module provide utilities and helpers to interact with the configuration

use std::{convert::TryFrom, path::PathBuf};

use clevercloud_sdk::{oauth10a::Credentials, PUBLIC_ENDPOINT};
use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Constants

pub const OPERATOR_LISTEN: &str = "0.0.0.0:7080";

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
pub enum ConfigurationError {
    #[error("failed to load file '{0:?}', {1}")]
    File(PathBuf, ConfigError),
    #[error("failed to load configuration, {0}")]
    Cast(ConfigError),
    #[error("failed to set default for key '{0}', {1}")]
    Default(String, ConfigError),
    #[error("failed to set environment source, {0}")]
    Environment(ConfigError),
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
    type Error = ConfigurationError;

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let mut config = Config::default();

        config
            .set_default("api.endpoint", PUBLIC_ENDPOINT)
            .map_err(|err| ConfigurationError::Default("api.endpoint".into(), err))?;

        config
            .set_default("api.token", "")
            .map_err(|err| ConfigurationError::Default("api.token".into(), err))?;

        config
            .set_default("api.secret", "")
            .map_err(|err| ConfigurationError::Default("api.secret".into(), err))?;

        config
            .set_default("api.consumerKey", "")
            .map_err(|err| ConfigurationError::Default("api.consumerKey".into(), err))?;

        config
            .set_default("api.consumerSecret", "")
            .map_err(|err| ConfigurationError::Default("api.consumerSecret".into(), err))?;

        config
            .set_default("operator.listen", OPERATOR_LISTEN)
            .map_err(|err| ConfigurationError::Default("operator.listen".into(), err))?;

        config
            .merge(Environment::with_prefix(
                &env!("CARGO_PKG_NAME").replace("-", "_"),
            ))
            .map_err(ConfigurationError::Environment)?;

        config
            .merge(File::from(path.to_owned()).required(true))
            .map_err(|err| ConfigurationError::File(path, err))?;

        config.try_into().map_err(ConfigurationError::Cast)
    }
}

impl Configuration {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    pub fn try_default() -> Result<Self, ConfigurationError> {
        let mut config = Config::default();

        config
            .set_default("api.endpoint", PUBLIC_ENDPOINT)
            .map_err(|err| ConfigurationError::Default("api.endpoint".into(), err))?;

        config
            .set_default("operator.listen", OPERATOR_LISTEN)
            .map_err(|err| ConfigurationError::Default("operator.listen".into(), err))?;

        config
            .merge(Environment::with_prefix(
                &env!("CARGO_PKG_NAME").replace("-", "_"),
            ))
            .map_err(ConfigurationError::Environment)?;

        let path = PathBuf::from(format!("/usr/share/{}/config", env!("CARGO_PKG_NAME")));
        config
            .merge(File::from(path.to_owned()).required(false))
            .map_err(|err| ConfigurationError::File(path, err))?;

        let path = PathBuf::from(format!("/etc/{}/config", env!("CARGO_PKG_NAME")));
        config
            .merge(File::from(path.to_owned()).required(false))
            .map_err(|err| ConfigurationError::File(path, err))?;

        let path = PathBuf::from(format!(
            "{}/.config/{}/config",
            env!("HOME"),
            env!("CARGO_PKG_NAME")
        ));
        config
            .merge(File::from(path.to_owned()).required(false))
            .map_err(|err| ConfigurationError::File(path, err))?;

        let path = PathBuf::from(format!(
            "{}/.local/share/{}/config",
            env!("HOME"),
            env!("CARGO_PKG_NAME")
        ));
        config
            .merge(File::from(path.to_owned()).required(false))
            .map_err(|err| ConfigurationError::File(path, err))?;

        let path = PathBuf::from("config");
        config
            .merge(File::from(path.to_owned()).required(false))
            .map_err(|err| ConfigurationError::File(path, err))?;

        config.try_into().map_err(ConfigurationError::Cast)
    }
}
