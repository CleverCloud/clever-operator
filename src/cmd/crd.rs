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
            config_provider::ConfigProvider, elasticsearch::ElasticSearch, keycloak::Keycloak,
            kv::KV, matomo::Matomo, metabase::Metabase, mongodb::MongoDb, mysql::MySql,
            postgresql::PostgreSql, pulsar::Pulsar, redis::Redis,
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
    KV,
    Metabase,
    Keycloak,
    Matomo,
}

impl FromStr for CustomResource {
    type Err = Box<dyn Error + Send + Sync>;

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgresql" => Ok(Self::PostgreSql),
            "redis" => Ok(Self::Redis),
            "mysql" => Ok(Self::MySql),
            "mongodb" => Ok(Self::MongoDb),
            "pulsar" => Ok(Self::Pulsar),
            "config-provider" => Ok(Self::ConfigProvider),
            "elasticsearch" => Ok(Self::ElasticSearch),
            "kv" => Ok(Self::KV),
            "metabase" => Ok(Self::Metabase),
            "keycloak" => Ok(Self::Keycloak),
            "matomo" => Ok(Self::Matomo),
            _ => Err(format!(
                "failed to parse '{s}', available options are: 'postgresql', 'redis', \
                'mysql', 'mongodb, 'pulsar', 'config-server', 'elasticsearch', 'kv', \
                'metabase', 'keycloak' and 'matomo'"
            )
            .into()),
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
    #[clap(name = "view", aliases = &["v"], about = "View custom resource definition")]
    View {
        #[clap(name = "custom-resource")]
        custom_resource: Option<CustomResource>,
    },
}

#[async_trait]
impl Executor for CustomResourceDefinition {
    type Error = CustomResourceDefinitionError;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(config)))]
    async fn execute(&self, config: Arc<Configuration>) -> Result<(), Self::Error> {
        match self {
            Self::View { custom_resource } => view(config, custom_resource).await,
        }
    }
}

// -----------------------------------------------------------------------------
// view function

#[cfg_attr(feature = "tracing", tracing::instrument(skip(_config)))]
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
            CustomResource::KV => serde_yaml::to_string(&KV::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            CustomResource::Metabase => serde_yaml::to_string(&Metabase::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            CustomResource::Keycloak => serde_yaml::to_string(&Keycloak::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            CustomResource::Matomo => serde_yaml::to_string(&Matomo::crd())
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
            serde_yaml::to_string(&KV::crd()).map_err(CustomResourceDefinitionError::Serialize)?,
            serde_yaml::to_string(&Metabase::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            serde_yaml::to_string(&Keycloak::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
            serde_yaml::to_string(&Matomo::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
        ]
    };

    print!("{}", crds.join("\n---\n"));
    Ok(())
}
