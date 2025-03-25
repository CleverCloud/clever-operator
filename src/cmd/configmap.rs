//! # ConfigMap module
//!
//! This module provides custom resource module command line interface function
//! implementation

use std::{collections::BTreeMap, sync::Arc};

use clap::Subcommand;
use k8s_openapi::api::core::v1;

use crate::{cmd::Executor, svc::cfg::Configuration};

// -----------------------------------------------------------------------------
// ConfigMapError enum

#[derive(thiserror::Error, Debug)]
pub enum ConfigMapError {
    #[error("failed to serialize configmap, {0}")]
    Serialize(serde_yaml::Error),
    #[error("failed to encode configuration, {0}")]
    Encode(toml::ser::Error),
}

// -----------------------------------------------------------------------------
// ConfigMap enum

#[derive(Subcommand, Clone, Debug)]
pub enum ConfigMap {
    #[clap(name = "generate", aliases = &["g"], about = "Generate configmap from clever-operator configuration")]
    Generate {
        #[clap(short = 'n', long = "name", help = "Name of the configmap")]
        name: Option<String>,
        #[clap(short = 'N', long = "namespace", help = "Namespace of the configmap")]
        namespace: Option<String>,
    },
}

impl Executor for ConfigMap {
    type Error = ConfigMapError;

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
) -> Result<(), ConfigMapError> {
    let mut configmap = v1::ConfigMap::default();
    let configuration = toml::to_string(&*config).map_err(ConfigMapError::Encode)?;

    configmap.metadata.name = name;
    configmap.metadata.namespace = namespace;

    configmap.data = Some(BTreeMap::from([("config.toml".to_string(), configuration)]));

    println!(
        "{}",
        serde_yaml::to_string(&configmap).map_err(ConfigMapError::Serialize)?
    );
    Ok(())
}
