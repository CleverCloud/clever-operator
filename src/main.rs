//! # Clever operator
//!
//! A kubernetes operator that expose clever cloud's resources through custom
//! resource definition
use std::{convert::TryFrom, error::Error, path::PathBuf, sync::Arc};

use slog::{o, Drain, Level, LevelFilter, Logger};
use slog_async::Async;
use slog_scope::{debug, info, set_global_logger, GlobalLoggerGuard as Guard};
use slog_term::{FullFormat, TermDecorator};
use structopt::StructOpt;
use svc::cfg::Configuration;

mod svc;

pub fn initialize(verbosity: &usize) -> Guard {
    let level = Level::from_usize(Level::Critical.as_usize() + verbosity).unwrap_or(Level::Trace);

    let decorator = TermDecorator::new().build();
    let drain = FullFormat::new(decorator).build().fuse();
    let drain = LevelFilter::new(drain, level).fuse();
    let drain = Async::new(drain).build().fuse();

    set_global_logger(Logger::root(drain, o!()))
}

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
}

#[paw::main]
#[tokio::main]
async fn main(args: Args) -> Result<(), Box<dyn Error + Send + Sync>> {
    let _guard = initialize(&args.verbosity);
    let config = Arc::new(match &args.config {
        Some(path) => Configuration::try_from(path.to_owned())?,
        None => Configuration::try_default()?,
    });

    if args.check {
        debug!("{:#?}", config);
        info!("Configuration is healthy!");
        return Ok(());
    }

    info!("{} halted!", env!("CARGO_PKG_NAME"));
    Ok(())
}
