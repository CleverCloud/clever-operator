//! # PostgreSQL addon
//!
//! This module provide the postgresql custom resource and its definition

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
use slog_scope::{error, info};

use crate::svc::{
    apis::{addon::provider::PostgreSqlVersion, ClientError},
    k8s::{
        self,
        addon::{AddonExt, Instance},
        finalizer, resource, secret, ControllerBuilder, State,
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
    pub version: PostgreSqlVersion,
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
        let state = ctx.get_ref();
        let (namespace, name) = resource::namespaced_name(origin);

        // ---------------------------------------------------------------------
        // Step 1: set finalizer
        info!("Set finalizer on custom resource"; "kind" => &origin.kind, "uid" => &origin.meta().uid,"name" => &name, "namespace" => &namespace);
        let mut modified = finalizer::add(origin.to_owned(), POSTGRESQL_ADDON_FINALIZER);

        // we could defer the patch request as this step as no side effects

        // ---------------------------------------------------------------------
        // Step 2: create the addon
        info!("Upsert addon for custom resource"; "kind" => &origin.kind, "uid" => &origin.meta().uid,"name" => &name, "namespace" => &namespace);
        let addon = modified
            .upsert(state.config.to_owned(), &state.apis)
            .await?;

        modified.set_addon_id(Some(addon.id));

        info!("Update information and status of custom resource"; "kind" => &origin.kind, "uid" => &origin.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(origin, &modified).map_err(ReconcilerError::Diff)?;
        let modified = resource::patch(state.kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(state.kube.to_owned(), modified, patch))
            .await?;

        // ---------------------------------------------------------------------
        // Step 3: create the secret
        let secrets = modified
            .secrets(state.config.to_owned(), &state.apis)
            .await?;

        if let Some(secrets) = secrets {
            let s = secret::new(&modified, secrets);
            let (s_ns, s_name) = resource::namespaced_name(&s);

            info!("Upsert kubernetes secret resource for custom resource"; "kind" => &origin.kind, "uid" => &origin.meta().uid,"name" => &name, "namespace" => &namespace);
            info!("Upsert kubernetes secret"; "kind" => "Secret", "name" => &s_name, "namespace" => &s_ns);
            secret::upsert(state.kube.to_owned(), &modified, &s).await?;
        }

        Ok(())
    }

    async fn delete(ctx: &Context<State>, origin: &PostgreSql) -> Result<(), ReconcilerError> {
        let state = ctx.get_ref();
        let mut modified = origin.to_owned();
        let (namespace, name) = resource::namespaced_name(origin);

        // ---------------------------------------------------------------------
        // Step 1: delete the addon
        info!("Delete addon for custom resource"; "kind" => &origin.kind, "uid" => &origin.meta().uid,"name" => &name, "namespace" => &namespace);
        modified
            .delete(state.config.to_owned(), &state.apis)
            .await?;

        modified.set_addon_id(None);

        // we could defer the patch request as the next step cannot failed

        // ---------------------------------------------------------------------
        // Step 2: remove the finalizer

        info!("Remove finalizer on custom resource"; "kind" => &origin.kind, "uid" => &origin.meta().uid,"name" => &name, "namespace" => &namespace);
        let modified = finalizer::remove(modified, POSTGRESQL_ADDON_FINALIZER);

        info!("Update information of custom resource"; "kind" => &origin.kind, "uid" => &origin.meta().uid,"name" => &name, "namespace" => &namespace);
        let patch = resource::diff(origin, &modified).map_err(ReconcilerError::Diff)?;
        resource::patch(state.kube.to_owned(), &modified, patch.to_owned()).await?;

        Ok(())
    }
}
