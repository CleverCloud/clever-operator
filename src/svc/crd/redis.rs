//! # Redis addon
//!
//! This module provide the redis custom resource and its definition

use std::{
    fmt::{self, Display, Formatter},
    sync::Arc,
};

use async_trait::async_trait;
use clevercloud_sdk::{
    v2::{
        self,
        addon::{self, CreateOpts},
    },
    v4::{
        self,
        addon_provider::{plan, redis, AddonProviderId},
    },
};
use futures::TryFutureExt;
use kube::{api::ListParams, Api, Resource, ResourceExt};
use kube_derive::CustomResource;
use kube_runtime::{controller, watcher, Controller};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::svc::{
    clevercloud::{self, ext::AddonExt},
    crd::Instance,
    k8s::{self, finalizer, recorder, resource, secret, Context, ControllerBuilder},
};

// -----------------------------------------------------------------------------
// Constants

pub const ADDON_FINALIZER: &str = "api.clever-cloud.com/redis";

// -----------------------------------------------------------------------------
// Opts structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Opts {
    #[serde(rename = "version")]
    pub version: redis::Version,
    #[serde(rename = "encryption")]
    pub encryption: bool,
}

#[allow(clippy::from_over_into)]
impl Into<addon::Opts> for Opts {
    fn into(self) -> addon::Opts {
        addon::Opts {
            version: Some(self.version.to_string()),
            encryption: Some(self.encryption.to_string()),
            ..Default::default()
        }
    }
}

// -----------------------------------------------------------------------------
// Spec structure

#[derive(CustomResource, JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug)]
#[kube(group = "api.clever-cloud.com")]
#[kube(version = "v1")]
#[kube(kind = "Redis")]
#[kube(singular = "redis")]
#[kube(plural = "redis")]
#[kube(shortname = "r")]
#[kube(status = "Status")]
#[kube(namespaced)]
#[kube(apiextensions = "v1")]
#[kube(derive = "PartialEq")]
pub struct Spec {
    #[serde(rename = "organisation")]
    pub organisation: String,
    #[serde(rename = "options")]
    pub options: Opts,
    #[serde(rename = "instance")]
    pub instance: Instance,
}

// -----------------------------------------------------------------------------
// Status structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct Status {
    #[serde(rename = "addon")]
    pub addon: Option<String>,
}

// -----------------------------------------------------------------------------
// Redis implementation

#[allow(clippy::from_over_into)]
impl Into<CreateOpts> for Redis {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn into(self) -> CreateOpts {
        CreateOpts {
            name: AddonExt::name(&self),
            region: self.spec.instance.region.to_owned(),
            provider_id: AddonProviderId::Redis.to_string(),
            plan: self.spec.instance.plan.to_owned(),
            options: self.spec.options.into(),
        }
    }
}

impl AddonExt for Redis {
    type Error = ReconcilerError;

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn id(&self) -> Option<String> {
        if let Some(status) = &self.status {
            return status.addon.to_owned();
        }

        None
    }

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn organisation(&self) -> String {
        self.spec.organisation.to_owned()
    }

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn name(&self) -> String {
        let delimiter = Self::delimiter();

        Self::prefix()
            + &delimiter
            + &Self::kind(&()).to_string()
            + &delimiter
            + &self
                .uid()
                .expect("expect all resources in kubernetes to have an identifier")
    }
}

impl Redis {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    pub fn set_addon_id(&mut self, id: Option<String>) {
        let mut status = self.status.get_or_insert_with(Status::default);

        status.addon = id;
        self.status = Some(status.to_owned());
    }

    #[cfg_attr(feature = "trace", tracing::instrument)]
    pub fn get_addon_id(&self) -> Option<String> {
        self.status.to_owned().unwrap_or_default().addon
    }
}

