//! # Command module
//!
//! This module provides command line interface structures and helpers
use std::{future::Future, io, path::PathBuf, sync::Arc};

use clap::{ArgAction, Parser, Subcommand};
use clever_kubernetes_operator_core::{BoxError, FutureController, Registry};
use clevercloud_sdk::v4::addon_provider::AddonProviderId;
use paw::ParseArgs;

use crate::{
    cmd::{configmap::ConfigMapError, crd::CustomResourceDefinitionError, secret::SecretError},
    svc::{
        cfg::Configuration,
        clevercloud::{self, catalog},
        crd::{
            azimutt, cellar, config_provider, elasticsearch, keycloak, kv, matomo, metabase,
            mongodb, mysql, otoroshi, postgresql, pulsar, redis,
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

pub trait Executor {
    type Error;

    fn execute(
        &self,
        config: Arc<Configuration>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
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
    #[error("failed to run a controller, {0}")]
    Controller(BoxError),
    #[error("refusing to start the daemon with no controller registered")]
    NoController,
    #[error("failed to serve http content, {0}")]
    Serve(http::server::Error),
}

// -----------------------------------------------------------------------------
// Command enum

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    #[clap(name = "custom-resource-definition", aliases = &["crd"], subcommand, about = "Interact with custom resource definition")]
    CustomResourceDefinition(crd::CustomResourceDefinition),
    #[clap(name = "configmap", aliases = &["cm"], subcommand, about = "Generate configmap from clever-kubernetes-operator configuration")]
    ConfigMap(configmap::ConfigMap),
    #[clap(name = "secret", aliases = &["s"], subcommand, about = "Generate secret from clever-kubernetes-operator configuration")]
    Secret(secret::Secret),
}

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
    let clever_client =
        clevercloud::client::new(config.api.to_owned()).map_err(Error::CleverClient)?;

    // -------------------------------------------------------------------------
    // Create context to give to each reconciler
    let context = Arc::new(Context::new(kube_client, clever_client, config));

    // -------------------------------------------------------------------------
    // Build the registry of controllers. Every add-on controller is registered
    // here; running only a subset will come later with configurable modules.
    //
    // The block below is the one list of supported add-ons: it registers their
    // controllers and evaluates to their catalog, which the drift check further
    // down compares to the one of the api. Keeping both on the same line is
    // what prevents the two from drifting apart.
    let mut registry = Registry::new();

    macro_rules! register {
        ($($kind:literal => $module:ident => $provider:expr),+ $(,)?) => {{
            $(
                let ctx = context.to_owned();
                registry.register(FutureController::boxed($kind, async move {
                    $module::Reconciler::default()
                        .watch(ctx)
                        .await
                        .map_err(|err| Box::new(err) as BoxError)
                }));
            )+

            vec![$(catalog::Supported { kind: $kind, provider: $provider }),+]
        }};
    }

    let supported = register! {
        "PostgreSql" => postgresql => AddonProviderId::PostgreSql,
        "Redis" => redis => AddonProviderId::Redis,
        "MySql" => mysql => AddonProviderId::MySql,
        "MongoDb" => mongodb => AddonProviderId::MongoDb,
        "Pulsar" => pulsar => AddonProviderId::Pulsar,
        "ConfigProvider" => config_provider => AddonProviderId::ConfigProvider,
        "ElasticSearch" => elasticsearch => AddonProviderId::ElasticSearch,
        "KV" => kv => AddonProviderId::KV,
        "Metabase" => metabase => AddonProviderId::Metabase,
        "Keycloak" => keycloak => AddonProviderId::Keycloak,
        "Matomo" => matomo => AddonProviderId::Matomo,
        "Otoroshi" => otoroshi => AddonProviderId::Otoroshi,
        "Azimutt" => azimutt => AddonProviderId::Azimutt,
        "Cellar" => cellar => AddonProviderId::Cellar,
    };

    // A registry with no controller would make `run()` resolve immediately and
    // the daemon exit cleanly without watching anything — fail loudly instead.
    // This becomes reachable once modules are configurable.
    if registry.is_empty() {
        return Err(Error::NoController);
    }

    // -------------------------------------------------------------------------
    // Report the add-ons the api and the operator disagree on, once. This is a
    // diagnostic: it is bounded in time and never prevents the daemon from
    // starting, the api of a self-hosted installation may well be slow or
    // unreachable at this point.
    catalog::check(&context.apis, &supported).await;

    // -------------------------------------------------------------------------
    // Run the controllers alongside the termination signal and the http server;
    // resolve as soon as any of them stops.
    tokio::select! {
        res = registry.run() => res.map_err(Error::Controller),
        res = tokio::signal::ctrl_c() => res.map_err(Error::SigTerm),
        res = http::server::serve(http::server::router(), context.config.operator.listen) => res.map_err(Error::Serve),
    }
}
