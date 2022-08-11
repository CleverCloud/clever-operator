//! # Custom resource definition module
//!
//! This module provides custom resource module command line interface function
//! implementation

use std::{error::Error, str::FromStr, sync::Arc};

use async_trait::async_trait;
use clap::Subcommand;
use kube::CustomResourceExt;

use crate::{
    cmd::Executor,
    svc::{
        cfg::Configuration,
        crd::{
            config_provider::ConfigProvider, elasticsearch::ElasticSearch, mongodb::MongoDb,
            mysql::MySql, postgresql::PostgreSql, pulsar::Pulsar, redis::Redis,
        },
    },
};

// -----------------------------------------------------------------------------
// CustomResource enum

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum CustomResource {
    PostgreSql,
    Redis,
    MySql,
    MongoDb,
    Pulsar,
    ConfigProvider,
    ElasticSearch,
}

impl FromStr for CustomResource {
    type Err = Box<dyn Error + Send + Sync>;

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgresql" => Ok(Self::PostgreSql),
            "redis" => Ok(Self::Redis),
            "mysql" => Ok(Self::MySql),
            "mongodb" => Ok(Self::MongoDb),
            "pulsar" => Ok(Self::Pulsar),
            "config-provider" => Ok(Self::ConfigProvider),
            "elasticsearch" => Ok(Self::ElasticSearch),
            _ => Err(format!("failed to parse '{}', available options are 'elasticsearch', 'config-provider', 'pulsar', 'postgresql', 'redis', 'mysql' or 'mongodb", s).into()),
        }
    }
}

// -----------------------------------------------------------------------------
// CustomResourceDefinitionError enum

#[derive(thiserror::Error, Debug)]
pub enum CustomResourceDefinitionError {
    #[error("failed to serialize custom resource definition, {0}")]
    Serialize(serde_yaml::Error),
}

// -----------------------------------------------------------------------------
// CustomResourceDefinition enum

#[derive(Subcommand, Clone, Debug)]
pub enum CustomResourceDefinition {
    /// View custom resource definition
    #[clap(name = "view", aliases = &["v"])]
    View {
        #[clap(name = "custom-resource")]
        custom_resource: Option<CustomResource>,
    },
}

#[async_trait]
impl Executor for CustomResourceDefinition {
    type Error = CustomResourceDefinitionError;

    #[cfg_attr(feature = "trace", tracing::instrument(skip(config)))]
    async fn execute(&self, config: Arc<Configuration>) -> Result<(), Self::Error> {
        match self {
            Self::View { custom_resource } => view(config, custom_resource).await,
        }
    }
}

// -----------------------------------------------------------------------------
// view function

#[cfg_attr(feature = "trace", tracing::instrument(skip(_config)))]
pub async fn view(
    _config: Arc<Configuration>,
    custom_resource: &Option<CustomResource>,
) -> Result<(), CustomResourceDefinitionError> {
    let crds = if let Some(cr) = custom_resource {
        vec![match cr {
            CustomResource::PostgreSql => serde_yaml::to_string(&PostgreSql::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            CustomResource::Redis => serde_yaml::to_string(&Redis::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            CustomResource::MySql => serde_yaml::to_string(&MySql::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            CustomResource::MongoDb => serde_yaml::to_string(&MongoDb::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            CustomResource::Pulsar => serde_yaml::to_string(&Pulsar::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            CustomResource::ConfigProvider => serde_yaml::to_string(&ConfigProvider::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            CustomResource::ElasticSearch => serde_yaml::to_string(&ElasticSearch::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
        }]
    } else {
        vec![
            serde_yaml::to_string(&PostgreSql::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            serde_yaml::to_string(&Redis::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            serde_yaml::to_string(&MySql::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            serde_yaml::to_string(&MongoDb::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            serde_yaml::to_string(&Pulsar::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            serde_yaml::to_string(&ConfigProvider::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            serde_yaml::to_string(&ElasticSearch::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
        ]
    };

    print!("{}", crds.join(""));
    Ok(())
}
