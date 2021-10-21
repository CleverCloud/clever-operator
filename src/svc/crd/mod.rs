//! # Custom resource definition module
//!
//! This module provide custom resource definition managed by the operator,
//! their structures, implementation and reconciliation loop.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod mongodb;
pub mod mysql;
pub mod postgresql;
pub mod redis;

// -----------------------------------------------------------------------------
// Instance structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Instance {
    pub region: String,
    pub plan: String,
}
