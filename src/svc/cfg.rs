//! # Configuration module
//!
//! This module provide utilities and helpers to interact with the configuration

use std::{convert::TryFrom, path::PathBuf};

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Constants

pub const PUBLIC_ENDPOINT: &str = "https://api.clever-cloud.com";

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
// Configuration structures

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Configuration {
    #[serde(rename = "api")]
    pub api: Api,
}

impl TryFrom<PathBuf> for Configuration {
    type Error = ConfigurationError;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let mut config = Config::default();

        config
            .set_default("api.endpoint", PUBLIC_ENDPOINT)
            .map_err(|err| ConfigurationError::Default("api.endpoint".into(), err))?;

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
    pub fn try_default() -> Result<Self, ConfigurationError> {
        let mut config = Config::default();

        config
            .set_default("api.endpoint", PUBLIC_ENDPOINT)
            .map_err(|err| ConfigurationError::Default("api.endpoint".into(), err))?;

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
