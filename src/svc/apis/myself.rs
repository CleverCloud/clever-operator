//! # Self module
//!
//! This module provide strutures and helpers to interact with the
//! clever-cloud's self api.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use slog_scope::debug;

use crate::svc::{
    apis::{Client, ClientError, RestClient},
    cfg::Configuration,
};

// -----------------------------------------------------------------------------
// Myself structure and helpers

#[derive(Serialize, PartialEq, Eq, Deserialize, Clone, Debug)]
pub struct Myself {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "email")]
    pub email: String,
    #[serde(rename = "phone")]
    pub phone: String,
    #[serde(rename = "address")]
    pub address: String,
    #[serde(rename = "city")]
    pub city: String,
    #[serde(rename = "zipcode")]
    pub zipcode: String,
    #[serde(rename = "country")]
    pub country: String,
    #[serde(rename = "avatar")]
    pub avatar: String,
    #[serde(rename = "creationDate")]
    pub creation_date: u64,
    #[serde(rename = "lang")]
    pub lang: String,
    #[serde(rename = "emailValidated")]
    pub email_validated: bool,
    #[serde(rename = "oauthApps")]
    pub oauth_apps: Vec<String>,
    #[serde(rename = "admin")]
    pub admin: bool,
    #[serde(rename = "canPay")]
    pub can_pay: bool,
    #[serde(rename = "preferredMFA")]
    pub preferred_mfa: String,
    #[serde(rename = "hasPassword")]
    pub has_password: bool,
}

// -----------------------------------------------------------------------------
// Helpers functions

/// returns information about the person logged in
#[allow(dead_code)]
pub async fn get(config: Arc<Configuration>, client: Client) -> Result<Myself, ClientError> {
    let path = format!("{}/v2/self", config.api.endpoint);

    debug!("execute a request to get information about the logged in user"; "path" => &path);
    Ok(client.get(&path).await?)
}
