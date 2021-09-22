//! # Custom resource definition module
//!
//! This module provides custom resource module command line interface function
//! implementation

use std::sync::Arc;

use kube::CustomResourceExt;

use crate::{svc::cfg::Configuration, svc::k8s::addon::postgresql::PostgreSql};

// -----------------------------------------------------------------------------
// CustomResourceDefinitionError enum

#[derive(thiserror::Error, Debug)]
pub enum CustomResourceDefinitionError {
    #[error("failed to serialize custom resource definition, {0}")]
    Serialize(serde_yaml::Error),
}

// -----------------------------------------------------------------------------
// view function

pub async fn view(_config: Arc<Configuration>) -> Result<(), CustomResourceDefinitionError> {
    let crds = vec![serde_yaml::to_string(&PostgreSql::crd())
        .map_err(CustomResourceDefinitionError::Serialize)?];

    print!("{}", crds.join("\n"));
    Ok(())
}
