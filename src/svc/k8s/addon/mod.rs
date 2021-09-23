//! # Addon module
//!
//! This module provide structure, custom resource and their definition for addon
pub mod postgresql;

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use hyper::StatusCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::svc::{
    apis::{
        addon::{self, Addon, CreateAddonOpts},
        Client, ClientError,
    },
    cfg::Configuration,
};

// -----------------------------------------------------------------------------
// Instance structure

#[derive(JsonSchema, Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Instance {
    pub region: String,
    pub plan: String,
}

// -----------------------------------------------------------------------------
// AddonExt trait

#[async_trait]
pub trait AddonExt: Into<CreateAddonOpts> + Clone + Sync + Send {
    type Error: From<ClientError> + Send;

    fn id(&self) -> Option<String>;

    fn organisation(&self) -> String;

    fn name(&self) -> String;

    async fn get(
        &self,
        config: Arc<Configuration>,
        client: &Client,
    ) -> Result<Option<Addon>, Self::Error> {
        if let Some(id) = &self.id() {
            match addon::get(config.to_owned(), client, &self.organisation(), id).await {
                Ok(addon) => {
                    return Ok(Some(addon));
                }
                Err(ClientError::StatusCode(code, _))
                    if StatusCode::NOT_FOUND.as_u16() == code.as_u16() =>
                {
                    // try to retrieve the addon from the name
                    return Ok(addon::list(config, client, &self.organisation())
                        .await
                        .map_err(Into::into)?
                        .iter()
                        .find(|addon| addon.name == Some(self.name()))
                        .map(ToOwned::to_owned));
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        Ok(None)
    }

    async fn upsert(
        &self,
        config: Arc<Configuration>,
        client: &Client,
    ) -> Result<Addon, Self::Error> {
        if let Some(addon) = self.get(config.to_owned(), client).await? {
            return Ok(addon);
        }

        Ok(addon::create(
            config,
            client,
            &self.organisation(),
            &self.to_owned().into(),
        )
        .await?)
    }

    async fn delete(&self, config: Arc<Configuration>, client: &Client) -> Result<(), Self::Error> {
        if let Some(a) = self.get(config.to_owned(), client).await? {
            addon::delete(config, client, &self.organisation(), &a.id).await?;
        }

        Ok(())
    }

    async fn secrets(
        &self,
        config: Arc<Configuration>,
        client: &Client,
    ) -> Result<Option<BTreeMap<String, String>>, Self::Error> {
        if let Some(id) = &self.id() {
            return Ok(Some(
                addon::environment(config, client, &self.organisation(), id).await?,
            ));
        }

        Ok(None)
    }
}
