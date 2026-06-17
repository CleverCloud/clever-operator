//! # Controller
//!
//! A uniform abstraction over the operator's custom-resource controllers, so
//! the daemon can run an arbitrary, dynamically-built set of them rather than a
//! hardcoded list.

use std::{future::Future, pin::Pin};

use crate::strategy::SyncStrategy;

/// A type-erased error returned by a controller.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The future driving a controller until it stops or errors.
pub type ControllerFuture = Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send>>;

/// A long-running controller that watches and reconciles a custom resource.
///
/// Each module provides one so the [`crate::registry::Registry`] can run them
/// uniformly.
pub trait Controller: Send + 'static {
    /// Human-readable kind, used for logging (e.g. `"PostgreSql"`).
    fn kind(&self) -> &'static str;

    /// The synchronization strategy of this controller.
    ///
    /// Defaults to [`SyncStrategy::ExportOwned`], matching the add-on
    /// controllers. A `Bidirectional` controller (e.g. NodeGroups) overrides it
    /// once that lifecycle is implemented.
    fn strategy(&self) -> SyncStrategy {
        SyncStrategy::default()
    }

    /// Consume the controller and return the future driving its watch loop.
    fn run(self: Box<Self>) -> ControllerFuture;
}

/// A [`Controller`] backed by an already-built future.
///
/// It is the bridge used to wrap the existing reconcilers' `watch` loops as
/// controllers without changing them.
pub struct FutureController {
    kind: &'static str,
    future: ControllerFuture,
}

impl FutureController {
    /// Build a boxed controller from a kind and the future of its watch loop.
    pub fn boxed(
        kind: &'static str,
        future: impl Future<Output = Result<(), BoxError>> + Send + 'static,
    ) -> Box<dyn Controller> {
        Box::new(Self {
            kind,
            future: Box::pin(future),
        })
    }
}

impl Controller for FutureController {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn run(self: Box<Self>) -> ControllerFuture {
        self.future
    }
}
