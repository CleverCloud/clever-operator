//! # Postgresql addon provider plan module
//!
//! This module provide helpers and structures to interact with the plan api of
//! the postgresql addon provider

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use slog_scope::debug;

use crate::svc::{
    apis::{
        addon::{provider::AddonProviderId, Feature},
        Client, ClientError, RestClient,
    },
    cfg::Configuration,
};

// -----------------------------------------------------------------------------
// Plan structure

#[derive(Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug)]
pub struct Plan {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "slug")]
    pub slug: String,
    #[serde(rename = "price")]
    pub price: f64,
    #[serde(rename = "price_id")]
    pub price_id: String,
    #[serde(rename = "features")]
    pub features: Vec<Feature>,
    #[serde(rename = "zones")]
    pub zones: Vec<String>,
}

// -----------------------------------------------------------------------------
// AddonProviderPlan structure

#[derive(Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug)]
pub struct AddonProviderPlan {
    #[serde(rename = "id")]
    pub id: AddonProviderId,
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
    #[serde(rename = "plans")]
    pub plans: Vec<Plan>,
}

// -----------------------------------------------------------------------------
// Helpers method

/// returns the list of plan for the postgresql addon provider
pub async fn list(
    config: Arc<Configuration>,
    client: &Client,
    organisation_id: &str,
) -> Result<AddonProviderPlan, ClientError> {
    let path = format!(
        "{}/v2/products/addonproviders/{}?orga_id={}",
        config.api.endpoint,
        AddonProviderId::PostgreSql,
        organisation_id
    );

    debug!("execute a request to list plans of the postgresql addon-provider"; "path" => &path, "name" => AddonProviderId::PostgreSql.to_string());
    Ok(client.get(&path).await?)
}

/// list plans for the organisation and try to find one matching the pattern
/// returns the plan if found
pub async fn find(
    config: Arc<Configuration>,
    client: &Client,
    organisation_id: &str,
    pattern: &str,
) -> Result<Option<Plan>, ClientError> {
    Ok(list(config, client, organisation_id)
        .await?
        .plans
        .iter()
        .find(|plan| plan.slug == pattern || plan.id == pattern || plan.name == pattern)
        .map(ToOwned::to_owned))
}
