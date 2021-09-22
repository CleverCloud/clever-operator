//! # Kubernetes module
//!
//! This module provide kubernetes custom resources, helpers and custom resource definition
//! generator

use std::{error::Error, fmt::Debug, hash::Hash, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use kube::{CustomResourceExt, Resource, ResourceExt};
use kube_runtime::{
    controller::{self, Context, ReconcilerAction},
    watcher, Controller,
};
use serde::de::DeserializeOwned;
use slog_scope::{debug, error, info, trace};
use tokio::time::{sleep_until, Instant};

use crate::svc::{apis, cfg::Configuration};

pub mod addon;
pub mod client;
pub mod finalizer;
pub mod resource;
pub mod secret;

// -----------------------------------------------------------------------------
// State structure

/// contains clients to interact with kubernetes and clever-cloud apis.
#[derive(Clone)]
pub struct State {
    pub kube: kube::Client,
    pub apis: apis::Client,
    pub config: Arc<Configuration>,
}

impl From<(kube::Client, apis::Client, Arc<Configuration>)> for State {
    fn from((kube, apis, config): (kube::Client, apis::Client, Arc<Configuration>)) -> Self {
        Self { kube, apis, config }
    }
}

impl State {
    pub fn new(k: kube::Client, a: apis::Client, c: Arc<Configuration>) -> Self {
        Self::from((k, a, c))
    }
}

// -----------------------------------------------------------------------------
// ControllerBuilder trait

/// provides a common way to create a kubernetes
/// controller [`Controller<T>`]
pub trait ControllerBuilder<T>
where
    T: Resource + Clone + Debug,
    <T as Resource>::DynamicType: Eq + Hash,
{
    /// returns a new created kubernetes controller
    fn build(&self, state: State) -> Controller<T>;
}

// -----------------------------------------------------------------------------
// ReconcilerError enum

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub enum RequeueAction {
    Requeue(Duration),
    NoRequeue,
}

// -----------------------------------------------------------------------------
// Reconciler trait

/// provides two method which is given to a kubenetes
/// controller [`Controller<T>`]
#[async_trait]
pub trait Reconciler<T>
where
    T: ResourceExt + CustomResourceExt + Send + Sync + 'static,
{
    type Error: Error + Send + Sync;

    /// create or update the object, this is part of the the reconcile function
    async fn upsert(ctx: &Context<State>, obj: &T) -> Result<(), Self::Error>;

    /// delete the object from kubernetes and third parts
    async fn delete(ctx: &Context<State>, bj: &T) -> Result<(), Self::Error>;

    /// returns a [`ReconcilerAction`] to perform following the given error
    fn retry(err: &Self::Error, _ctx: Context<State>) -> ReconcilerAction {
        // Implements a basic reconciliation which always re-schedule the event
        // 500 ms later
        trace!("Requeue failed reconciliation"; "duration" => 500, "error" => err.to_string());
        ReconcilerAction {
            requeue_after: Some(Duration::from_millis(500)),
        }
    }

    /// process the object and perform actions on kubernetes and/or
    /// clever-cloud api returns a [`ReconcilerAction`] to maybe perform another
    /// reconciliation or an error, if something gets wrong.
    async fn reconcile(obj: T, ctx: Context<State>) -> Result<ReconcilerAction, Self::Error> {
        let (namespace, name) = resource::namespaced_name(&obj);
        let api_resource = T::api_resource();

        if resource::deleted(&obj) {
            info!("Received deletion event for custom resource"; "kind" => &api_resource.kind, "uid" => &obj.meta().uid, "name" => &name, "namespace" => &namespace);
            if let Err(err) = Self::delete(&ctx, &obj).await {
                error!("Failed to delete custom resource"; "kind" => &api_resource.kind, "uid" => &obj.meta().uid, "name" => &name, "namespace" => &namespace, "error" => err.to_string());
                return Err(err);
            }
        } else {
            info!("Received upsertion event for custom resource"; "kind" => &api_resource.kind, "uid" => &obj.meta().uid, "name" => &obj.meta().name, "namespace" => &obj.meta().namespace);
            if let Err(err) = Self::upsert(&ctx, &obj).await {
                error!("Failed to upsert custom resource"; "kind" => &api_resource.kind, "uid" => &obj.meta().uid, "name" => &name, "namespace" => &namespace, "error" => err.to_string());
                return Err(err);
            }
        }

        Ok(ReconcilerAction {
            requeue_after: None,
        })
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

/// Blanklet implementation of [`WatcherError<T>`]
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
    T: DeserializeOwned + ResourceExt + CustomResourceExt + Clone + Debug + Send + Sync + 'static,
    <T as Resource>::DynamicType: Unpin + Eq + Hash + Clone + Debug + Send + Sync,
    Self: Send + Sync + 'static,
    <Self as Reconciler<T>>::Error: WatcherError + Send + Sync,
{
    type Error: WatcherError + Send + Sync;

    /// listen for events of the custom resource as generic parameter
    async fn watch(&self, state: State) -> Result<(), <Self as Watcher<T>>::Error> {
        let context = Context::new(state.to_owned());
        let api_resource = T::api_resource();
        let mut stream = self
            .build(state.to_owned())
            .run(Self::reconcile, Self::retry, context)
            .boxed();

        loop {
            let instant = Instant::now() + Duration::from_millis(100);

            match stream.try_next().await {
                Ok(None) => {
                    debug!("We have reached the end of the infinite watch stream");
                    return Ok(());
                }
                Ok(Some((obj, ReconcilerAction { requeue_after }))) => {
                    info!("Successfully reconcile resource"; "requeue" => requeue_after.map(|d| d.as_millis()), "kind" => &api_resource.kind, "name" => &obj.name, "namespace" => &obj.namespace);
                }
                Err(err) => {
                    error!("Failed to reconcile resource"; "kind" => &api_resource.kind, "error" => err.to_string());
                }
            }

            trace!("Put watch event loop to bed"; "kind" => &api_resource.kind, "duration" => instant.checked_duration_since(Instant::now()).map(|d| d.as_millis()).unwrap_or_else(|| 0));
            sleep_until(instant).await;
        }
    }
}

/// Blanklet implementation for [`Watcher<T>`]
impl<T, U> Watcher<T> for U
where
    T: DeserializeOwned + ResourceExt + CustomResourceExt + Clone + Debug + Send + Sync + 'static,
    <T as Resource>::DynamicType: Unpin + Eq + Hash + Clone + Debug + Send + Sync,
    U: Reconciler<T> + ControllerBuilder<T>,
    U::Error: WatcherError + Send + Sync,
    Self: Send + Sync + 'static,
{
    type Error = U::Error;
}