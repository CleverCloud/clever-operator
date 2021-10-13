//! # Extensions module
//!
//! This module provide extensions to help building custom resource reconciler
//! loop

use std::collections::BTreeMap;

use async_trait::async_trait;
use clevercloud_sdk::{
    oauth10a::ClientError,
    v2::addon::{self, Addon, CreateAddonOpts},
    Client,
};
use hyper::StatusCode;
use slog_scope::{debug, trace};

// -----------------------------------------------------------------------------
// AddonExt trait

#[async_trait]
pub trait AddonExt: Into<CreateAddonOpts> + Clone + Sync + Send {
    type Error: From<ClientError> + Sync + Send;

    fn id(&self) -> Option<String>;

    fn organisation(&self) -> String;

    fn name(&self) -> String;

    async fn get(&self, client: &Client) -> Result<Option<Addon>, Self::Error> {
        if let Some(id) = &self.id() {
            trace!("Retrieve the addon from the identifier"; "id" => &id, "name" => self.name());
            match addon::get(client, &self.organisation(), id).await {
                Ok(addon) => {
                    return Ok(Some(addon));
                }
                Err(ClientError::StatusCode(code, _))
                    if StatusCode::NOT_FOUND.as_u16() == code.as_u16() =>
                {
                    // try to retrieve the addon from the name
                    trace!("Trying to retrieve the addon by name for the addon"; "id" => &id, "name" => self.name());
                    return Ok(addon::list(client, &self.organisation())
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

        trace!("No such identifier to retrieve crate"; "name" => self.name());
        Ok(None)
    }

    async fn upsert(&self, client: &Client) -> Result<Addon, Self::Error> {
        debug!("Try to retrieve the addon, before creating a new one"; "id" => &self.id(), "name" => self.name());
        if let Some(addon) = self.get(client).await? {
            return Ok(addon);
        }

        debug!("Creating a new addon"; "id" => &self.id(), "name" => self.name());
        Ok(addon::create(client, &self.organisation(), &self.to_owned().into()).await?)
    }

    async fn delete(&self, client: &Client) -> Result<(), Self::Error> {
        if let Some(a) = self.get(client).await? {
            addon::delete(client, &self.organisation(), &a.id).await?;
        }

        Ok(())
    }

    async fn secrets(
        &self,
        client: &Client,
    ) -> Result<Option<BTreeMap<String, String>>, Self::Error> {
        if let Some(id) = &self.id() {
            return Ok(Some(
                addon::environment(client, &self.organisation(), id).await?,
            ));
        }

        Ok(None)
    }
}