//! # Event recorder module
//!
//! This module provide an alternative to the golang EventRecorder structure
//!
//! See following links for more details:
//! - https://book-v1.book.kubebuilder.io/beyond_basics/creating_events.html
//! - https://github.com/kubernetes/client-go/blob/master/tools/record/event.go#L56
//! - https://docs.openshift.com/online/pro/rest_api/core/event-core-v1.html

use std::{
    convert::TryFrom,
    error::Error,
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use k8s_openapi::api::core::v1::Event;
use kube::{Client, CustomResourceExt, ResourceExt};
use slog_scope::debug;

use crate::svc::k8s::resource;

pub mod event;

// -----------------------------------------------------------------------------
// Level enum

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum Level {
    Warning,
    Normal,
}

impl FromStr for Level {
    type Err = Box<dyn Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "warning" => Self::Warning,
            "normal" => Self::Normal,
            _ => {
                return Err(format!(
                    "failed to parse '{}', available options are 'normal' or 'warning",
                    s
                )
                .into());
            }
        })
    }
}

impl TryFrom<String> for Level {
    type Error = Box<dyn Error + Send + Sync>;

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
    fn into(self) -> String {
        self.to_string()
    }
}

// -----------------------------------------------------------------------------
// Helper methods

/// record an event for the given object
pub async fn record<T, U>(
    client: Client,
    obj: &T,
    kind: &Level,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt,
    U: ToString,
{
    debug!("Create '{}' event for resource", action.to_string(); "uid" => &obj.meta().uid,"name" => &obj.name(), "namespace" => &obj.namespace(), "message" => message);
    resource::upsert(client, &event::new(obj, kind, action, message), false).await
}

/// shortcut for the [`record`] method with the 'Normal' [`Level`]
pub async fn normal<T, U>(
    client: Client,
    obj: &T,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt,
    U: ToString,
{
    record(client, obj, &Level::Normal, action, message).await
}

/// shortcut for the [`record`] method witj the 'Warning' [`Level`]
pub async fn warning<T, U>(
    client: Client,
    obj: &T,
    action: &U,
    message: &str,
) -> Result<Event, kube::Error>
where
    T: ResourceExt + CustomResourceExt,
    U: ToString,
{
    record(client, obj, &Level::Warning, action, message).await
}
