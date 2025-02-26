//! # ConfigProvider addon
//!
//! This module provide the configuration custom resource and its definition

use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
    sync::Arc,
};

use async_trait::async_trait;
use clevercloud_sdk::{
    v2::{
        self,
        addon::{self, CreateOpts},
    },
    v4::addon_provider::{
        AddonProviderId,
        config_provider::addon::environment::{self, Variable},
        plan,
    },
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
    k8s::{
        self, Context, ControllerBuilder, finalizer, recorder, resource,
        secret::{self, OVERRIDE_CONFIGURATION_NAME},
    },
};

// -----------------------------------------------------------------------------
// Constants

pub const ADDON_FINALIZER: &str = "api.clever-cloud.com/config-provider";

// -----------------------------------------------------------------------------
// MySqlSpec structure

#[derive(CustomResource, JsonSchema, Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
#[kube(group = "api.clever-cloud.com")]
#[kube(version = "v1")]
#[kube(kind = "ConfigProvider")]
#[kube(singular = "configprovider")]
#[kube(plural = "configproviders")]
#[kube(shortname = "cp")]
#[kube(status = "Status")]
#[kube(namespaced)]
#[kube(derive = "PartialEq")]
#[kube(
    printcolumn = r#"{"name":"organisation", "type":"string", "description":"Organisation", "jsonPath":".spec.organisation"}"#
)]
#[kube(
    printcolumn = r#"{"name":"addon", "type":"string", "description":"Addon", "jsonPath":".status.addon"}"#
)]
pub struct Spec {
    #[serde(rename = "organisation")]
    pub organisation: String,
    #[serde(rename = "variables")]
    pub variables: BTreeMap<String, String>,
}

// -----------------------------------------------------------------------------
// MySqlStatus structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct Status {
    #[serde(rename = "addon")]
    pub addon: Option<String>,
}

// -----------------------------------------------------------------------------
// MySql implementation

#[allow(clippy::from_over_into)]
impl Into<CreateOpts> for ConfigProvider {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn into(self) -> CreateOpts {
        CreateOpts {
            name: AddonExt::name(&self),
            region: "par".to_owned(), // config provider is only available in the "par" datacenter
            provider_id: AddonProviderId::ConfigProvider.to_string(),
            plan: plan::CONFIG_PROVIDER.to_owned(),
            options: addon::Opts::default(),
        }
    }
}

impl AddonExt for ConfigProvider {
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

impl ConfigProvider {
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
// MySqlAction structure

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

impl From<plan::Error> for ReconcilerError {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: plan::Error) -> Self {
        Self::from(clevercloud::Error::from(err))
    }
}

impl From<environment::Error> for ReconcilerError {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(err: environment::Error) -> Self {
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

impl ControllerBuilder<ConfigProvider> for Reconciler {
    fn build(&self, state: Arc<Context>) -> Controller<ConfigProvider> {
        let client = state.kube.to_owned();
        let secret = Api::<Secret>::all(client.to_owned());

        Controller::new(Api::all(client), watcher::Config::default())
            .owns(secret, watcher::Config::default())
    }
}

#[async_trait]
impl k8s::Reconciler<ConfigProvider> for Reconciler {
    type Error = ReconcilerError;

    async fn upsert(ctx: Arc<Context>, origin: Arc<ConfigProvider>) -> Result<(), ReconcilerError> {
        let Context {
            kube,
            apis,
            config: _,
        } = ctx.as_ref();

        let kind = ConfigProvider::kind(&()).to_string();
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
        // Step 2: upsert addon
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
            "Create configuration provider on clever-cloud '{}'",
            addon.id
        );
        recorder::normal(kube.to_owned(), &modified, action, message).await?;

        // ---------------------------------------------------------------------
        // Step 3: upsert environment variables
        info!(
            kind = &kind,
            namespace = &namespace,
            name = &name,
            addon = &addon.real_id,
            "Upsert environment variables for custom resource for addon",
        );

        // We could not use the "addon_xxxx" identifier, we have to use the "config_xxxx" identifier
        let variables = environment::get(&apis, &addon.real_id).await?.iter().fold(
            BTreeMap::new(),
            |mut acc, var| {
                acc.insert(var.name.to_owned(), var.value.to_owned());
                acc
            },
        );

        if modified.spec.variables != variables {
            debug!(
                kind = &kind,
                namespace = &namespace,
                name = &name,
                addon = &addon.real_id,
                "Update config-provider's environment variables with custom resource ones for addon"
            );

            let variables = modified
                .spec
                .variables
                .iter()
                .fold(vec![], |mut acc, (k, v)| {
                    acc.push(Variable::from((k.to_owned(), v.to_owned())));
                    acc
                });

            environment::put(&apis, &addon.real_id, &variables).await?;
        }

        // ---------------------------------------------------------------------
        // Step 4: create the secret
        let s = secret::new(&modified, modified.spec.variables.to_owned());
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

        Ok(())
    }

    async fn delete(ctx: Arc<Context>, origin: Arc<ConfigProvider>) -> Result<(), ReconcilerError> {
        let Context {
            apis,
            kube,
            config: _,
        } = ctx.as_ref();

        let mut modified = (*origin).to_owned();
        let kind = ConfigProvider::kind(&()).to_string();
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
        let message = "Delete configuration provider on clever-cloud";
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
