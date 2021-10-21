//! # Command module
//!
//! This module provide command line interface structures and helpers
use std::{error::Error, io, net::AddrParseError, path::PathBuf, process::abort, sync::Arc};

use async_trait::async_trait;
use clevercloud_sdk::{oauth10a::Credentials, Client};
use hyper::{
    service::{make_service_fn, service_fn},
    Server,
};
use slog_scope::{crit, error, info};
use structopt::StructOpt;

use crate::{
    cmd::crd::CustomResourceDefinitionError,
    svc::{
        cfg::Configuration,
        crd::{mongodb, mysql, postgresql, redis},
        k8s::{client, State, Watcher},
        telemetry::router,
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

    #[cfg_attr(feature = "trace", tracing::instrument)]
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
    #[error("failed to parse listen address '{0}', {1}")]
    Listen(String, AddrParseError),
    #[error("failed to handle termintion signal, {0}")]
    SigTerm(io::Error),
    #[error("failed to create kubernetes client, {0}")]
    Client(kube::Error),
}

// -----------------------------------------------------------------------------
// daemon function

#[cfg_attr(feature = "trace", tracing::instrument)]
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
    let credentials: Credentials = config.api.to_owned().into();
    let clever_client = Client::from(credentials);

    // -------------------------------------------------------------------------
    // Create state to give to each reconciler
    let postgresql_state = State::new(kube_client, clever_client, config.to_owned());
    let redis_state = postgresql_state.to_owned();
    let mysql_state = postgresql_state.to_owned();
    let mongodb_state = postgresql_state.to_owned();

    // -------------------------------------------------------------------------
    // Create reconcilers
    let handles = vec![
        tokio::spawn(async {
            let reconciler = postgresql::Reconciler::default();

            info!("Start to listen for events of postgresql addon custom resource");
            if let Err(err) = reconciler.watch(postgresql_state).await {
                crit!("Could not reconcile postgresql addon custom resource"; "error" => err.to_string());
            }

            abort();
        }),
        tokio::spawn(async {
            let reconciler = redis::Reconciler::default();

            info!("Start to listen for events of redis addon custom resource");
            if let Err(err) = reconciler.watch(redis_state).await {
                crit!("Could not reconcile redis addon custom resource"; "error" => err.to_string());
            }

            abort();
        }),
        tokio::spawn(async {
            let reconciler = mysql::Reconciler::default();

            info!("Start to listen for events of mysql addon custom resource");
            if let Err(err) = reconciler.watch(mysql_state).await {
                crit!("Could not reconcile mysql addon custom resource"; "error" => err.to_string());
            }

            abort();
        }),
        tokio::spawn(async {
            let reconciler = mongodb::Reconciler::default();

            info!("Start to listen for events of mongodb addon custom resource");
            if let Err(err) = reconciler.watch(mongodb_state).await {
                crit!("Could not reconcile mongodb addon custom resource"; "error" => err.to_string());
            }

            abort();
        }),
    ];

    // -------------------------------------------------------------------------
    // Create http server
    let addr = config
        .operator
        .listen
        .parse()
        .map_err(|err| DaemonError::Listen(config.operator.listen.to_owned(), err))?;

    let server = tokio::spawn(async move {
        let builder = match Server::try_bind(&addr) {
            Ok(builder) => builder,
            Err(err) => {
                crit!("Could not bind http server"; "error" => err.to_string());
                abort();
            }
        };

        let server = builder.serve(make_service_fn(|_| async {
            Ok::<_, Box<dyn Error + Send + Sync>>(service_fn(router))
        }));

        info!("Start to listen for http request on {}", addr);
        if let Err(err) = server.await {
            crit!("Could not serve http server"; "error" => err.to_string());
        }

        abort()
    });

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
                error!("Could not wait for the task to complete"; "error" => err.to_string());
            }
        }
    }

    // -------------------------------------------------------------------------
    // Cancel http server
    server.abort();
    if let Err(err) = server.await {
        if !err.is_cancelled() {
            error!("Could not wait for the http server to gracefully close"; "error" => err.to_string());
        }
    }

    Ok(())
}
