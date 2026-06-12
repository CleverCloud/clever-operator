//! # Logging module
//!
//! This module provides logging facilities and helpers

use tracing::Level;
use tracing_subscriber::{filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::svc::cfg::Configuration;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to set initialize registry globally, {0}")]
    InitializeRegistry(tracing_subscriber::util::TryInitError),
}

// -----------------------------------------------------------------------------
// helpers

pub const fn level(verbosity: usize) -> Level {
    match verbosity {
        0 => Level::ERROR,
        1 => Level::WARN,
        2 => Level::INFO,
        3 => Level::DEBUG,
        _ => Level::TRACE,
    }
}

pub fn initialize(_config: &Configuration, verbosity: usize) -> Result<(), Error> {
    let filter = LevelFilter::from_level(level(verbosity));
    let registry = tracing_subscriber::registry().with(filter).with(
        fmt::Layer::new()
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_line_number(true)
            .with_target(true),
    );

    registry.try_init().map_err(Error::InitializeRegistry)
}
