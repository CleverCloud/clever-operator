//! # Reconciliation strategy
//!
//! Describes how a module keeps its Kubernetes custom resources and the
//! upstream Clever Cloud API in sync. This is the seam that lets different
//! lifecycles coexist behind the common [`crate::controller::Controller`]
//! abstraction.

/// The synchronization strategy of a controller.
///
/// Defaults to [`SyncStrategy::ExportOwned`], matching the existing add-on
/// controllers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncStrategy {
    /// The Kubernetes custom resource is the **source of truth**.
    ///
    /// The operator provisions the upstream resource on apply and deprovisions
    /// it on CR deletion (via a finalizer); it reacts to CR events only. This is
    /// how the add-on controllers behave.
    #[default]
    ExportOwned,
    /// The Clever Cloud **API** is the source of truth.
    ///
    /// The resource can be created from Kubernetes *or* from the API; the
    /// operator reconciles both ways and polls upstream to reflect changes back
    /// into the CR. **Not implemented yet.**
    Bidirectional,
}

impl SyncStrategy {
    /// A short, stable label for logs and metrics.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ExportOwned => "export-owned",
            Self::Bidirectional => "bidirectional",
        }
    }
}

impl std::fmt::Display for SyncStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
