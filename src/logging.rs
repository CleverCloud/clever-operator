//! # Logging module
//!
//! This module provides logging facilities and helpers

use tracing::Level;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to set global default subscriber, {0}")]
    GlobalDefaultSubscriber(tracing::subscriber::SetGlobalDefaultError),
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

pub fn initialize(verbosity: usize) -> Result<(), Error> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(level(verbosity))
            .with_thread_names(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(true)
            .finish(),
    )
    .map_err(Error::GlobalDefaultSubscriber)
}
