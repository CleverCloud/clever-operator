//! # PostgreSQL addon
//!
//! This module provide the postgresql custom resource and its definition

use std::fmt::{self, Display, Formatter};

use async_trait::async_trait;
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
    apis::{addon::provider::postgresql, ClientError},
    k8s::{
        self,
        addon::{AddonExt, Instance},
        finalizer, recorder, resource, secret, ControllerBuilder, State,
    },
};

// -----------------------------------------------------------------------------
// Constants

pub const POSTGRESQL_ADDON_FINALIZER: &str = "api.clever-cloud.com/postgresql";

// -----------------------------------------------------------------------------
// PostgreSqlOptions structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct PostgreSqlOpts {
    #[serde(rename = "version")]
    pub version: postgresql::Version,
    #[serde(rename = "encryption")]
    pub encryption: bool,
}

// -----------------------------------------------------------------------------
// PostgreSqlSpec structure

#[derive(CustomResource, JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug)]
#[kube(group = "api.clever-cloud.com")]
#[kube(version = "v1")]
#[kube(kind = "PostgreSql")]
#[kube(singular = "postgresql")]
#[kube(plural = "postgresqls")]
#[kube(shortname = "pg")]
#[kube(status = "PostgreSqlStatus")]
#[kube(namespaced)]
#[kube(apiextensions = "v1")]
#[kube(derive = "PartialEq")]
pub struct PostgreSqlSpec {
    #[serde(rename = "organisation")]
    pub organisation: String,
    #[serde(rename = "options")]
    pub options: PostgreSqlOpts,
    #[serde(rename = "instance")]
    pub instance: Instance,
}

// -----------------------------------------------------------------------------
// PostgreSQLStatus structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct PostgreSqlStatus {
    #[serde(rename = "addon")]
    pub addon: Option<String>,
}

// -----------------------------------------------------------------------------
// PostgreSql implementation

impl AddonExt for PostgreSql {
    type Error = ReconcilerError;

    fn id(&self) -> Option<String> {
        if let Some(status) = &self.status {
            return status.addon.to_owned();
        }

        None
    }

    fn organisation(&self) -> String {
        self.spec.organisation.to_owned()
    }

    fn name(&self) -> String {
        self.uid()
            .expect("expect all resources in kubernetes to have an identifier")
    }
}

impl PostgreSql {
    pub fn set_addon_id(&mut self, id: Option<String>) {
        let mut status = self.status.get_or_insert_with(PostgreSqlStatus::default);

        status.addon = id;
        self.status = Some(status.to_owned());
    }

    pub fn get_addon_id(&self) -> Option<String> {
        self.status
            .to_owned()
            .unwrap_or_else(PostgreSqlStatus::default)
            .addon
    }
}

// -----------------------------------------------------------------------------
// PostgreSqlAction structure

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub enum PostgreSqlAction {
    UpsertFinalizer,
    UpsertAddon,
    UpsertSecret,
    OverridesInstancePlan,
    DeleteFinalizer,
    DeleteAddon,
}

impl Display for PostgreSqlAction {
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
    #[error("requeue is needed")]
    Requeue,
}

impl From<kube::Error> for ReconcilerError {
    fn from(err: kube::Error) -> Self {
        Self::KubeClient(err)
    }
}

impl From<ClientError> for ReconcilerError {
    fn from(err: ClientError) -> Self {
        Self::CleverClient(err)
    }
}

impl From<controller::Error<Self, watcher::Error>> for ReconcilerError {
    fn from(err: controller::Error<ReconcilerError, watcher::Error>) -> Self {
        Self::Reconcile(err.to_string())
    }
}

// -----------------------------------------------------------------------------
// Reconciler structure

#[derive(Clone, Default)]
pub struct Reconciler {}

impl ControllerBuilder<PostgreSql> for Reconciler {
    fn build(&self, state: State) -> Controller<PostgreSql> {
        Controller::new(Api::all(state.kube), ListParams::default())
    }
}

#[async_trait]
impl k8s::Reconciler<PostgreSql> for Reconciler {
    type Error = ReconcilerError;

