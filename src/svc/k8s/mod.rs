//! # Kubernetes module
//!
//! This module provide kubernetes custom resources, helpers and custom resource definition
//! generator

#[cfg(feature = "metrics")]
use std::sync::LazyLock;
use std::{error::Error, fmt::Debug, hash::Hash, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::NamespaceResourceScope;
use kube::{
    CustomResourceExt, Resource, ResourceExt,
    runtime::{
        Controller,
        controller::{self, Action},
        watcher,
    },
};
#[cfg(feature = "metrics")]
use prometheus::{CounterVec, opts, register_counter_vec};
use serde::de::DeserializeOwned;
use tokio::time::{Instant, sleep_until};
#[cfg(feature = "tracing")]
use tracing::Instrument;
use tracing::{debug, error, info, trace};

use crate::svc::{cfg::Configuration, clevercloud};

pub mod client;
pub mod finalizer;
pub mod recorder;
pub mod resource;
pub mod secret;

// -----------------------------------------------------------------------------
// constants

pub const RECONCILIATION_UPSERT_EVENT: &str = "upsert";
pub const RECONCILIATION_DELETE_EVENT: &str = "delete";

// -----------------------------------------------------------------------------
// Telemetry

#[cfg(feature = "metrics")]
static RECONCILIATION_SUCCESS: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_operator_reconciliation_success",
            "number of successful reconciliation"
        ),
        &["kind"]
    )
    .expect("metrics 'kubernetes_operator_reconciliation_success' to not be already initialized")
});

#[cfg(feature = "metrics")]
static RECONCILIATION_FAILED: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_operator_reconciliation_failed",
            "number of failed reconciliation"
        ),
        &["kind"]
    )
    .expect("metrics 'kubernetes_operator_reconciliation_failed' to not be already initialized")
});

#[cfg(feature = "metrics")]
static RECONCILIATION_EVENT: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_operator_reconciliation_event",
            "number of upsert event",
        ),
        &["kind", "namespace", "event"]
    )
    .expect("metrics 'kubernetes_operator_reconciliation_event' to not be already initialized")
});

#[cfg(feature = "metrics")]
static RECONCILIATION_DURATION: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_operator_reconciliation_duration",
            "duration of reconciliation",
        ),
        &["kind", "unit"]
    )
    .expect("metrics 'kubernetes_operator_reconciliation_duration' to not be already initialized")
});

// -----------------------------------------------------------------------------
// State structure

/// contains clients to interact with kubernetes and clever-cloud apis.
#[derive(Clone)]
pub struct Context {
    pub kube: kube::Client,
    pub apis: clevercloud::client::Client,
    pub config: Arc<Configuration>,
}

impl
    From<(
        kube::Client,
        clevercloud::client::Client,
        Arc<Configuration>,
    )> for Context
{
    fn from(
        (kube, apis, config): (
            kube::Client,
            clevercloud::client::Client,
            Arc<Configuration>,
        ),
    ) -> Self {
        Self { kube, apis, config }
    }
}

impl Context {
    pub fn new(k: kube::Client, a: clevercloud::client::Client, c: Arc<Configuration>) -> Self {
        Self::from((k, a, c))
    }
}

// -----------------------------------------------------------------------------
// ControllerBuilder trait

/// provides a common way to create a kubernetes
/// controller [`Controller<T>`]
pub trait ControllerBuilder<T>
where
    T: Resource<Scope = NamespaceResourceScope> + Clone + Debug,
    <T as Resource>::DynamicType: Eq + Hash,
{
    /// returns a new created kubernetes controller
    fn build(&self, context: Arc<Context>) -> Controller<T>;
}

// -----------------------------------------------------------------------------
// Reconciler trait

/// provides two method which is given to a kubenetes controller
/// [`Controller<T>`]
#[async_trait]
pub trait Reconciler<T>
where
    T: Resource<Scope = NamespaceResourceScope>
        + ResourceExt
        + CustomResourceExt
        + Debug
        + Clone
        + Send
        + Sync
        + 'static,
{
    type Error: Error + Send + Sync;

    /// create or update the object, this is part of the the reconcile function
    async fn upsert(ctx: Arc<Context>, obj: Arc<T>) -> Result<(), Self::Error>;

    /// delete the object from kubernetes and third parts
    async fn delete(ctx: Arc<Context>, obj: Arc<T>) -> Result<(), Self::Error>;

    /// returns a [`Action`] to perform following the given error
    fn retry(_obj: Arc<T>, err: &Self::Error, _ctx: Arc<Context>) -> Action {
        // Implements a basic reconciliation which always re-schedule the event
        // 500 ms later
        trace!("Requeue failed reconciliation for 500ms, {}", err);
        Action::requeue(Duration::from_millis(500))
    }

    /// process the object and perform actions on kubernetes and/or
    /// clever-cloud api returns a [`Action`] to maybe perform another
    /// reconciliation or an error, if something gets wrong.
    async fn reconcile(obj: Arc<T>, ctx: Arc<Context>) -> Result<Action, Self::Error> {
        let (namespace, name) = resource::namespaced_name(&*obj);
        let api_resource = T::api_resource();

        if resource::deleted(obj.as_ref()) {
            info!(
                kind = &api_resource.kind,
                namespace = &namespace,
                name = &name,
                "Received deletion event for custom resource",
            );

            #[cfg(feature = "metrics")]
            RECONCILIATION_EVENT
                .with_label_values(&[&api_resource.kind, &namespace, RECONCILIATION_DELETE_EVENT])
                .inc();

            #[cfg(not(feature = "tracing"))]
            let result = Self::delete(ctx, obj.to_owned()).await;

            #[cfg(feature = "tracing")]
            let result = Self::delete(ctx, obj.to_owned())
                .instrument(tracing::info_span!("Reconciler::delete"))
                .await;

            if let Err(err) = result {
                error!(
                    kind = &api_resource.kind,
                    namespace = &namespace,
                    name = &name,
                    error = err.to_string(),
                    "Failed to delete custom resource"
                );

                return Err(err);
            }
        } else {
            info!(
                kind = &api_resource.kind,
                namespace = &namespace,
                name = &name,
                "Received upsertion event for custom resource",
            );

            #[cfg(feature = "metrics")]
            RECONCILIATION_EVENT
                .with_label_values(&[&api_resource.kind, &namespace, RECONCILIATION_UPSERT_EVENT])
                .inc();

            #[cfg(not(feature = "tracing"))]
            let result = Self::upsert(ctx, obj.to_owned()).await;

            #[cfg(feature = "tracing")]
            let result = Self::upsert(ctx, obj.to_owned())
                .instrument(tracing::info_span!("Reconciler::upsert"))
                .await;

            if let Err(err) = result {
                error!(
                    kind = &api_resource.kind,
                    namespace = &namespace,
                    name = &name,
                    error = err.to_string(),
                    "Failed to upsert custom resource"
                );

                return Err(err);
            }
        }

        Ok(Action::await_change())
    }
}

