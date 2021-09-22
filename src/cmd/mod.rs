//! # Command module
//!
//! This module provide command line interface structures and helpers
use std::{io, path::PathBuf, process::abort, sync::Arc};

use async_trait::async_trait;
use slog_scope::{crit, error, info};
use structopt::StructOpt;

use crate::{
    cmd::crd::CustomResourceDefinitionError,
    svc::{
        apis,
        cfg::Configuration,
        k8s::{addon::postgresql, client, State, Watcher},
    },
};

pub mod crd;

// -----------------------------------------------------------------------------
// Executor trait

#[async_trait]
pub trait Executor {
    type Error;

    async fn execute(&self, config: Arc<Configuration>) -> Result<(), Self::Error>;
}

// -----------------------------------------------------------------------------
// CommandError enum

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("failed to execute command '{0}', {1}")]
    Execution(String, Arc<CommandError>),
    #[error("failed to execute command, {0}")]
    CustomResourceDefinition(CustomResourceDefinitionError),
}

// -----------------------------------------------------------------------------
// Command enum

#[derive(StructOpt, Clone, Debug)]
pub enum Command {
    /// Interact with custom resource definition
    #[structopt(name = "custom-resource-definition", aliases= &["crd"])]
    CustomResourceDefinition(crd::CustomResourceDefinition),
}

#[async_trait]
impl Executor for Command {
    type Error = CommandError;

    async fn execute(&self, config: Arc<Configuration>) -> Result<(), Self::Error> {
        match self {
            Self::CustomResourceDefinition(crd) => crd
                .execute(config)
                .await
                .map_err(CommandError::CustomResourceDefinition)
                .map_err(|err| {
                    CommandError::Execution("custom-resource-definition".into(), Arc::new(err))
                }),
        }
    }
}

// -----------------------------------------------------------------------------
// Args struct

#[derive(StructOpt, Clone, Debug)]
#[structopt(about = env!("CARGO_PKG_DESCRIPTION"))]
pub struct Args {
    /// Increase log verbosity
    #[structopt(short = "v", global = true, parse(from_occurrences))]
    pub verbosity: usize,
    /// Specify location of kubeconfig
    #[structopt(short = "k", long = "kubeconfig", global = true)]
    pub kubeconfig: Option<PathBuf>,
    /// Specify location of configuration
    #[structopt(short = "c", long = "config", global = true)]
    pub config: Option<PathBuf>,
    /// Check if configuration is healthy
    #[structopt(short = "t", long = "check", global = true)]
    pub check: bool,
    #[structopt(subcommand)]
    pub command: Option<Command>,
}

// -----------------------------------------------------------------------------
// DaemonError enum

#[derive(thiserror::Error, Debug)]
pub enum DaemonError {
    #[error("failed to handle termintion signal, {0}")]
    SigTerm(io::Error),
    #[error("failed to create kubernetes client, {0}")]
    Client(kube::Error),
}

// -----------------------------------------------------------------------------
// daemon function

pub async fn daemon(
    kubeconfig: Option<PathBuf>,
    config: Arc<Configuration>,
) -> Result<(), DaemonError> {
    // -------------------------------------------------------------------------
    // Create a new kubernetes client from path if defined, or via the
    // environment or defaults locations
    let kube_client = client::try_new(kubeconfig)
        .await
        .map_err(DaemonError::Client)?;

    // -------------------------------------------------------------------------
    // Create a new clever-cloud client
    let clever_client = apis::Client::from(config.to_owned());

    // -------------------------------------------------------------------------
    // Create state to give to each reconciler
    let state = State::new(kube_client, clever_client, config);

    // -------------------------------------------------------------------------
    // Create reconcilers
    let handles = vec![tokio::spawn(async {
        let reconciler = postgresql::Reconciler::default();

        info!("Start to listen for events of postgresql addon custom resource");
        if let Err(err) = reconciler.watch(state).await {
            crit!("Could not reconcile postgresql addon custom resource"; "error" => err.to_string());
        }

        abort();
    })];

    // -------------------------------------------------------------------------
    // Wait for termination signal
    tokio::signal::ctrl_c()
        .await
        .map_err(DaemonError::SigTerm)?;

    // -------------------------------------------------------------------------
    // Cancel reconcilers
    handles.iter().for_each(|handle| handle.abort());

    for handle in handles {
        if let Err(err) = handle.await {
            if !err.is_cancelled() {
                error!("could not wait for the task to complete"; "error" => err.to_string());
            }
        }
    }

    Ok(())
}
