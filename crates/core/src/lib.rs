//! # Clever Kubernetes operator — core
//!
//! Shared building blocks for the Clever Cloud Kubernetes operator.
//!
//! For now this crate hosts the controller abstraction ([`Controller`]) and the
//! [`Registry`] used by the operator daemon to run a dynamic set of controllers.
//! It will grow with leader election, Kubernetes primitives and Clever Cloud
//! client wiring as the operator is refactored onto a common core.

pub mod controller;
pub mod registry;

pub use controller::{BoxError, Controller, ControllerFuture, FutureController};
pub use registry::Registry;
