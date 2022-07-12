//! # Secret module
//!
//! This module provide helpers to generate secrets from a custom resource

use std::{collections::BTreeMap, fmt::Debug};

use k8s_openapi::api::core::v1::Secret;
use kube::{api::ObjectMeta, CustomResourceExt, ResourceExt};

use crate::svc::k8s::resource;

#[cfg_attr(feature = "trace", tracing::instrument)]
pub fn name<T>(obj: &T) -> String
where
    T: ResourceExt + Debug,
{
    format!("{}-secrets", obj.name_any())
}

#[cfg_attr(feature = "trace", tracing::instrument)]
pub fn new<T>(obj: &T, secrets: BTreeMap<String, String>) -> Secret
where
    T: ResourceExt + CustomResourceExt + Debug,
{
    let owner = resource::owner_reference(obj);
    let metadata = ObjectMeta {
        name: Some(name(obj)),
        namespace: obj.namespace(),
        owner_references: Some(vec![owner]),
        ..Default::default()
    };

    Secret {
        metadata,
        string_data: Some(secrets),
        ..Default::default()
    }
}
