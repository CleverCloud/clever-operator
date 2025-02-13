//! # Materia Key-Value addon
//!
//! This module provides the materia key-value custom resource and its definition

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
    v4::{self, addon_provider::AddonProviderId},
};
use futures::TryFutureExt;
use k8s_openapi::api::core::v1::Secret;
use kube::{
    runtime::{controller, watcher, Controller},
    Api, CustomResource, Resource, ResourceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::svc::{
    clevercloud::{self, ext::AddonExt},
    k8s::{
        self, finalizer, recorder, resource,
        secret::{self, OVERRIDE_CONFIGURATION_NAME},
        Context, ControllerBuilder,
    },
};

// -----------------------------------------------------------------------------
// Constants

pub const ADDON_FINALIZER: &str = "api.clever-cloud.com/materia-kv";
pub const ADDON_ALPHA_PLAN: &str = "plan_53a1728d-4b9e-4254-94c4-b19163af587b";

// -----------------------------------------------------------------------------
// Instance structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Instance {
    #[serde(rename = "region")]
    pub region: String,
}

// -----------------------------------------------------------------------------
// Spec structure

#[derive(CustomResource, JsonSchema, Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
#[kube(group = "api.clever-cloud.com")]
#[kube(version = "v1alpha1")]
#[kube(kind = "KV")]
#[kube(singular = "kv")]
#[kube(plural = "kvs")]
#[kube(status = "Status")]
#[kube(namespaced)]
#[kube(derive = "PartialEq")]
#[kube(
    printcolumn = r#"{"name":"organisation", "type":"string", "description":"Organisation", "jsonPath":".spec.organisation"}"#
)]
#[kube(
    printcolumn = r#"{"name":"addon", "type":"string", "description":"Addon", "jsonPath":".status.addon"}"#
)]
#[kube(
    printcolumn = r#"{"name":"region", "type":"string", "description":"Region", "jsonPath":".spec.instance.region"}"#
)]
pub struct Spec {
    #[serde(rename = "organisation")]
    pub organisation: String,
    #[serde(rename = "instance")]
    pub instance: Instance,
}

// -----------------------------------------------------------------------------
// Status structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct Status {
    #[serde(rename = "addon")]
    pub addon: Option<String>,
}

// -----------------------------------------------------------------------------
// Pulsar implementation

#[allow(clippy::from_over_into)]
impl Into<CreateOpts> for KV {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn into(self) -> CreateOpts {
        CreateOpts {
            name: AddonExt::name(&self),
            region: self.spec.instance.region.to_owned(),
            provider_id: AddonProviderId::KV.to_string(),
            plan: ADDON_ALPHA_PLAN.to_string(),
            options: addon::Opts::default(),
        }
    }
}

impl AddonExt for KV {
    type Error = ReconcilerError;

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn id(&self) -> Option<String> {
        if let Some(status) = &self.status {
            return status.addon.to_owned();
        }

        None
    }

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn organisation(&self) -> String {
        self.spec.organisation.to_owned()
    }

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn name(&self) -> String {
        let delimiter = Self::delimiter();

        Self::prefix()
            + &delimiter
            + &Self::kind(&())
            + &delimiter
            + &self
                .uid()
                .expect("expect all resources in kubernetes to have an identifier")
    }
}

impl KV {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn set_addon_id(&mut self, id: Option<String>) {
        let status = self.status.get_or_insert_with(Status::default);

        status.addon = id;
        self.status = Some(status.to_owned());
    }

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn get_addon_id(&self) -> Option<String> {
        self.status.to_owned().unwrap_or_default().addon
    }
}

// -----------------------------------------------------------------------------
// Action structure

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub enum Action {
    UpsertFinalizer,
    UpsertAddon,
    UpsertSecret,
    DeleteFinalizer,
    DeleteAddon,
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::UpsertFinalizer => write!(f, "UpsertFinalizer"),
            Self::UpsertAddon => write!(f, "UpsertAddon"),
            Self::UpsertSecret => write!(f, "UpsertSecret"),
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
    #[error("failed to create clevercloud client, {0}")]
    CreateCleverClient(clevercloud::client::Error),
    #[error("failed to execute request on kubernetes api, {0}")]
    KubeClient(kube::Error),
    #[error("failed to compute diff between the original and modified object, {0}")]
    Diff(serde_json::Error),
}

impl From<kube::Error> for ReconcilerError {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: kube::Error) -> Self {
        Self::KubeClient(err)
    }
}

impl From<clevercloud::Error> for ReconcilerError {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: clevercloud::Error) -> Self {
        Self::CleverClient(err)
    }
}

impl From<v2::addon::Error> for ReconcilerError {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: v2::addon::Error) -> Self {
        Self::from(clevercloud::Error::from(err))
    }
}

impl From<v4::addon_provider::plan::Error> for ReconcilerError {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: v4::addon_provider::plan::Error) -> Self {
        Self::from(clevercloud::Error::from(err))
    }
}

impl From<controller::Error<Self, watcher::Error>> for ReconcilerError {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: controller::Error<ReconcilerError, watcher::Error>) -> Self {
        Self::Reconcile(err.to_string())
    }
}

impl From<clevercloud::client::Error> for ReconcilerError {
    fn from(err: clevercloud::client::Error) -> Self {
        Self::CreateCleverClient(err)
    }
}

// -----------------------------------------------------------------------------
// Reconciler structure

#[derive(Clone, Default, Debug)]
pub struct Reconciler {}

impl ControllerBuilder<KV> for Reconciler {
    fn build(&self, state: Arc<Context>) -> Controller<KV> {
        let client = state.kube.to_owned();
        let secret = Api::<Secret>::all(client.to_owned());

        Controller::new(Api::all(client), watcher::Config::default())
            .owns(secret, watcher::Config::default())
    }
}