    async fn upsert(ctx: &Context<State>, origin: &PostgreSql) -> Result<(), ReconcilerError> {
        let State { kube, apis, config } = ctx.get_ref();
        let (namespace, name) = resource::namespaced_name(origin);

        // ---------------------------------------------------------------------
        // Step 1: set finalizer

        info!("Set finalizer on custom resource"; "kind" => &origin.kind, "uid" => &origin.meta().uid,"name" => &name, "namespace" => &namespace);
        let modified = finalizer::add(origin.to_owned(), POSTGRESQL_ADDON_FINALIZER);

        debug!("Update information of custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(origin, &modified).map_err(ReconcilerError::Diff)?;
        let mut modified = resource::patch(kube.to_owned(), &modified, patch).await?;

        let action = &PostgreSqlAction::UpsertFinalizer;
        let message = &format!("Create finalizer '{}'", POSTGRESQL_ADDON_FINALIZER);
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 2: translate plan

        if !modified.spec.instance.plan.starts_with("plan_") {
            info!("Resolve plan for postgresql addon provider"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace, "pattern" => &modified.spec.instance.plan);
            let plan = postgresql::plan::find(
                config.to_owned(),
                apis,
                &modified.spec.organisation,
                &modified.spec.instance.plan,
            )
            .await?;

            // Update the spec is not a good practise as it lead to
            // no-deterministic and infinite reconciliation loop. It should be
            // avoided or done with caution.
            if let Some(plan) = plan {
                info!("Override plan for custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace, "plan" => &plan.id);
                let oplan = modified.spec.instance.plan.to_owned();
                modified.spec.instance.plan = plan.id.to_owned();

                debug!("Update information of custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
                let patch = resource::diff(origin, &modified).map_err(ReconcilerError::Diff)?;
                let modified =
                    resource::patch(kube.to_owned(), &modified, patch.to_owned()).await?;

                let action = &PostgreSqlAction::OverridesInstancePlan;
                let message = &format!("Overrides instance plan from '{}' to '{}'", oplan, plan.id);
                info!("Create '{}' event for resource", action; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace, "message" => message);
                recorder::normal(kube.to_owned(), &modified, action, message).await?;
            }

            // require a requeue
            return Err(ReconcilerError::Requeue);
        }

        // ---------------------------------------------------------------------
        // Step 3: upsert addon

        info!("Upsert addon for custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let addon = modified.upsert(config.to_owned(), apis).await?;

        modified.set_addon_id(Some(addon.id.to_owned()));

        debug!("Update information and status of custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(origin, &modified).map_err(ReconcilerError::Diff)?;
        let modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
            .await?;

        let action = &PostgreSqlAction::UpsertAddon;
        let message = &format!(
            "Create managed postgresql instance on clever-cloud '{}'",
            addon.id
        );
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 4: create the secret

        let secrets = modified.secrets(config.to_owned(), apis).await?;

        if let Some(secrets) = secrets {
            let s = secret::new(&modified, secrets);
            let (s_ns, s_name) = resource::namespaced_name(&s);

            info!("Upsert kubernetes secret resource for custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
            info!("Upsert kubernetes secret"; "kind" => "Secret", "name" => &s_name, "namespace" => &s_ns);
            let secret = resource::upsert(kube.to_owned(), &s, false).await?;

            let action = &PostgreSqlAction::UpsertSecret;
            let message = &format!("Create kubernetes secret '{}'", secret.name());
            recorder::normal(kube.to_owned(), &modified, action, message).await?;
        }

        Ok(())
    }

    async fn delete(ctx: &Context<State>, origin: &PostgreSql) -> Result<(), ReconcilerError> {
        let State { apis, kube, config } = ctx.get_ref();
        let mut modified = origin.to_owned();
        let (namespace, name) = resource::namespaced_name(origin);

        // ---------------------------------------------------------------------
        // Step 1: delete the addon

        info!("Delete addon for custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        modified.delete(config.to_owned(), apis).await?;
        modified.set_addon_id(None);

        debug!("Update information and status of custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(origin, &modified).map_err(ReconcilerError::Diff)?;
        let modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
            .await?;

        let action = &PostgreSqlAction::DeleteAddon;
        let message = "Delete managed postgresql instance on clever-cloud";
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 2: remove the finalizer

        info!("Remove finalizer on custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let modified = finalizer::remove(modified, POSTGRESQL_ADDON_FINALIZER);

        let action = &PostgreSqlAction::DeleteFinalizer;
        let message = "Delete finalizer from custom resource";
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        debug!("Update information of custom resource"; "kind" => &modified.kind, "uid" => &modified.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(origin, &modified).map_err(ReconcilerError::Diff)?;
        resource::patch(kube.to_owned(), &modified, patch.to_owned()).await?;

        Ok(())
    }
}
