//! # Client module
//!
//! This module provide an helper to create a kubernetes client

use std::{convert::TryFrom, path::PathBuf};

use kube::{
    config::{KubeConfigOptions, Kubeconfig},
    Config,
};

/// returns a new kubernetes client from the given path if defined
/// or retrieve it from environment or defaults paths
pub async fn try_new(path: Option<PathBuf>) -> Result<kube::Client, kube::Error> {
    let kubeconfig = match path {
        None => Kubeconfig::read()?,
        Some(path) => Kubeconfig::read_from(path)?,
    };

    let opts = KubeConfigOptions::default();
    let config = Config::from_custom_kubeconfig(kubeconfig, &opts).await?;

    Ok(kube::Client::try_from(config)?)
}
