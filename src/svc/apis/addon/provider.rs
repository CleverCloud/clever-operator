//! # Addon provider module
//!
//! This module provide structures and helpers to interact with clever-cloud's
//! addon-provider

use std::{
    collections::HashMap,
    error::Error,
    fmt::{self, Display, Formatter},
    str::FromStr,
    sync::Arc,
};

use schemars::JsonSchema_repr as JsonSchemaRepr;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr as DeserializeRepr, Serialize_repr as SerializeRepr};
use slog_scope::debug;

use crate::svc::{
    apis::{Client, ClientError, RestClient},
    cfg::Configuration,
};

// -----------------------------------------------------------------------------
// Feature structure

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Feature {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "enabled")]
    pub enabled: bool,
}

// -----------------------------------------------------------------------------
// Version enum

#[derive(
    JsonSchemaRepr,
    SerializeRepr,
    DeserializeRepr,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Clone,
    Debug,
)]
#[serde(untagged)]
#[repr(u8)]
pub enum PostgreSqlVersion {
    V13 = 13,
    V12 = 12,
    V11 = 11,
    V10 = 10,
    V9dot6 = 96,
}

impl FromStr for PostgreSqlVersion {
    type Err = Box<dyn Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "13" => Self::V13,
            "12" => Self::V12,
            "11" => Self::V11,
            "10" => Self::V10,
            "9.6" => Self::V9dot6,
            _ => {
                return Err(format!("failed to parse version from {}, available versions are 13, 12, 11, 10 and 9.6", s).into());
            }
        })
    }
}

impl Display for PostgreSqlVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::V13 => write!(f, "13"),
            Self::V12 => write!(f, "12"),
            Self::V11 => write!(f, "11"),
            Self::V10 => write!(f, "10"),
            Self::V9dot6 => write!(f, "9.6"),
        }
    }
}

// -----------------------------------------------------------------------------
// Cluster structure

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct PostgreSqlCluster {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "label")]
    pub label: String,
    #[serde(rename = "zone")]
    pub zone: String,
    #[serde(rename = "version")]
    pub version: PostgreSqlVersion,
    #[serde(rename = "features")]
    pub features: Vec<Feature>,
}

// -----------------------------------------------------------------------------
// AddonProvider structure

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct PostgreSqlAddonProvider {
    #[serde(rename = "providerId")]
    pub provider_id: AddonProviderId,
    #[serde(rename = "clusters")]
    pub clusters: Vec<PostgreSqlCluster>,
    #[serde(rename = "dedicated")]
    pub dedicated: HashMap<PostgreSqlVersion, Vec<Feature>>,
    #[serde(rename = "defaultDedicatedVersion")]
    pub default: PostgreSqlVersion,
}

// -----------------------------------------------------------------------------
// AddonProviderName structure

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
#[serde(untagged)]
pub enum AddonProviderId {
    PostgreSql,
}

impl FromStr for AddonProviderId {
    type Err = Box<dyn Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "postgresql-addon" => Self::PostgreSql,
            _ => {
                return Err(format!("failed to parse addon provider identifier {}, available option is 'postgresql-addon'", s).into())
            }
        })
    }
}

impl Display for AddonProviderId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::PostgreSql => write!(f, "postgresql-addon"),
        }
    }
}

// -----------------------------------------------------------------------------
// Helpers functions

/// returns information about the postgresql addon provider
pub async fn get_postgresql_addon_provider(
    config: Arc<Configuration>,
    client: &Client,
) -> Result<PostgreSqlAddonProvider, ClientError> {
    let path = format!(
        "{}/v4/addon-providers/{}",
        config.api.endpoint,
        AddonProviderId::PostgreSql
    );

    debug!("execute a request to get information about the postgresql addon-provider"; "path" => &path, "name" => AddonProviderId::PostgreSql.to_string());
    Ok(client.get(&path).await?)
}