// -----------------------------------------------------------------------------
// RedisAction structure

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub enum Action {
    UpsertFinalizer,
    UpsertAddon,
    UpsertSecret,
    OverridesInstancePlan,
    DeleteFinalizer,
    DeleteAddon,
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::UpsertFinalizer => write!(f, "UpsertFinalizer"),
            Self::UpsertAddon => write!(f, "UpsertAddon"),
            Self::UpsertSecret => write!(f, "UpsertSecret"),
            Self::OverridesInstancePlan => write!(f, "OverridesInstancePlan"),
            Self::DeleteFinalizer => write!(f, "DeleteFinalizer"),
            Self::DeleteAddon => write!(f, "DeleteAddon"),
        }
    }
}

// -----------------------------------------------------------------------------
// ReconcilerError enum

#[derive(thiserror::Error, Debug)]
pub enum ReconcilerError {
    #[error("failed to reconcile resource, {0}")]
    Reconcile(String),
    #[error("failed to execute request on clever-cloud api, {0}")]
    CleverClient(clevercloud::Error),
    #[error("failed to execute request on kubernetes api, {0}")]
    KubeClient(kube::Error),
    #[error("failed to compute diff between the original and modified object, {0}")]
    Diff(serde_json::Error),
}

impl From<kube::Error> for ReconcilerError {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from(err: kube::Error) -> Self {
        Self::KubeClient(err)
    }
}

impl From<clevercloud::Error> for ReconcilerError {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from(err: clevercloud::Error) -> Self {
        Self::CleverClient(err)
    }
}

impl From<v2::addon::Error> for ReconcilerError {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from(err: v2::addon::Error) -> Self {
        Self::from(clevercloud::Error::from(err))
    }
}

impl From<v4::addon_provider::plan::Error> for ReconcilerError {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from(err: v4::addon_provider::plan::Error) -> Self {
        Self::from(clevercloud::Error::from(err))
    }
}

impl From<controller::Error<Self, watcher::Error>> for ReconcilerError {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from(err: controller::Error<ReconcilerError, watcher::Error>) -> Self {
        Self::Reconcile(err.to_string())
    }
}

// -----------------------------------------------------------------------------
// Reconciler structure

#[derive(Clone, Default, Debug)]
pub struct Reconciler {}

impl ControllerBuilder<Redis> for Reconciler {
    fn build(&self, state: Arc<Context>) -> Controller<Redis> {
        Controller::new(Api::all(state.kube.to_owned()), ListParams::default())
    }
}

#[async_trait]
impl k8s::Reconciler<Redis> for Reconciler {
    type Error = ReconcilerError;

