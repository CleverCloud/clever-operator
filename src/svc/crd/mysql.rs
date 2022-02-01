//! # MySql addon
//!
//! This module provide the mysql custom resource and its definition

use std::{
    fmt::{self, Display, Formatter},
    sync::Arc,
};

use async_trait::async_trait;
use clevercloud_sdk::{
    oauth10a::ClientError,
    v2::addon::{AddonOpts, CreateAddonOpts},
    v4::addon_provider::{mysql, plan, AddonProviderId},
};
use futures::TryFutureExt;
use kube::{api::ListParams, Api, Resource, ResourceExt};
use kube_derive::CustomResource;
use kube_runtime::{
    controller::{self, Context},
    watcher, Controller,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use slog_scope::{debug, error, info};

use crate::svc::{
    clevercloud::ext::AddonExt,
    crd::Instance,
    k8s::{self, finalizer, recorder, resource, secret, ControllerBuilder, State},
};

// -----------------------------------------------------------------------------
// Constants

pub const ADDON_FINALIZER: &str = "api.clever-cloud.com/mysql";

// -----------------------------------------------------------------------------
// MySqlOpts structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct MySqlOpts {
    #[serde(rename = "version")]
    pub version: mysql::Version,
    #[serde(rename = "encryption")]
    pub encryption: bool,
}

#[allow(clippy::from_over_into)]
impl Into<AddonOpts> for MySqlOpts {
    fn into(self) -> AddonOpts {
        AddonOpts {
            version: Some(self.version.to_string()),
            encryption: Some(self.encryption.to_string()),
        }
    }
}

// -----------------------------------------------------------------------------
// MySqlSpec structure

#[derive(CustomResource, JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug)]
#[kube(group = "api.clever-cloud.com")]
#[kube(version = "v1")]
#[kube(kind = "MySql")]
#[kube(singular = "mysql")]
#[kube(plural = "mysqls")]
#[kube(shortname = "my")]
#[kube(status = "MySqlStatus")]
#[kube(namespaced)]
#[kube(apiextensions = "v1")]
#[kube(derive = "PartialEq")]
pub struct MySqlSpec {
    #[serde(rename = "organisation")]
    pub organisation: String,
    #[serde(rename = "options")]
    pub options: MySqlOpts,
    #[serde(rename = "instance")]
    pub instance: Instance,
}

// -----------------------------------------------------------------------------
// MySqlStatus structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct MySqlStatus {
    #[serde(rename = "addon")]
    pub addon: Option<String>,
}

// -----------------------------------------------------------------------------
// MySql implementation

#[allow(clippy::from_over_into)]
impl Into<CreateAddonOpts> for MySql {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn into(self) -> CreateAddonOpts {
        CreateAddonOpts {
            name: AddonExt::name(&self),
            region: self.spec.instance.region.to_owned(),
            provider_id: AddonProviderId::MySql.to_string(),
            plan: self.spec.instance.plan.to_owned(),
            options: self.spec.options.into(),
        }
    }
}

impl AddonExt for MySql {
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
        "kubernetes_".to_string()
            + &self
                .uid()
                .expect("expect all resources in kubernetes to have an identifier")
    }
}

impl MySql {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    pub fn set_addon_id(&mut self, id: Option<String>) {
        let mut status = self.status.get_or_insert_with(MySqlStatus::default);

        status.addon = id;
        self.status = Some(status.to_owned());
    }

    #[cfg_attr(feature = "trace", tracing::instrument)]
    pub fn get_addon_id(&self) -> Option<String> {
        self.status.to_owned().unwrap_or_default().addon
    }
}

// -----------------------------------------------------------------------------
// MySqlAction structure

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub enum MySqlAction {
    UpsertFinalizer,
    UpsertAddon,
    UpsertSecret,
    OverridesInstancePlan,
    DeleteFinalizer,
    DeleteAddon,
}

impl Display for MySqlAction {
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
    CleverClient(ClientError),
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

impl From<ClientError> for ReconcilerError {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from(err: ClientError) -> Self {
        Self::CleverClient(err)
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

impl ControllerBuilder<MySql> for Reconciler {
    fn build(&self, state: State) -> Controller<MySql> {
        Controller::new(Api::all(state.kube), ListParams::default())
    }
}

#[async_trait]
impl k8s::Reconciler<MySql> for Reconciler {
    type Error = ReconcilerError;

