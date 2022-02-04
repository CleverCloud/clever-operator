//! # Clever Cloud module
//!
//! This module provide structures, traits and helpers related to Clever-Cloud
//! and the `clevercloud-sdk` crate.

use clevercloud_sdk::{v2, v4};

pub mod ext;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Addon(v2::addon::Error),
    #[error("{0}")]
    Plan(v4::addon_provider::plan::Error),
}

impl From<v2::addon::Error> for Error {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from(err: v2::addon::Error) -> Self {
        Self::Addon(err)
    }
}

impl From<v4::addon_provider::plan::Error> for Error {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from(err: v4::addon_provider::plan::Error) -> Self {
        Self::Plan(err)
    }
}
