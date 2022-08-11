//! # Command module
//!
//! This module provide command line interface structures and helpers
use std::{io, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use clap::{ArgAction, Parser, Subcommand};
use clevercloud_sdk::oauth10a::Credentials;
use paw::ParseArgs;
use tracing::{error, info};

use crate::{
    cmd::crd::CustomResourceDefinitionError,
    svc::{
        cfg::Configuration,
        clevercloud,
        crd::{config_provider, elasticsearch, mongodb, mysql, postgresql, pulsar, redis},
        http,
        k8s::{client, Context, Watcher},
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
pub enum Error {
    #[error("failed to execute command '{0}', {1}")]
    Execution(String, Arc<Error>),
    #[error("failed to execute command, {0}")]
    CustomResourceDefinition(CustomResourceDefinitionError),
    #[error("failed to handle termintion signal, {0}")]
    SigTerm(io::Error),
    #[error("failed to create kubernetes client, {0}")]
    Client(client::Error),
    #[error("failed to create clevercloud client, {0}")]
    CleverClient(clevercloud::client::Error),
    #[error("failed to watch PostgreSql resources, {0}")]
    WatchPostgreSql(postgresql::ReconcilerError),
    #[error("failed to watch Redis resources, {0}")]
    WatchRedis(redis::ReconcilerError),
    #[error("failed to watch MySql resources, {0}")]
    WatchMySql(mysql::ReconcilerError),
    #[error("failed to watch ElasticSearch resources, {0}")]
    WatchElasticSearch(elasticsearch::ReconcilerError),
    #[error("failed to watch MongoDb resources, {0}")]
    WatchMongoDb(mongodb::ReconcilerError),
    #[error("failed to watch ConfigProvider resources, {0}")]
    WatchConfigProvider(config_provider::ReconcilerError),
    #[error("failed to watch Pulsar resources, {0}")]
    WatchPulsar(pulsar::ReconcilerError),
    #[error("failed to serve http content, {0}")]
    Serve(http::server::Error),
    #[error("failed to spawn task on tokio, {0}")]
    Join(tokio::task::JoinError),
}

// -----------------------------------------------------------------------------
// Command enum

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    /// Interact with custom resource definition
    #[clap(name = "custom-resource-definition", aliases= &["crd"], subcommand)]
    CustomResourceDefinition(crd::CustomResourceDefinition),
}

#[async_trait]
impl Executor for Command {
    type Error = Error;

    #[cfg_attr(feature = "trace", tracing::instrument(skip(config)))]
    async fn execute(&self, config: Arc<Configuration>) -> Result<(), Self::Error> {
        match self {
            Self::CustomResourceDefinition(crd) => crd
                .execute(config)
                .await
                .map_err(Error::CustomResourceDefinition)
                .map_err(|err| {
                    Error::Execution("custom-resource-definition".into(), Arc::new(err))
                }),
        }
    }
}

// -----------------------------------------------------------------------------
// Args struct

#[derive(Parser, Clone, Debug)]
#[clap(author, version, about)]
pub struct Args {
    /// Increase log verbosity
    #[clap(short = 'v', global = true, action = ArgAction::Count)]
    pub verbosity: u8,
    /// Specify location of kubeconfig
    #[clap(short = 'k', long = "kubeconfig", global = true)]
    pub kubeconfig: Option<PathBuf>,
    /// Specify location of configuration
    #[clap(short = 'c', long = "config", global = true)]
    pub config: Option<PathBuf>,
    /// Check if configuration is healthy
    #[clap(short = 't', long = "check", global = true)]
    pub check: bool,
    #[clap(subcommand)]
    pub command: Option<Command>,
}

impl ParseArgs for Args {
    type Error = Error;

    fn parse_args() -> Result<Self, Self::Error> {
        Ok(Self::parse())
    }
}

// -----------------------------------------------------------------------------
// daemon function

#[cfg_attr(feature = "trace", tracing::instrument(skip(config)))]
pub async fn daemon(kubeconfig: Option<PathBuf>, config: Arc<Configuration>) -> Result<(), Error> {
    // -------------------------------------------------------------------------
    // Create a new kubernetes client from path if defined, or via the
    // environment or defaults locations
    let kube_client = client::try_new(kubeconfig).await.map_err(Error::Client)?;

    // -------------------------------------------------------------------------
    // Create a new clever-cloud client
    let credentials: Credentials = config.api.to_owned().into();
    let clever_client =
        clevercloud::client::try_new(credentials, &config.proxy).map_err(Error::CleverClient)?;

    // -------------------------------------------------------------------------
    // Create context to give to each reconciler
    let context = Arc::new(Context::new(kube_client, clever_client, config.to_owned()));

    let postgresql_ctx = context.to_owned();
    let mysql_ctx = context.to_owned();
    let mongodb_ctx = context.to_owned();
    let redis_ctx = context.to_owned();
    let elasticsearch_ctx = context.to_owned();
    let config_provider_ctx = context.to_owned();
    let pulsar_ctx = context.to_owned();

    // -------------------------------------------------------------------------
    // Start services

    tokio::select! {
        r = tokio::spawn(async move {
            info!("Start to listen for events of postgresql addon custom resource");
            postgresql::Reconciler::default()
                .watch(postgresql_ctx)
                .await
                .map_err(Error::WatchPostgreSql)
        }) => r,
        r = tokio::spawn(async move {
            info!("Start to listen for events of redis addon custom resource");
            redis::Reconciler::default()
                .watch(redis_ctx)
                .await
                .map_err(Error::WatchRedis)
        }) => r,
        r = tokio::spawn(async move {
            info!("Start to listen for events of mysql addon custom resource");
            mysql::Reconciler::default()
                .watch(mysql_ctx)
                .await
                .map_err(Error::WatchMySql)
        }) => r,
        r = tokio::spawn(async move {
            info!("Start to listen for events of mongodb addon custom resource");
            mongodb::Reconciler::default()
                .watch(mongodb_ctx)
                .await
                .map_err(Error::WatchMongoDb)
        }) => r,
        r = tokio::spawn(async move {
            info!("Start to listen for events of pulsar addon custom resource");
            pulsar::Reconciler::default()
                .watch(pulsar_ctx)
                .await
                .map_err(Error::WatchPulsar)
        }) => r,
        r = tokio::spawn(async move {
            info!("Start to listen for events of config-provider addon custom resource");
            config_provider::Reconciler::default()
                .watch(config_provider_ctx)
                .await
                .map_err(Error::WatchConfigProvider)
        }) => r,
        r = tokio::spawn(async move {
            info!("Start to listen for events of elasticsearch addon custom resource");
            elasticsearch::Reconciler::default()
                .watch(elasticsearch_ctx)
                .await
                .map_err(Error::WatchElasticSearch)
        }) => r,
        r = tokio::spawn(async move { tokio::signal::ctrl_c().await.map_err(Error::SigTerm) }) => r,
        r = tokio::spawn(async move { http::server::serve(config.to_owned()).await.map_err(Error::Serve) }) => r,
    }.map_err(Error::Join)??;

    Ok(())
}
