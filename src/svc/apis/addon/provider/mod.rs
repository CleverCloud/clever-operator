//! # Addon provider module
//!
//! This module provide structures and helpers to interact with clever-cloud's
//! addon-provider

use std::{
    convert::TryFrom,
    error::Error,
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

pub mod postgresql;

// -----------------------------------------------------------------------------
// Feature structure

#[derive(Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug)]
pub struct Feature {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "enabled")]
    pub enabled: bool,
}

// -----------------------------------------------------------------------------
// AddonProviderName structure

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
#[serde(untagged, try_from = "String", into = "String")]
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

impl TryFrom<String> for AddonProviderId {
    type Error = Box<dyn Error + Send + Sync>;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_str(&s)
    }
}

#[allow(clippy::from_over_into)]
impl Into<String> for AddonProviderId {
    fn into(self) -> String {
        self.to_string()
    }
}

impl Display for AddonProviderId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::PostgreSql => write!(f, "postgresql-addon"),
        }
    }
}
