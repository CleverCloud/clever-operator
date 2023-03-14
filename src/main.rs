//! # Clever operator
//!
//! A kubernetes operator that expose clever cloud's resources through custom
//! resource definition

use std::{convert::TryFrom, sync::Arc};

use tracing::{error, info};

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
    #[error("failed to build tracing subscription, {0}")]
    Subscription(opentelemetry::trace::TraceError),
    #[cfg(feature = "tracker")]
    #[error("failed to parse sentry dsn uri, {0}")]
    ParseSentryDsn(sentry_types::ParseDsnError),
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
    let config = Arc::new(match &args.config {
        Some(path) => Configuration::try_from(path.to_owned())?,
        None => Configuration::try_default()?,
    });

    config.help();
    logging::initialize(&config, args.verbosity as usize)?;
    if args.check {
        println!("{} configuration is healthy!", env!("CARGO_PKG_NAME"));
        return Ok(());
    }

    #[cfg(feature = "tracker")]
    let _sguard = match config.sentry.dsn.as_ref() {
        None => None,
        Some(dsn) => {
            info!(
                dsn = dsn,
                "Configure sentry integration using the given dsn"
            );

            Some(sentry::init(sentry::ClientOptions {
                dsn: Some(dsn.parse().map_err(Error::ParseSentryDsn)?),
                release: sentry::release_name!(),
                ..Default::default()
            }))
        }
    };

    let result = match &args.command {
        Some(cmd) => cmd.execute(config).await,
        None => daemon(args.kubeconfig, config).await,
    }
    .map_err(Error::Command);

    if let Err(err) = result {
        error!(
            error = err.to_string(),
            "could not execute {} properly",
            env!("CARGO_PKG_NAME"),
        );

        return Err(err);
    }

    #[cfg(feature = "trace")]
    opentelemetry::global::shutdown_tracer_provider();

    info!("{} halted!", env!("CARGO_PKG_NAME"));
    Ok(())
}
