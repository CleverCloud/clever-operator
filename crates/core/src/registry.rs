//! # Registry
//!
//! Holds the set of [`Controller`]s to run and starts them. This replaces the
//! daemon's hardcoded `tokio::select!`: controllers are added dynamically (so a
//! future configuration can enable only a subset) and run concurrently.

use tokio::task::JoinSet;
use tracing::info;

use crate::controller::{BoxError, Controller};

/// A set of controllers to run together.
#[derive(Default)]
pub struct Registry {
    controllers: Vec<Box<dyn Controller>>,
}

impl Registry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a controller to the registry.
    pub fn register(&mut self, controller: Box<dyn Controller>) -> &mut Self {
        self.controllers.push(controller);
        self
    }

    /// Number of registered controllers.
    pub fn len(&self) -> usize {
        self.controllers.len()
    }

    /// Whether the registry has no controller.
    pub fn is_empty(&self) -> bool {
        self.controllers.is_empty()
    }

    /// Run every registered controller concurrently, resolving as soon as the
    /// first one stops or errors (the remaining ones are then aborted). This
    /// mirrors the previous `tokio::select!` behaviour.
    pub async fn run(self) -> Result<(), BoxError> {
        let mut set = JoinSet::new();
        for controller in self.controllers {
            let kind = controller.kind();
            info!(kind, "Start to listen for events of custom resource");
            let future = controller.run();
            // Carry the kind into the spawned future so a controller error
            // remains tied to the resource it came from (kind is `Copy`).
            set.spawn(async move { future.await.map_err(|err| format!("{kind}: {err}").into()) });
        }

        match set.join_next().await {
            Some(Ok(result)) => result,
            Some(Err(err)) => Err(Box::new(err)),
            None => Ok(()),
        }
    }
}
