//! # Event recorder module
//!
//! This module provide an alternative to the golang EventRecorder structure
//!
//! See following links for more details:
//! - <https://book-v1.book.kubebuilder.io/beyond_basics/creating_events.html>
//! - <https://github.com/kubernetes/client-go/blob/master/tools/record/event.go#L56>
//! - <https://docs.openshift.com/online/pro/rest_api/core/event-core-v1.html>

use std::{
    convert::TryFrom,
    fmt::{self, Debug, Display, Formatter},
    str::FromStr,
};

use k8s_openapi::api::core::v1::Event;
use kube::{Client, CustomResourceExt, ResourceExt};
use tracing::debug;
#[cfg(feature = "trace")]
use tracing::Instrument;

use crate::svc::k8s::resource;

pub mod event;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to parse '{0}', available options are 'normal' or 'warning'")]
    Parse(String),
}

// -----------------------------------------------------------------------------
// Level enumeration

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum Level {
    Warning,
    Normal,
}

impl FromStr for Level {
    type Err = Error;

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "warning" => Self::Warning,
            "normal" => Self::Normal,
            _ => {
                return Err(Error::Parse(s.to_string()));
            }
        })
    }
}

impl TryFrom<String> for Level {
    type Error = Error;

    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_str(&s)
    }
}

impl Display for Level {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Warning => write!(f, "Warning"),
            Self::Normal => write!(f, "Normal"),
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<String> for Level {
    #[cfg_attr(feature = "trace", tracing::instrument)]
    fn into(self) -> String {
        self.to_string()
    }
}

// -----------------------------------------------------------------------------
// Helper methods

#[cfg(not(feature = "trace"))]
/// record an event for the given object
pub async fn record<T, U>(
    client: Client,
    obj: &T,
    kind: &Level,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    irecord(client, obj, kind, action, message).await
}

#[cfg(feature = "trace")]
/// record an event for the given object
pub async fn record<T, U>(
    client: Client,
    obj: &T,
    kind: &Level,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    irecord(client, obj, kind, action, message)
        .instrument(tracing::info_span!("recorder::record"))
        .await
}

/// record an event for the given object
async fn irecord<T, U>(
    client: Client,
    obj: &T,
    kind: &Level,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    debug!(
        "Create '{}' event for resource '{}/{}', {}",
        action.to_string(),
        &obj.namespace().unwrap_or_else(|| "<none>".to_string()),
        &obj.name(),
        message
    );
    resource::upsert(client, &event::new(obj, kind, action, message), false).await
}

#[cfg(not(feature = "trace"))]
/// shortcut for the [`record`] method with the 'Normal' [`Level`]
pub async fn normal<T, U>(
    client: Client,
    obj: &T,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    inormal(client, obj, action, message).await
}

#[cfg(feature = "trace")]
/// shortcut for the [`record`] method with the 'Normal' [`Level`]
pub async fn normal<T, U>(
    client: Client,
    obj: &T,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    inormal(client, obj, action, message)
        .instrument(tracing::info_span!("record::normal"))
        .await
}

/// shortcut for the [`record`] method with the 'Normal' [`Level`]
async fn inormal<T, U>(
    client: Client,
    obj: &T,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    record(client, obj, &Level::Normal, action, message).await
}

#[cfg(not(feature = "trace"))]
/// shortcut for the [`record`] method witj the 'Warning' [`Level`]
pub async fn warning<T, U>(
    client: Client,
    obj: &T,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    iwarning(client, obj, action, message).await
}

#[cfg(feature = "trace")]
/// shortcut for the [`record`] method witj the 'Warning' [`Level`]
pub async fn warning<T, U>(
    client: Client,
    obj: &T,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    iwarning(client, obj, action, message)
        .instrument(tracing::info_span!("recorder::warning"))
        .await
}

/// shortcut for the [`record`] method witj the 'Warning' [`Level`]
async fn iwarning<T, U>(
    client: Client,
    obj: &T,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    record(client, obj, &Level::Warning, action, message).await
}
