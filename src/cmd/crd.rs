//! # Custom resource definition module
//!
//! This module provides custom resource module command line interface function
//! implementation

use std::{error::Error, str::FromStr, sync::Arc};

use async_trait::async_trait;
use kube::CustomResourceExt;
use structopt::StructOpt;

use crate::{
    cmd::Executor,
    svc::{cfg::Configuration, crd::postgresql::PostgreSql},
};

// -----------------------------------------------------------------------------
// CustomResource enum

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum CustomResource {
    PostgreSql,
}

impl FromStr for CustomResource {
    type Err = Box<dyn Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgresql" => Ok(Self::PostgreSql),
            _ => Err(format!("failed to parse '{}', available option is postgresql", s).into()),
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

#[derive(StructOpt, Clone, Debug)]
pub enum CustomResourceDefinition {
    /// View custom resource definition
    #[structopt(name = "view", aliases = &["v"])]
    View {
        #[structopt(name = "custom-resource")]
        custom_resource: Option<CustomResource>,
    },
}

#[async_trait]
impl Executor for CustomResourceDefinition {
    type Error = CustomResourceDefinitionError;

    async fn execute(&self, config: Arc<Configuration>) -> Result<(), Self::Error> {
        match self {
            Self::View { custom_resource } => view(config, custom_resource).await,
        }
    }
}

// -----------------------------------------------------------------------------
// view function

pub async fn view(
    _config: Arc<Configuration>,
    custom_resource: &Option<CustomResource>,
) -> Result<(), CustomResourceDefinitionError> {
    let crds = if let Some(cr) = custom_resource {
        vec![match cr {
            CustomResource::PostgreSql => serde_yaml::to_string(&PostgreSql::crd())
                .map_err(CustomResourceDefinitionError::Serialize)?,
        }]
    } else {
        vec![serde_yaml::to_string(&PostgreSql::crd())
            .map_err(CustomResourceDefinitionError::Serialize)?]
    };

    print!("{}", crds.join("\n"));
    Ok(())
}
