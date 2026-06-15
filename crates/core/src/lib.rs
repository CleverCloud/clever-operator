//! # Clever Kubernetes operator — core
//!
//! Shared building blocks for the Clever Cloud Kubernetes operator.
//!
//! This crate is meant to host the code reused across the operator: the
//! controller runtime/registry, leader election, Kubernetes primitives and the
//! Clever Cloud client wiring. It is intentionally minimal for now and is
//! populated incrementally as the operator is refactored onto a common core.
