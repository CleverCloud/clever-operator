//! # Clever Cloud module
//!
//! This module provide structures, traits and helpers related to Clever-Cloud
//! and the `clevercloud-sdk` crate.

use clevercloud_sdk::{
    v2,
    v4::addon_provider::{config_provider::addon::environment, plan},
};

pub mod client;
pub mod ext;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Addon(v2::addon::Error),
    #[error("{0}")]
    Plan(plan::Error),
    #[error("{0}")]
    Environment(environment::Error),
}

impl From<v2::addon::Error> for Error {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: v2::addon::Error) -> Self {
        Self::Addon(err)
    }
}

impl From<plan::Error> for Error {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: plan::Error) -> Self {
        Self::Plan(err)
    }
}

impl From<environment::Error> for Error {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: environment::Error) -> Self {
        Self::Environment(err)
    }
}
