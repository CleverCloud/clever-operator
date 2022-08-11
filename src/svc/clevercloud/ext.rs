//! # Extensions module
//!
//! This module provide extensions to help building custom resource reconciler
//! loop

use std::{collections::BTreeMap, fmt::Debug};

use async_trait::async_trait;
use clevercloud_sdk::{
    oauth10a::ClientError,
    v2::addon::{self, Addon, CreateOpts, Error},
};
use hyper::StatusCode;
use tracing::{debug, trace};

use crate::svc::clevercloud::client::Client;

// -----------------------------------------------------------------------------
// AddonExt trait

#[async_trait]
pub trait AddonExt: Into<CreateOpts> + Clone + Debug + Sync + Send {
    type Error: From<Error> + Sync + Send;

    fn id(&self) -> Option<String>;

    fn organisation(&self) -> String;

    fn name(&self) -> String;

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn prefix() -> String {
        "kubernetes".to_string()
    }

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn delimiter() -> String {
        "::".to_string()
    }

    #[cfg_attr(feature = "trace", tracing::instrument)]
    async fn get(&self, client: &Client) -> Result<Option<Addon>, Self::Error> {
        if let Some(id) = &self.id() {
            trace!(
                "Retrieve the addon from the identifier '{}' ({})",
                &id,
                self.name()
            );
            match addon::get(client, &self.organisation(), id).await {
                Ok(addon) => {
                    return Ok(Some(addon));
                }
                Err(Error::Get(_, _, ClientError::StatusCode(code, _)))
                    if StatusCode::NOT_FOUND.as_u16() == code.as_u16() =>
                {
                    // try to retrieve the addon from the name
                    trace!(
                        "Trying to retrieve the addon by name '{}' for the addon '{}'",
                        self.name(),
                        &id
                    );
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

        trace!("No such identifier to retrieve addon '{}'", self.name());
        Ok(None)
    }

    #[cfg_attr(feature = "trace", tracing::instrument)]
    async fn upsert(&self, client: &Client) -> Result<Addon, Self::Error> {
        debug!(
            "Try to retrieve the addon '{}' ({}), before creating a new one",
            self.id().unwrap_or_else(|| "<none>".to_string()),
            self.name()
        );
        if let Some(addon) = self.get(client).await? {
            return Ok(addon);
        }

        debug!("Creating a new addon '{}'", self.name());
        Ok(addon::create(client, &self.organisation(), &self.to_owned().into()).await?)
    }

    #[cfg_attr(feature = "trace", tracing::instrument)]
    async fn delete(&self, client: &Client) -> Result<(), Self::Error> {
        if let Some(a) = self.get(client).await? {
            addon::delete(client, &self.organisation(), &a.id).await?;
        }

        Ok(())
    }

    #[cfg_attr(feature = "trace", tracing::instrument)]
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