    async fn upsert(ctx: &Context<State>, origin: Arc<MySql>) -> Result<(), ReconcilerError> {
        let State {
            kube,
            apis,
            config: _,
        } = ctx.get_ref();
        let kind = MySql::kind(&()).to_string();
        let (namespace, name) = resource::namespaced_name(&*origin);

        // ---------------------------------------------------------------------
        // Step 1: set finalizer

        info!("Set finalizer on custom resource"; "kind" => &kind, "uid" => &origin.meta().uid,"name" => &name, "namespace" => &namespace);
        let modified = finalizer::add((*origin).to_owned(), ADDON_FINALIZER);

        debug!("Update information of custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        let mut modified = resource::patch(kube.to_owned(), &modified, patch).await?;

        let action = &MySqlAction::UpsertFinalizer;
        let message = &format!("Create finalizer '{}'", ADDON_FINALIZER);
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 2: translate plan

        if !modified.spec.instance.plan.starts_with("plan_") {
            info!("Resolve plan for mysql addon provider"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace, "pattern" => &modified.spec.instance.plan);
            let plan = plan::find(
                apis,
                &AddonProviderId::MySql,
                &modified.spec.organisation,
                &modified.spec.instance.plan,
            )
            .await?;

            // Update the spec is not a good practise as it lead to
            // no-deterministic and infinite reconciliation loop. It should be
            // avoided or done with caution.
            if let Some(plan) = plan {
                info!("Override plan for custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace, "plan" => &plan.id);
                let oplan = modified.spec.instance.plan.to_owned();
                modified.spec.instance.plan = plan.id.to_owned();

                debug!("Update information of custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
                let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
                let modified =
                    resource::patch(kube.to_owned(), &modified, patch.to_owned()).await?;

                let action = &MySqlAction::OverridesInstancePlan;
                let message = &format!("Overrides instance plan from '{}' to '{}'", oplan, plan.id);
                info!("Create '{}' event for resource", action; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace, "message" => message);
                recorder::normal(kube.to_owned(), &modified, action, message).await?;
            }

            // Stop reconciliation here and wait for next iteration, already
            // triggered by the above patch request
            return Ok(());
        }

        // ---------------------------------------------------------------------
        // Step 3: upsert addon

        info!("Upsert addon for custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let addon = modified.upsert(apis).await?;

        modified.set_addon_id(Some(addon.id.to_owned()));

        debug!("Update information and status of custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        let modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
            .await?;

        let action = &MySqlAction::UpsertAddon;
        let message = &format!(
            "Create managed mysql instance on clever-cloud '{}'",
            addon.id
        );
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 4: create the secret

        let secrets = modified.secrets(apis).await?;
        if let Some(secrets) = secrets {
            let s = secret::new(&modified, secrets);
            let (s_ns, s_name) = resource::namespaced_name(&s);

            info!("Upsert kubernetes secret resource for custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
            info!("Upsert kubernetes secret"; "kind" => "Secret", "name" => &s_name, "namespace" => &s_ns);
            let secret = resource::upsert(kube.to_owned(), &s, false).await?;

            let action = &MySqlAction::UpsertSecret;
            let message = &format!("Create kubernetes secret '{}'", secret.name());
            recorder::normal(kube.to_owned(), &modified, action, message).await?;
        }

        Ok(())
    }

    async fn delete(ctx: &Context<State>, origin: Arc<MySql>) -> Result<(), ReconcilerError> {
        let State {
            apis,
            kube,
            config: _,
        } = ctx.get_ref();
        let mut modified = (*origin).to_owned();
        let kind = MySql::kind(&()).to_string();
        let (namespace, name) = resource::namespaced_name(&*origin);

        // ---------------------------------------------------------------------
        // Step 1: delete the addon

        info!("Delete addon for custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        modified.delete(apis).await?;
        modified.set_addon_id(None);

        debug!("Update information and status of custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        let modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
            .await?;

        let action = &MySqlAction::DeleteAddon;
        let message = "Delete managed mysql instance on clever-cloud";
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 2: remove the finalizer

        info!("Remove finalizer on custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let modified = finalizer::remove(modified, ADDON_FINALIZER);

        let action = &MySqlAction::DeleteFinalizer;
        let message = "Delete finalizer from custom resource";
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        debug!("Update information of custom resource"; "kind" => &kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        resource::patch(kube.to_owned(), &modified, patch.to_owned()).await?;

        Ok(())
    }
}