    async fn upsert(ctx: Arc<Context>, origin: Arc<Redis>) -> Result<(), ReconcilerError> {
        let Context {
            kube,
            apis,
            config: _,
        } = ctx.as_ref();
        let kind = Redis::kind(&()).to_string();
        let (namespace, name) = resource::namespaced_name(&*origin);

        // ---------------------------------------------------------------------
        // Step 1: set finalizer

        info!(
            "Set finalizer on custom resource '{}' ('{}/{}')",
            &kind, &namespace, &name
        );
        let modified = finalizer::add((*origin).to_owned(), ADDON_FINALIZER);

        debug!(
            "Update information of custom resource '{}' ('{}/{}')",
            &kind, &namespace, &name
        );
        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        let mut modified = resource::patch(kube.to_owned(), &modified, patch).await?;

        let action = &Action::UpsertFinalizer;
        let message = &format!("Create finalizer '{}'", ADDON_FINALIZER);
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 2: translate plan

        if !modified.spec.instance.plan.starts_with("plan_") {
            info!(
                "Resolve plan for '{}' addon provider for resource '{}/{}' using '{}'",
                &kind, &namespace, &name, &modified.spec.instance.plan
            );
            let plan = plan::find(
                apis,
                &AddonProviderId::Redis,
                &modified.spec.organisation,
                &modified.spec.instance.plan,
            )
            .await?;

            // Update the spec is not a good practise as it lead to
            // no-deterministic and infinite reconciliation loop. It should be
            // avoided or done with caution.
            if let Some(plan) = plan {
                info!(
                    "Override plan for custom resource '{}' ('{}/{}') with plan '{}'",
                    &kind, &name, &namespace, &plan.id
                );
                let oplan = modified.spec.instance.plan.to_owned();
                modified.spec.instance.plan = plan.id.to_owned();

                debug!(
                    "Update information of custom resource '{}' ('{}/{}')",
                    &kind, &namespace, &name
                );
                let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
                let modified =
                    resource::patch(kube.to_owned(), &modified, patch.to_owned()).await?;

                let action = &Action::OverridesInstancePlan;
                let message = &format!("Overrides instance plan from '{}' to '{}'", oplan, plan.id);
                info!(
                    "Create '{}' event for resource '{}' ('{}/{}') with following message, {}",
                    action, &kind, &namespace, &name, message
                );
                recorder::normal(kube.to_owned(), &modified, action, message).await?;
            }

            // Stop reconciliation here and wait for next iteration, already
            // triggered by the above patch request
            return Ok(());
        }

        // ---------------------------------------------------------------------
        // Step 3: upsert addon

        info!(
            "Upsert addon for custom resource '{}' ('{}/{}')",
            &kind, &namespace, &name
        );
        let addon = modified.upsert(apis).await?;

        modified.set_addon_id(Some(addon.id.to_owned()));

        debug!(
            "Update information and status of custom resource '{}' ('{}/{}')",
            &kind, &namespace, &name
        );
        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        let modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
            .await?;

        let action = &Action::UpsertAddon;
        let message = &format!(
            "Create managed redis instance on clever-cloud '{}'",
            addon.id
        );
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 4: create the secret

        let secrets = modified.secrets(apis).await?;
        if let Some(secrets) = secrets {
            let s = secret::new(&modified, secrets);
            let (s_ns, s_name) = resource::namespaced_name(&s);

            info!(
                "Upsert kubernetes secret resource for custom resource '{}' ('{}/{}')",
                &kind, &namespace, &name
            );
            info!("Upsert kubernetes secret '{}/{}'", &s_ns, &s_name);
            let secret = resource::upsert(kube.to_owned(), &s, false).await?;

            let action = &Action::UpsertSecret;
            let message = &format!("Create kubernetes secret '{}'", secret.name());
            recorder::normal(kube.to_owned(), &modified, action, message).await?;
        }

        Ok(())
    }

    async fn delete(ctx: Arc<Context>, origin: Arc<Redis>) -> Result<(), ReconcilerError> {
        let Context {
            apis,
            kube,
            config: _,
        } = ctx.as_ref();
        let mut modified = (*origin).to_owned();
        let kind = Redis::kind(&()).to_string();
        let (namespace, name) = resource::namespaced_name(&*origin);

        // ---------------------------------------------------------------------
        // Step 1: delete the addon

        info!(
            "Delete addon for custom resource '{}' ('{}/{}')",
            &kind, &namespace, &name
        );
        modified.delete(apis).await?;
        modified.set_addon_id(None);

        debug!(
            "Update information and status of custom resource '{}' ('{}/{}')",
            &kind, &namespace, &name
        );
        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        let modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
            .await?;

        let action = &Action::DeleteAddon;
        let message = "Delete managed redis instance on clever-cloud";
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 2: remove the finalizer

        info!(
            "Remove finalizer on custom resource '{}' ('{}/{}')",
            &kind, &namespace, &name
        );
        let modified = finalizer::remove(modified, ADDON_FINALIZER);

        let action = &Action::DeleteFinalizer;
        let message = "Delete finalizer from custom resource";
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        debug!(
            "Update information of custom resource '{}' ('{}/{}')",
            &kind, &namespace, &name
        );
        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        resource::patch(kube.to_owned(), &modified, patch.to_owned()).await?;

        Ok(())
    }
}
