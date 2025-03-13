//! # ConfigMap module
//!
//! This module provides custom resource module command line interface function
//! implementation

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use clap::Subcommand;
use k8s_openapi::{ByteString, api::core::v1};

use crate::{cmd::Executor, svc::cfg::Configuration};

// -----------------------------------------------------------------------------
// Secret enum

#[derive(thiserror::Error, Debug)]
pub enum SecretError {
    #[error("failed to serialize configmap, {0}")]
    Serialize(serde_yaml::Error),
    #[error("failed to encode configuration, {0}")]
    Encode(toml::ser::Error),
}

// -----------------------------------------------------------------------------
// Secret enum

#[derive(Subcommand, Clone, Debug)]
pub enum Secret {
    #[clap(name = "generate", aliases = &["g"], about = "Generate configmap from clever-operator configuration")]
    Generate {
        #[clap(short = 'n', long = "name", help = "Name of the configmap")]
        name: Option<String>,
        #[clap(short = 'N', long = "namespace", help = "Namespace of the configmap")]
        namespace: Option<String>,
    },
}

#[async_trait]
impl Executor for Secret {
    type Error = SecretError;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(config)))]
    async fn execute(&self, config: Arc<Configuration>) -> Result<(), Self::Error> {
        match self {
            Self::Generate { namespace, name } => {
                generate(config, namespace.to_owned(), name.to_owned()).await
            }
        }
    }
}

// -----------------------------------------------------------------------------
// generate function

#[cfg_attr(feature = "tracing", tracing::instrument(skip(config)))]
pub async fn generate(
    config: Arc<Configuration>,
    namespace: Option<String>,
    name: Option<String>,
) -> Result<(), SecretError> {
    let mut secret = v1::Secret::default();
    let configuration = toml::to_string(&*config).map_err(SecretError::Encode)?;

    secret.metadata.name = name;
    secret.metadata.namespace = namespace;

    secret.data = Some(BTreeMap::from([(
        "config.toml".to_string(),
        ByteString(configuration.into_bytes()),
    )]));

    println!(
        "{}",
        serde_yaml::to_string(&secret).map_err(SecretError::Serialize)?
    );
    Ok(())
}
