//! # Client module
//!
//! This module provides helpers to create a clever-cloud client

use clevercloud_sdk::oauth10a::reqwest;
use k8s_openapi::api::core::v1::Secret;

use std::string::FromUtf8Error;

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
    #[error("failed to read key '{0}' of secret '{1}/{2}' as utf-8, {3}")]
    Utf8(&'static str, String, String, FromUtf8Error),
    #[error("failed to parse configuration file, {0}")]
    Configuration(Box<cfg::Error>),
}

impl From<cfg::Error> for Error {
    fn from(err: cfg::Error) -> Self {
        Self::Configuration(Box::new(err))
    }
}

// -----------------------------------------------------------------------------
// helpers

#[cfg_attr(feature = "tracing", tracing::instrument(skip(secret)))]
pub async fn try_from(secret: Secret) -> Result<Client, Error> {
    let (namespace, name) = resource::namespaced_name(&secret);

    let data = match &secret.data {
        Some(data) => data,
        None => {
            return Err(Error::SecretData(namespace, name));
        }
    };

    let Some(bytestr) = data.get("config") else {
        return Err(Error::SecretKey("config", namespace, name));
    };

    // The api server is the one doing the base64 round trip: `ByteString` already
    // holds the content of the key.
    let content = String::from_utf8(bytestr.0.to_owned())
        .map_err(|err| Error::Utf8("config", namespace, name, err))?;

    let configuration = NamespaceConfiguration::try_from(content.as_str())?;

    Ok(Client::from(configuration.api))
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use k8s_openapi::{ByteString, api::core::v1::Secret};

    use super::try_from;

    /// The content of the key is what the api server decoded, building the
    /// client must not expect a second layer of encoding.
    #[tokio::test]
    async fn secret_content_is_read_as_the_api_server_hands_it_over() {
        let mut secret = Secret::default();

        secret.metadata.namespace = Some("namespace".to_string());
        secret.metadata.name = Some("clever-kubernetes-operator".to_string());
        secret.data = Some(BTreeMap::from([(
            "config".to_string(),
            ByteString(b"[api]\ntoken = \"token\"\n".to_vec()),
        )]));

        try_from(secret)
            .await
            .expect("to build a client from the secret");
    }
}
