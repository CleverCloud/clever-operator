//! # Secret module
//!
//! This module provide helpers to generate secrets from a custom resource

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::Secret;
use kube::{api::ObjectMeta, CustomResourceExt, ResourceExt};

use crate::svc::k8s::resource;

pub fn name<T>(obj: &T) -> String
where
    T: ResourceExt,
{
    format!("{}-secrets", obj.name())
}

pub fn new<T>(obj: &T, secrets: BTreeMap<String, String>) -> Secret
where
    T: ResourceExt + CustomResourceExt,
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
