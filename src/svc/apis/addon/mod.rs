//! # Addon module
//!
//! This module provide addon structures and helpers to interact with
//! clever-cloud's addon apis.

use std::{collections::BTreeMap, sync::Arc};

use kube::core::object::HasSpec;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use slog_scope::debug;

use crate::svc::{
    apis::{addon::provider::AddonProviderId, Client, ClientError, RestClient},
    cfg::Configuration,
    k8s::addon::{
        postgresql::{PostgreSql, PostgreSqlOpts},
        AddonExt,
    },
};

pub mod provider;

// -----------------------------------------------------------------------------
// Provider structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug)]
pub struct Provider {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "website")]
    pub website: String,
    #[serde(rename = "supportEmail")]
    pub support_email: String,
    #[serde(rename = "googlePlusName")]
    pub google_plus_name: String,
    #[serde(rename = "twitterName")]
    pub twitter_name: String,
    #[serde(rename = "analyticsId")]
    pub analytics_id: String,
    #[serde(rename = "shortDesc")]
    pub short_description: String,
    #[serde(rename = "longDesc")]
    pub long_description: String,
    #[serde(rename = "logoUrl")]
    pub logo_url: String,
    #[serde(rename = "status")]
    pub status: String,
    #[serde(rename = "openInNewTab")]
    pub open_in_new_tab: bool,
    #[serde(rename = "canUpgrade")]
    pub can_upgrade: bool,
    #[serde(rename = "regions")]
    pub regions: Vec<String>,
}

// -----------------------------------------------------------------------------
// Feature structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug)]
pub struct Feature {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "value")]
    pub value: String,
    #[serde(rename = "computable_value")]
    pub computable_value: Option<String>,
    #[serde(rename = "name_code")]
    pub name_code: Option<String>,
}

// -----------------------------------------------------------------------------
// Plan structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug)]
pub struct Plan {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "slug")]
    pub slug: String,
    #[serde(rename = "price")]
    pub price: f32,
    #[serde(rename = "price_id")]
    pub price_id: String,
    #[serde(rename = "features")]
    pub features: Vec<Feature>,
    #[serde(rename = "zones")]
    pub zones: Vec<String>,
}

// -----------------------------------------------------------------------------
// Addon structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug)]
pub struct Addon {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: Option<String>,
    #[serde(rename = "realId")]
    pub real_id: String,
    #[serde(rename = "region")]
    pub region: String,
    #[serde(rename = "provider")]
    pub provider: Provider,
    #[serde(rename = "plan")]
    pub plan: Plan,
    #[serde(rename = "creationDate")]
    pub creation_date: u64,
    #[serde(rename = "configKeys")]
    pub config_keys: Vec<String>,
}

// -----------------------------------------------------------------------------
// AddonOpts enum

#[derive(Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord, Clone, Debug)]
#[serde(untagged)]
pub enum AddonOpts {
    PostgreSql {
        #[serde(rename = "version")]
        version: String,
        #[serde(rename = "encryption")]
        encryption: String,
    },
}

impl From<PostgreSqlOpts> for AddonOpts {
    fn from(opts: PostgreSqlOpts) -> Self {
        Self::PostgreSql {
            version: opts.version.to_string(),
            encryption: opts.encryption.to_string(),
        }
    }
}

// -----------------------------------------------------------------------------
// CreateAddonOpts structure

#[derive(Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug)]
pub struct CreateAddonOpts {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "region")]
    pub region: String,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    #[serde(rename = "plan")]
    pub plan: String,
    #[serde(rename = "options")]
    pub options: AddonOpts,
}

impl From<PostgreSql> for CreateAddonOpts {
    fn from(postgresql: PostgreSql) -> Self {
        let spec = postgresql.spec();

        Self {
            name: postgresql.name(),
            region: spec.instance.region.to_owned(),
            provider_id: AddonProviderId::PostgreSql.to_string(),
            plan: spec.instance.plan.to_owned(),
            options: AddonOpts::from(spec.options.to_owned()),
        }
    }
}

// -----------------------------------------------------------------------------
// EnvironmentVariable struct

#[derive(Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug)]
pub struct EnvironmentVariable {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "value")]
    pub value: String,
}

// -----------------------------------------------------------------------------
// Helpers functions

/// returns the list of addons for the given organisation
pub async fn list(
    config: Arc<Configuration>,
    client: &Client,
    organisation_id: &str,
) -> Result<Vec<Addon>, ClientError> {
    let path = format!(
        "{}/v2/organisations/{}/addons",
        config.api.endpoint, organisation_id,
    );

    debug!("execute a request to get the list of addons"; "path" => &path, "organisation" => organisation_id);
    Ok(client.get(&path).await?)
}

/// returns the addon for the given the organisation and identifier
pub async fn get(
    config: Arc<Configuration>,
    client: &Client,
    organisation_id: &str,
    id: &str,
) -> Result<Addon, ClientError> {
    let path = format!(
        "{}/v2/organisations/{}/addons/{}",
        config.api.endpoint, organisation_id, id
    );

    debug!("execute a request to get information about an addon"; "path" => &path, "organisatiion" => organisation_id, "id" => id);
    Ok(client.get(&path).await?)
}

/// create the addon and returns it
pub async fn create(
    config: Arc<Configuration>,
    client: &Client,
    organisation_id: &str,
    opts: &CreateAddonOpts,
) -> Result<Addon, ClientError> {
    let path = format!(
        "{}/v2/organisations/{}/addons",
        config.api.endpoint, organisation_id
    );

    debug!("execute a request to create an addon"; "path" => &path, "organisation" => organisation_id, "name" => &opts.name, "region" => &opts.region, "plan" => &opts.plan, "provider-id" => &opts.provider_id.to_string());
    Ok(client.post(&path, opts).await?)
}

/// delete the given addon
pub async fn delete(
    config: Arc<Configuration>,
    client: &Client,
    organisation_id: &str,
    id: &str,
) -> Result<(), ClientError> {
    let path = format!(
        "{}/v2/organisations/{}/addons/{}",
        config.api.endpoint, organisation_id, id
    );

    debug!("execute a request to delete an addon"; "path" => &path, "organisation" => organisation_id, "id" => id);
    Ok(client.delete(&path).await?)
}

/// returns environment variables for an addon
pub async fn environment(
    config: Arc<Configuration>,
    client: &Client,
    organisation_id: &str,
    id: &str,
) -> Result<BTreeMap<String, String>, ClientError> {
    let path = format!(
        "{}/v2/organisations/{}/addons/{}/env",
        config.api.endpoint, organisation_id, id
    );

    debug!("execute a request to get secret of a addon"; "path" => &path, "organisation" => organisation_id, "id" => id);
    let env: Vec<EnvironmentVariable> = client.get(&path).await?;

    Ok(env.iter().fold(BTreeMap::new(), |mut acc, var| {
        acc.insert(var.name.to_owned(), var.value.to_owned());
        acc
    }))
}
