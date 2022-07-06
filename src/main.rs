//! # Clever operator
//!
//! A kubernetes operator that expose clever cloud's resources through custom
//! resource definition
use std::{convert::TryFrom, sync::Arc};

#[cfg(feature = "trace")]
use opentelemetry::global;
#[cfg(feature = "trace")]
use opentelemetry_jaeger::Propagator;
use tracing::{debug, error, info};
#[cfg(feature = "trace")]
use tracing_subscriber::{layer::SubscriberExt, Registry};

use crate::{
    cmd::{daemon, Args, Executor},
    svc::cfg::Configuration,
};

pub mod cmd;
pub mod logging;
pub mod svc;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to interact with command line interface, {0}")]
    Command(cmd::Error),
    #[error("failed to initialize logging system, {0}")]
    Logging(logging::Error),
    #[error("failed to load configuration, {0}")]
    Configuration(svc::cfg::Error),
    #[error("failed to set subscriber, {0}")]
    Subscriber(tracing::subscriber::SetGlobalDefaultError),
    #[cfg(feature = "trace")]
    #[error("failed to build tracing subscription")]
    Subscription(opentelemetry::trace::TraceError),
}

impl From<cmd::Error> for Error {
    fn from(err: cmd::Error) -> Self {
        Self::Command(err)
    }
}

impl From<logging::Error> for Error {
    fn from(err: logging::Error) -> Self {
        Self::Logging(err)
    }
}

impl From<svc::cfg::Error> for Error {
    fn from(err: svc::cfg::Error) -> Self {
        Self::Configuration(err)
    }
}

impl From<tracing::subscriber::SetGlobalDefaultError> for Error {
    fn from(err: tracing::subscriber::SetGlobalDefaultError) -> Self {
        Self::Subscriber(err)
    }
}

#[cfg(feature = "trace")]
impl From<opentelemetry::trace::TraceError> for Error {
    fn from(err: opentelemetry::trace::TraceError) -> Self {
        Self::Subscription(err)
    }
}

// -----------------------------------------------------------------------------
// main entrypoint

#[paw::main]
#[tokio::main]
pub(crate) async fn main(args: Args) -> Result<(), Error> {
    logging::initialize(args.verbosity as usize)?;

    let config = Arc::new(match &args.config {
        Some(path) => Configuration::try_from(path.to_owned())?,
        None => Configuration::try_default()?,
    });

    config.help();
    if args.check {
        debug!("{:#?}", config);
        info!("{} configuration is healthy!", env!("CARGO_PKG_NAME"));
        return Ok(());
    }

    #[cfg(feature = "tracker")]
    let _sguard = config.sentry.dsn.as_ref().map(|dsn| {
        sentry::init((
            dsn.to_owned(),
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ))
    });

    #[cfg(feature = "trace")]
    if !config.jaeger.endpoint.is_empty() {
        info!(
            "Start to trace using jaeger with opentelemetry compatibility on endpoint {}",
            &config.jaeger.endpoint
        );
        global::set_text_map_propagator(Propagator::new());

        let mut builder = opentelemetry_jaeger::new_pipeline()
            .with_collector_endpoint(config.jaeger.endpoint.to_owned())
            .with_service_name(env!("CARGO_PKG_NAME"));

        if let Some(user) = &config.jaeger.user {
            builder = builder.with_collector_username(user);
        }

        if let Some(password) = &config.jaeger.password {
            builder = builder.with_collector_password(password);
        }

        let layer = tracing_opentelemetry::layer().with_tracer(builder.install_simple()?);

        tracing::subscriber::set_global_default(Registry::default().with(layer))?;
    }

    let result = match &args.command {
        Some(cmd) => cmd.execute(config).await,
        None => daemon(args.kubeconfig, config).await,
    }
    .map_err(Error::Command);

    if let Err(err) = result {
        error!(
            "could not execute {} properly, {}",
            env!("CARGO_PKG_NAME"),
            err
        );
        return Err(err);
    }

    #[cfg(feature = "trace")]
    global::shutdown_tracer_provider();

    info!("{} halted!", env!("CARGO_PKG_NAME"));
    Ok(())
}
