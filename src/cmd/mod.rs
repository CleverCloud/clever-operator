//! # Command module
//!
//! This module provides command line interface structures and helpers
use std::{io, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use clap::{ArgAction, Parser, Subcommand};
use paw::ParseArgs;
use tracing::{error, info};

use crate::{
    cmd::{configmap::ConfigMapError, crd::CustomResourceDefinitionError, secret::SecretError},
    svc::{
        cfg::Configuration,
        clevercloud,
        crd::{
            config_provider, elasticsearch, keycloak, kv, matomo, metabase, mongodb, mysql,
            postgresql, pulsar, redis,
        },
        http,
        k8s::{Context, Watcher, client},
    },
};

pub mod configmap;
pub mod crd;
pub mod secret;

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
    #[error("failed to execute command, {0}")]
    ConfigMap(ConfigMapError),
    #[error("failed to execute command, {0}")]
    Secret(SecretError),
    #[error("failed to handle termination signal, {0}")]
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
    #[error("failed to watch kv resources, {0}")]
    WatchKV(kv::ReconcilerError),
    #[error("failed to watch metabase resources, {0}")]
    WatchMetabase(metabase::ReconcilerError),
    #[error("failed to watch kv resources, {0}")]
    WatchKeycloak(keycloak::ReconcilerError),
    #[error("failed to watch matomo resources, {0}")]
    WatchMatomo(matomo::ReconcilerError),
    #[error("failed to serve http content, {0}")]
    Serve(http::server::Error),
    #[error("failed to spawn task on tokio, {0}")]
    Join(tokio::task::JoinError),
}

// -----------------------------------------------------------------------------
// Command enum

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    #[clap(name = "custom-resource-definition", aliases = &["crd"], subcommand, about = "Interact with custom resource definition")]
    CustomResourceDefinition(crd::CustomResourceDefinition),
    #[clap(name = "configmap", aliases = &["cm"], subcommand, about = "Generate configmap from clever-operator configuration")]
    ConfigMap(configmap::ConfigMap),
    #[clap(name = "secret", aliases = &["s"], subcommand, about = "Generate secret from clever-operator configuration")]
    Secret(secret::Secret),
}

#[async_trait]
impl Executor for Command {
    type Error = Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(config)))]
    async fn execute(&self, config: Arc<Configuration>) -> Result<(), Self::Error> {
        match self {
            Self::CustomResourceDefinition(crd) => crd
                .execute(config)
                .await
                .map_err(Error::CustomResourceDefinition)
                .map_err(|err| {
                    Error::Execution("custom-resource-definition".into(), Arc::new(err))
                }),
            Self::ConfigMap(cm) => cm
                .execute(config)
                .await
                .map_err(Error::ConfigMap)
                .map_err(|err| Error::Execution("config-map".into(), Arc::new(err))),
            Self::Secret(s) => s
                .execute(config)
                .await
                .map_err(Error::Secret)
                .map_err(|err| Error::Execution("secret".into(), Arc::new(err))),
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
    /// Specify the location of kubeconfig
    #[clap(short = 'k', long = "kubeconfig", global = true)]
    pub kubeconfig: Option<PathBuf>,
    /// Specify location of configuration
    #[clap(short = 'c', long = "config", global = true)]
    pub config: Option<PathBuf>,
    /// Check if the configuration is healthy
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

pub async fn daemon(kubeconfig: Option<PathBuf>, config: Arc<Configuration>) -> Result<(), Error> {
    // -------------------------------------------------------------------------
    // Create a new kubernetes client from a path if defined, or via the
    // environment or defaults locations
    let kube_client = client::try_new(kubeconfig).await.map_err(Error::Client)?;

    // -------------------------------------------------------------------------
    // Create a new clever-cloud client
    let clever_client = clevercloud::client::Client::from(config.api.to_owned());

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
    let kv_ctx = context.to_owned();
    let metabase_ctx = context.to_owned();
    let keycloak_ctx = context.to_owned();
    let matomo_ctx = context.to_owned();

    // -------------------------------------------------------------------------
    // Start services

    tokio::select! {
        r = tokio::spawn(async move {
            info!(kind = "PostgreSql", "Start to listen for events of custom resource");
            postgresql::Reconciler::default()
                .watch(postgresql_ctx)
                .await
                .map_err(Error::WatchPostgreSql)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "Redis", "Start to listen for events of custom resource");
            redis::Reconciler::default()
                .watch(redis_ctx)
                .await
                .map_err(Error::WatchRedis)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "MySql", "Start to listen for events of custom resource");
            mysql::Reconciler::default()
                .watch(mysql_ctx)
                .await
                .map_err(Error::WatchMySql)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "MongoDb", "Start to listen for events of custom resource");
            mongodb::Reconciler::default()
                .watch(mongodb_ctx)
                .await
                .map_err(Error::WatchMongoDb)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "Pulsar", "Start to listen for events of custom resource");
            pulsar::Reconciler::default()
                .watch(pulsar_ctx)
                .await
                .map_err(Error::WatchPulsar)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "ConfigProvider", "Start to listen for events of custom resource");
            config_provider::Reconciler::default()
                .watch(config_provider_ctx)
                .await
                .map_err(Error::WatchConfigProvider)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "ElasticSearch", "Start to listen for events of custom resource");
            elasticsearch::Reconciler::default()
                .watch(elasticsearch_ctx)
                .await
                .map_err(Error::WatchElasticSearch)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "KV", "Start to listen for events of custom resource");
            kv::Reconciler::default()
                .watch(kv_ctx)
                .await
                .map_err(Error::WatchKV)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "Metabase", "Start to listen for events of custom resource");
            metabase::Reconciler::default()
                .watch(metabase_ctx)
                .await
                .map_err(Error::WatchMetabase)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "Keycloak", "Start to listen for events of custom resource");
            keycloak::Reconciler::default()
                .watch(keycloak_ctx)
                .await
                .map_err(Error::WatchKeycloak)
        }) => r,
        r = tokio::spawn(async move {
            info!(kind = "Matomo", "Start to listen for events of custom resource");
            matomo::Reconciler::default()
                .watch(matomo_ctx)
                .await
                .map_err(Error::WatchMatomo)
        }) => r,
        r = tokio::spawn(async move { tokio::signal::ctrl_c().await.map_err(Error::SigTerm) }) => r,
        r = tokio::spawn(async move { http::server::serve(http::server::router(), context.config.operator.listen).await.map_err(Error::Serve) }) => r,
    }.map_err(Error::Join)??;

    Ok(())
}
