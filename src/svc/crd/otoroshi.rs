//! # Otoroshi addon
//!
//! This module provide the otoroshi custom resource and its definition

use std::{
    fmt::{self, Display, Formatter},
    sync::Arc,
};

use clevercloud_sdk::{
    v2::{
        self,
        addon::{self, CreateOpts},
        plan,
    },
    v4::addon_provider::AddonProviderId,
};
use futures::TryFutureExt;
use k8s_openapi::api::core::v1::Secret;
use kube::{
    Api, CustomResource, Resource, ResourceExt,
    runtime::{Controller, controller, watcher},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::svc::{
    clevercloud::{self, ext::AddonExt},
    crd::Instance,
    k8s::{
        self, Context, ControllerBuilder, finalizer, recorder, resource,
        secret::{self, OVERRIDE_CONFIGURATION_NAME},
    },
};

// -----------------------------------------------------------------------------
// Constants

pub const ADDON_FINALIZER: &str = "api.clever-cloud.com/otoroshi";

// -----------------------------------------------------------------------------
// Opts structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct Opts {}

#[allow(clippy::from_over_into)]
impl Into<addon::Opts> for Opts {
    fn into(self) -> addon::Opts {
        addon::Opts::default()
    }
}

// -----------------------------------------------------------------------------
// Spec structure

#[derive(CustomResource, JsonSchema, Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
#[kube(group = "api.clever-cloud.com")]
#[kube(version = "v1")]
#[kube(kind = "Otoroshi")]
#[kube(singular = "otoroshi")]
#[kube(plural = "otoroshis")]
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
#[kube(
    printcolumn = r#"{"name":"instance", "type":"string", "description":"Instance", "jsonPath":".spec.instance.plan"}"#
)]
#[kube(
    printcolumn = r#"{"name":"url", "type":"string", "description":"Url", "jsonPath":".status.url"}"#
)]
pub struct Spec {
    #[serde(rename = "organisation")]
    pub organisation: String,
    #[serde(default, rename = "options")]
    pub options: Opts,
    #[serde(rename = "instance")]
    pub instance: Instance,
}

// -----------------------------------------------------------------------------
// Otoroshi Status structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct Status {
    #[serde(rename = "addon")]
    pub addon: Option<String>,
    #[serde(rename = "url")]
    pub url: Option<String>,
}

// -----------------------------------------------------------------------------
// Otoroshi implementation

#[allow(clippy::from_over_into)]
impl Into<CreateOpts> for Otoroshi {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn into(self) -> CreateOpts {
        CreateOpts {
            name: AddonExt::name(&self),
            region: self.spec.instance.region.to_owned(),
            provider_id: AddonProviderId::Otoroshi.to_string(),
            plan: self.spec.instance.plan.to_owned(),
            options: self.spec.options.into(),
        }
    }
}

impl AddonExt for Otoroshi {
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

impl Otoroshi {
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

    /// Sets the URL of the otoroshi instance (`CC_OTOROSHI_URL` secret)
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn set_url(&mut self, url: Option<String>) {
        let status = self.status.get_or_insert_with(Status::default);

        status.url = url;
        self.status = Some(status.to_owned());
    }