// -----------------------------------------------------------------------------
// WatcherError trait

/// group other trait needed to provide a default
/// implementation for [`Watcher<T>`] trait
pub trait WatcherError:
    From<kube::Error> + From<controller::Error<Self, watcher::Error>> + Error
where
    Self: 'static,
{
}

/// Blanket implementation of [`WatcherError<T>`]
impl<T> WatcherError for T
where
    T: From<kube::Error> + From<controller::Error<Self, watcher::Error>> + Error,
    Self: 'static,
{
}

// -----------------------------------------------------------------------------
// Watcher trait

/// provides a watch method that listen to events of
/// kubernetes custom resource using a [`Controller<T>`]
#[async_trait]
pub trait Watcher<T>: ControllerBuilder<T> + Reconciler<T>
where
    T: Resource<Scope = NamespaceResourceScope>
        + ResourceExt
        + CustomResourceExt
        + DeserializeOwned
        + Clone
        + Debug
        + Send
        + Sync
        + 'static,
    <T as Resource>::DynamicType: Unpin + Eq + Hash + Clone + Debug + Send + Sync,
    Self: Send + Sync + 'static,
    <Self as Reconciler<T>>::Error: WatcherError + Send + Sync,
{
    type Error: WatcherError + Send + Sync;

    /// listen for events of the custom resource as generic parameter
    async fn watch(&self, context: Arc<Context>) -> Result<(), <Self as Watcher<T>>::Error> {
        let api_resource = T::api_resource();
        let mut stream = self
            .build(context.to_owned())
            .run(Self::reconcile, Self::retry, context)
            .boxed();

        loop {
            let instant = Instant::now();

            match stream.try_next().await {
                Ok(None) => {
                    debug!("We have reached the end of the infinite watch stream");
                    return Ok(());
                }
                Ok(Some((obj, _action))) => {
                    info!(
                        kind = &api_resource.kind,
                        namespace = obj.namespace.unwrap_or_else(|| "<none>".to_string()),
                        name = obj.name,
                        "Successfully reconcile resource",
                    );

                    #[cfg(feature = "metrics")]
                    RECONCILIATION_SUCCESS
                        .with_label_values(&[&api_resource.kind])
                        .inc();
                }
                Err(controller::Error::ObjectNotFound(obj)) => {
                    debug!(
                        kind = &api_resource.kind,
                        namespace = obj.namespace.unwrap_or_else(|| "<none>".to_string()),
                        name = obj.name,
                        "Received an event about an already deleted resource",
                    );

                    #[cfg(feature = "metrics")]
                    RECONCILIATION_SUCCESS
                        .with_label_values(&[&api_resource.kind])
                        .inc();
                }
                Err(err) => {
                    error!(
                        kind = &api_resource.kind,
                        error = err.to_string(),
                        "Failed to reconcile resource",
                    );

                    #[cfg(feature = "metrics")]
                    RECONCILIATION_FAILED
                        .with_label_values(&[&api_resource.kind])
                        .inc();
                }
            }

            trace!(
                kind = &api_resource.kind,
                duration = Instant::now()
                    .checked_duration_since(instant + Duration::from_millis(100))
                    .map(|d| d.as_millis())
                    .unwrap_or_else(|| 0),
                "Put watch event loop for resource to sleep",
            );

            #[cfg(feature = "metrics")]
            RECONCILIATION_DURATION
                .with_label_values(&[&api_resource.kind, "us"])
                .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

            sleep_until(instant + Duration::from_millis(100)).await;
        }
    }
}

/// Blanket implementation for [`Watcher<T>`]
impl<T, U> Watcher<T> for U
where
    T: Resource<Scope = NamespaceResourceScope>
        + ResourceExt
        + CustomResourceExt
        + DeserializeOwned
        + Clone
        + Debug
        + Send
        + Sync
        + 'static,
    <T as Resource>::DynamicType: Unpin + Eq + Hash + Clone + Debug + Send + Sync,
    U: Reconciler<T> + ControllerBuilder<T>,
    U::Error: WatcherError + Send + Sync,
    Self: Send + Sync + 'static,
{
    type Error = U::Error;
}
