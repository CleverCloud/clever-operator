//! # Client module
//!
//! This module provides helpers to create a clever-cloud client

use base64::{Engine, engine::general_purpose::STANDARD as BASE64_ENGINE};
use clevercloud_sdk::oauth10a::reqwest;
use k8s_openapi::api::core::v1::Secret;
use tempfile::NamedTempFile;
use tokio::{fs::File, io::AsyncWriteExt, task::spawn_blocking as blocking};

use crate::svc::{
    cfg::{self, NamespaceConfiguration},
    k8s::resource,
};

// -----------------------------------------------------------------------------
// types

pub type Client = clevercloud_sdk::Client;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to create clever cloud client, {0}")]
    CreateCleverClient(reqwest::Error),
    #[error("failed to retrieve data from secret '{0}/{1}'")]
    SecretData(String, String),
    #[error("failed to find key '{0}' in secret '{1}/{2}")]
    SecretKey(&'static str, String, String),
    #[error("failed to decode configuration from key '{0}' in secret '{1}/{2}', {3}")]
    Base64Decode(&'static str, String, String, base64::DecodeError),
    #[error("failed to spawn blocking task, {0}")]
    Join(tokio::task::JoinError),
    #[error("failed to write configuration in temporary file, {0}")]
    Io(std::io::Error),
    #[error("failed to parse configuration file, {0}")]
    Configuration(cfg::Error),
}

impl From<tokio::task::JoinError> for Error {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: tokio::task::JoinError) -> Self {
        Self::Join(err)
    }
}

impl From<std::io::Error> for Error {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<cfg::Error> for Error {
    fn from(err: cfg::Error) -> Self {
        Self::Configuration(err)
    }
}

// -----------------------------------------------------------------------------
// helpers

#[cfg_attr(feature = "tracing", tracing::instrument)]
pub async fn try_from(secret: Secret) -> Result<Client, Error> {
    let buf = blocking(move || {
        let (namespace, name) = resource::namespaced_name(&secret);
        let data = match &secret.data {
            Some(data) => data,
            None => {
                return Err(Error::SecretData(namespace, name));
            }
        };

        match data.get("config") {
            Some(bytestr) => BASE64_ENGINE
                .decode(&bytestr.0)
                .map_err(|err| Error::Base64Decode("config", namespace, name, err)),
            None => Err(Error::SecretKey("config", namespace, name)),
        }
    })
    .await??;

    // The file will be automatically deleted when it is dropped
    // See:
    // - https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html
    let named_file = NamedTempFile::new()?;
    let path = named_file.path().to_path_buf();
    let mut file = File::from(named_file.into_file());

    file.write_all(&buf).await?;
    file.sync_all().await?;

    let configuration = NamespaceConfiguration::try_from(path)?;

    Ok(Client::from(configuration.api))
}