#[async_trait]
impl k8s::Reconciler<KV> for Reconciler {
    type Error = ReconcilerError;

    async fn upsert(ctx: Arc<Context>, origin: Arc<KV>) -> Result<(), ReconcilerError> {
        let Context {
            kube,
            apis,
            config: _,
        } = ctx.as_ref();

        let kind = KV::kind(&()).to_string();
        let (namespace, name) = resource::namespaced_name(&*origin);

        // ---------------------------------------------------------------------
        // Step 0: verify if there is a clever cloud client override
        debug!(
            namespace = namespace,
            secret = OVERRIDE_CONFIGURATION_NAME,
            "Try to retrieve the optional secret on namespace",
        );

        let secret: Option<Secret> =
            resource::get(kube.to_owned(), &namespace, OVERRIDE_CONFIGURATION_NAME).await?;
        let apis = match secret {
            Some(secret) => {
                info!(
                    namespace = namespace,
                    secret = OVERRIDE_CONFIGURATION_NAME,
                    "Use custom Clever Cloud client to connect the api using secret",
                );

                clevercloud::client::try_from(secret).await?
            }
            None => {
                info!("Use default Clever Cloud client to connect the api");
                apis.to_owned()
            }
        };

        // ---------------------------------------------------------------------
        // Step 1: set finalizer

        info!(
            kind = &kind,
            namespace = &namespace,
            name = &name,
            "Set finalizer on custom resource",
        );

        let modified = finalizer::add((*origin).to_owned(), ADDON_FINALIZER);

        debug!(
            kind = &kind,
            namespace = &namespace,
            name = &name,
            "Update information of custom resource",
        );

        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        let mut modified = resource::patch(kube.to_owned(), &modified, patch).await?;

        let action = &Action::UpsertFinalizer;
        let message = &format!("Create finalizer '{}'", ADDON_FINALIZER);
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 2:

        // This is not the step that you are looking for.

        // ---------------------------------------------------------------------
        // Step 3: upsert addon

        info!(
            kind = &kind,
            namespace = &namespace,
            name = &name,
            "Upsert addon for custom resource",
        );

        let addon = modified.upsert(&apis).await?;

        modified.set_addon_id(Some(addon.id.to_owned()));

        debug!(
            kind = &kind,
            namespace = &namespace,
            name = &name,
            "Update information and status of custom resource",
        );

        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        let modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
            .await?;

        let action = &Action::UpsertAddon;
        let message = &format!(
            "Create managed pulsar instance on clever-cloud '{}'",
            addon.id
        );
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 4: create the secret

        let secrets = modified.secrets(&apis).await?;
        if let Some(secrets) = secrets {
            let s = secret::new(&modified, secrets);
            let (s_ns, s_name) = resource::namespaced_name(&s);

            info!(
                kind = &kind,
                namespace = &namespace,
                name = &name,
                "Upsert kubernetes secret resource for custom resource",
            );

            info!(
                namespace = &s_ns,
                name = &s_name,
                "Upsert kubernetes secret",
            );

            let secret = resource::upsert(kube.to_owned(), &s, false).await?;

            let action = &Action::UpsertSecret;
            let message = &format!("Create kubernetes secret '{}'", secret.name_any());
            recorder::normal(kube.to_owned(), &modified, action, message).await?;
        }

        Ok(())
    }

    async fn delete(ctx: Arc<Context>, origin: Arc<KV>) -> Result<(), ReconcilerError> {
        let Context {
            apis,
            kube,
            config: _,
        } = ctx.as_ref();

        let mut modified = (*origin).to_owned();
        let kind = KV::kind(&()).to_string();
        let (namespace, name) = resource::namespaced_name(&*origin);

        // ---------------------------------------------------------------------
        // Step 0: verify if there is a clever cloud client override
        debug!(
            namespace = namespace,
            secret = OVERRIDE_CONFIGURATION_NAME,
            "Try to retrieve the optional secret",
        );

        let secret: Option<Secret> =
            resource::get(kube.to_owned(), &namespace, OVERRIDE_CONFIGURATION_NAME).await?;

        let apis = match secret {
            Some(secret) => {
                info!(
                    namespace = namespace,
                    secret = OVERRIDE_CONFIGURATION_NAME,
                    "Use custom Clever Cloud client to connect the api using secret",
                );

                clevercloud::client::try_from(secret).await?
            }
            None => {
                info!("Use default Clever Cloud client to connect the api");
                apis.to_owned()
            }
        };

        // ---------------------------------------------------------------------
        // Step 1: delete the addon

        info!(
            kind = &kind,
            namespace = &namespace,
            name = &name,
            "Delete addon for custom resource",
        );

        modified.delete(&apis).await?;
        modified.set_addon_id(None);

        debug!(
            kind = &kind,
            namespace = &namespace,
            name = &name,
            "Update information and status of custom resource",
        );

        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        let modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
            .await?;

        let action = &Action::DeleteAddon;
        let message = "Delete managed pulsar instance on clever-cloud";
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 2: remove the finalizer

        info!(
            kind = &kind,
            namespace = &namespace,
            name = &name,
            "Remove finalizer on custom resource",
        );

        let modified = finalizer::remove(modified, ADDON_FINALIZER);

        let action = &Action::DeleteFinalizer;
        let message = "Delete finalizer from custom resource";
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        debug!(
            kind = &kind,
            namespace = &namespace,
            name = &name,
            "Update information of custom resource",
        );

        let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
        resource::patch(kube.to_owned(), &modified, patch.to_owned()).await?;

        Ok(())
    }
}
