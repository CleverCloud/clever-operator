//! # Resource module
//!
//! This module provide helpers on kubernetes [`Resource`]

use std::fmt::Debug;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::{
    api::{Patch, PatchParams},
    Api, Client, CustomResourceExt, Resource, ResourceExt,
};

use serde::{de::DeserializeOwned, Serialize};
use slog_scope::debug;

// -----------------------------------------------------------------------------
// Helpers functions

/// returns if the resource is considered from kubernetes point of view as deleted
pub fn deleted<T>(obj: &T) -> bool
where
    T: Resource,
{
    obj.meta().deletion_timestamp.is_some()
}

/// returns the namespace and name of the kubernetes resource.
///
/// # Panic
///
/// panic if the namespace or name is null which is impossible btw
pub fn namespaced_name<T>(obj: &T) -> (String, String)
where
    T: ResourceExt,
{
    (
        obj.namespace()
            .expect("resource to be owned by a namespace"),
        obj.name(),
    )
}

/// returns differnce between the two given object serialize as json patch
pub fn diff<T>(origin: &T, modified: &T) -> Result<json_patch::Patch, serde_json::Error>
where
    T: Serialize,
{
    Ok(json_patch::diff(
        &serde_json::to_value(origin)?,
        &serde_json::to_value(modified)?,
    ))
}

/// make a patch request on the given resource using the given patch
pub async fn patch<T>(client: Client, obj: &T, patch: json_patch::Patch) -> Result<T, kube::Error>
where
    T: Resource + DeserializeOwned + Serialize + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    let (namespace, name) = namespaced_name(obj);

    if patch.0.is_empty() {
        debug!("skip patch request on resource, no operation to apply"; "name" => &name, "namespace" => &namespace);
        return Ok(obj.to_owned());
    }

    debug!("execute patch request on resource"; "name" => &name, "namespace" => &namespace, "patch" => serde_json::to_string(&patch).unwrap());
    Api::namespaced(client, &namespace)
        .patch(&name, &PatchParams::default(), &Patch::Json::<T>(patch))
        .await
}

/// make a patch request on the given resource's status using the given patch
pub async fn patch_status<T>(
    client: Client,
    obj: T,
    patch: json_patch::Patch,
) -> Result<T, kube::Error>
where
    T: Resource + DeserializeOwned + Serialize + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    let (namespace, name) = namespaced_name(&obj);

    if patch.0.is_empty() {
        debug!("skip patch request on resource's status, no operation to apply"; "name" => &name, "namespace" => &namespace);
        return Ok(obj.to_owned());
    }

    debug!("execute patch request on resource's status"; "name" => &name, "namespace" => &namespace, "patch" => serde_json::to_string(&patch).unwrap());
    Api::namespaced(client, &namespace)
        .patch_status(&name, &PatchParams::default(), &Patch::Json::<T>(patch))
        .await
}

/// returns a owner references object pointing to the given resource
pub fn owner_reference<T>(obj: &T) -> OwnerReference
where
    T: ResourceExt + CustomResourceExt,
{
    let api_resource = T::api_resource();

    OwnerReference {
        api_version: api_resource.api_version,
        block_owner_deletion: Some(true),
        controller: None,
        kind: api_resource.kind,
        name: obj.name(),
        uid: obj
            .uid()
            .expect("to have an unique identifier provided by kubernetes"),
    }
}