    /// Returns the URL of the otoroshi instance (`CC_OTOROSHI_URL` secret)
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn get_url(&self) -> Option<String> {
        self.status.to_owned().unwrap_or_default().url
    }
}

// -----------------------------------------------------------------------------
// Action structure

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub enum Action {
    UpsertFinalizer,
    UpsertAddon,
    UpsertSecret,
    UpdateUrl,
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
            Self::UpdateUrl => write!(f, "UpdateUrl"),
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

impl From<v2::plan::Error> for ReconcilerError {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: v2::plan::Error) -> Self {
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

impl ControllerBuilder<Otoroshi> for Reconciler {
    fn build(&self, state: Arc<Context>) -> Controller<Otoroshi> {
        let client = state.kube.to_owned();
        let secret = Api::<Secret>::all(client.to_owned());

        Controller::new(Api::all(client), watcher::Config::default())
            .owns(secret, watcher::Config::default())
    }
}

impl k8s::Reconciler<Otoroshi> for Reconciler {
    type Error = ReconcilerError;

    async fn upsert(ctx: Arc<Context>, origin: Arc<Otoroshi>) -> Result<(), ReconcilerError> {
        let Context {
            kube,
            apis,
            config: _,
        } = ctx.as_ref();

        let kind = Otoroshi::kind(&()).to_string();
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
        // Step 2: translate plan

        if !modified.spec.instance.plan.starts_with("plan_") {
            info!(
                kind = &kind,
                namespace = &namespace,
                name = &name,
                plan = &modified.spec.instance.plan,
                "Resolve plan for resource'",
            );

            let plan = plan::find(
                &apis,
                &AddonProviderId::Otoroshi,
                &modified.spec.instance.plan,
            )
            .await?;

            // Update the spec is not a good practice as it lead to
            // no-deterministic and infinite reconciliation loop. It should be
            // avoided or done with caution.
            if let Some(plan) = plan {
                info!(
                    kind = &kind,
                    namespace = &namespace,
                    name = &name,
                    plan = &plan.id,
                    "Override plan for custom resource",
                );

                let oplan = modified.spec.instance.plan.to_owned();
                plan.id.clone_into(&mut modified.spec.instance.plan);

                debug!(
                    kind = &kind,
                    namespace = &namespace,
                    name = &name,
                    "Update information of custom resource",
                );

                let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
                let modified =
                    resource::patch(kube.to_owned(), &modified, patch.to_owned()).await?;

                let action = &Action::OverridesInstancePlan;
                let message = &format!("Overrides instance plan from '{}' to '{}'", oplan, plan.id);

                info!(
                    action = action.to_string(),
                    kind = &kind,
                    namespace = &namespace,
                    name = &name,
                    message = message,
                    "Create event for custom resource",
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
        let mut modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
            .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
            .await?;

        let action = &Action::UpsertAddon;
        let message = &format!(
            "Create managed otoroshi instance on clever-cloud '{}'",
            addon.id
        );
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 4: upsert secret

        let secrets = modified.secrets(&apis).await?;

        // capture the url to update the status un Step 5
        let url = match &secrets {
            None => None,
            Some(secrets) => match secrets.get("CC_OTOROSHI_URL") {
                Some(secret) if !secret.is_empty() => Some(secret.to_string()),
                _ => None,
            },
        };

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

        // ---------------------------------------------------------------------
        // Step 5: update the status

        if let Some(url) = url {
            if modified.get_url().as_deref() != Some(url.as_str()) {
                let action = &Action::UpdateUrl;
                let message = &format!("Update url to '{url}'");

                modified.set_url(Some(url));

                let patch = resource::diff(&*origin, &modified).map_err(ReconcilerError::Diff)?;
                let modified = resource::patch(kube.to_owned(), &modified, patch.to_owned())
                    .and_then(|modified| resource::patch_status(kube.to_owned(), modified, patch))
                    .await?;

                info!(
                    action = action.to_string(),
                    kind = &kind,
                    namespace = &namespace,
                    name = &name,
                    message = message,
                    "Create event for custom resource",
                );

                recorder::normal(kube.to_owned(), &modified, action, message).await?;
            }
        }

        Ok(())
    }

    async fn delete(ctx: Arc<Context>, origin: Arc<Otoroshi>) -> Result<(), ReconcilerError> {
        let Context {
            apis,
            kube,
            config: _,
        } = ctx.as_ref();
        let mut modified = (*origin).to_owned();
        let kind = Otoroshi::kind(&()).to_string();
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
        modified.set_url(None);

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
        let message = "Delete managed otoroshi instance on clever-cloud";
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
