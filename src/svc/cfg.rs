//! # Configuration module
//!
//! This module provide utilities and helpers to interact with the configuration

use std::{convert::TryFrom, path::PathBuf};

use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Api {
    #[serde(rename = "token")]
    pub token: String,
    #[serde(rename = "secret")]
    pub secret: String,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct CleverCloud {
    #[serde(rename = "api")]
    pub api: Api,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Configuration {
    #[serde(rename = "clever-cloud")]
    pub clever_cloud: CleverCloud,
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigurationError {
    #[error("failed to load file '{0:?}', {1}")]
    File(PathBuf, ConfigError),
    #[error("failed to load configuration, {0}")]
    Cast(ConfigError),
}

impl TryFrom<PathBuf> for Configuration {
    type Error = ConfigurationError;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let mut config = Config::default();

        config
            .merge(File::from(path.to_owned()).required(true))
            .map_err(|err| ConfigurationError::File(path, err))?;

        Ok(config.try_into().map_err(ConfigurationError::Cast)?)
    }
}

impl Configuration {
    pub fn try_default() -> Result<Self, ConfigurationError> {
        let mut config = Config::default();

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

        Ok(config.try_into().map_err(ConfigurationError::Cast)?)
    }
}
