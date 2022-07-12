//! # Event module
//!
//! This module provide helpers to interact with the kubernetes core/v1/event
//! api

use std::fmt::Debug;

use chrono::Utc;
use k8s_openapi::{
    api::core::v1::{Event, EventSource},
    apimachinery::pkg::apis::meta::v1::{MicroTime, Time},
};
use kube::{api::ObjectMeta, CustomResourceExt, ResourceExt};

use crate::svc::k8s::{recorder::Level, resource};

// -----------------------------------------------------------------------------
// constants

pub const EVENT_FOR: &str = "for";

// -----------------------------------------------------------------------------
// Helper functions

#[cfg_attr(feature = "trace", tracing::instrument)]
/// create a new event from the given parameters
pub fn new<T, U>(obj: &T, kind: &Level, action: &U, message: &str) -> Event
where
    T: ResourceExt + CustomResourceExt + Debug,
    U: ToString + Debug,
{
    let now = Utc::now();

    Event {
        metadata: ObjectMeta {
            namespace: obj.namespace(),
            name: Some(format!(
                "{}-{}-{}",
                obj.name_any(),
                action.to_string().to_lowercase(),
                now.timestamp()
            )),
            ..Default::default()
        },
        type_: Some(kind.to_string()),
        action: Some(action.to_string()),
        count: Some(1),
        event_time: Some(MicroTime(now)),
        first_timestamp: Some(Time(now)),
        involved_object: resource::object_reference(obj),
        last_timestamp: Some(Time(now)),
        message: Some(message.to_string()),
        reason: Some(action.to_string()),
        reporting_component: Some("clever-operator".to_string()),
        reporting_instance: Some(format!(
            "{}/{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        )),
        series: None,
        source: Some(source()),
        ..Default::default()
    }
}

#[cfg_attr(feature = "trace", tracing::instrument)]
/// returns the source of this operator
pub fn source() -> EventSource {
    let host = hostname::get()
        .ok()
        .map(|host| host.to_string_lossy().to_string());

    EventSource {
        component: Some("clever-operator".to_string()),
        host,
    }
}
