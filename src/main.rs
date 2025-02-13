//! # Clever operator
//!
//! A kubernetes operator that exposes clever cloud resources through custom
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

    info!("{} halted!", env!("CARGO_PKG_NAME"));
    Ok(())
}
