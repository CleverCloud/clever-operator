//! # Resource module
//!
//! This module provide helpers on kubernetes [`Resource`]

use std::fmt::Debug;

use k8s_openapi::{
    api::core::v1::ObjectReference, apimachinery::pkg::apis::meta::v1::OwnerReference,
};
use kube::{
    api::{ListParams, Patch, PatchParams, PostParams},
    Api, Client, CustomResourceExt, Resource, ResourceExt,
};

use serde::{de::DeserializeOwned, Serialize};
use slog_scope::{debug, trace};

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
/// panic if the namespace or name is null which is impossible, if the resource
/// is created
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

    trace!("execute patch request on resource"; "name" => &name, "namespace" => &namespace, "patch" => serde_json::to_string(&patch).unwrap());
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

    trace!("execute patch request on resource's status"; "name" => &name, "namespace" => &namespace, "patch" => serde_json::to_string(&patch).unwrap());
    Api::namespaced(client, &namespace)
        .patch_status(&name, &PatchParams::default(), &Patch::Json::<T>(patch))
        .await
}

/// returns the list of resources matching the query
pub async fn find_by_labels<T>(client: Client, ns: &str, query: &str) -> Result<Vec<T>, kube::Error>
where
    T: Resource + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    trace!("execute a request to find by labels resources"; "namespace" => ns, "query" => query);
    Ok(Api::namespaced(client, ns)
        .list(&ListParams::default().labels(query))
        .await?
        .items)
}

/// returns the object using namespace and name by asking kubernetes
pub async fn get<T>(client: Client, ns: &str, name: &str) -> Result<Option<T>, kube::Error>
where
    T: Resource + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    let api: Api<T> = Api::namespaced(client, ns);

    trace!("execute a request to retrieve resource"; "namespace" => ns, "name" => name);
    match api.get(name).await {
        Ok(r) => Ok(Some(r)),
        Err(kube::Error::Api(err)) if err.code == 404 => Ok(None),
        Err(err) => Err(err),
    }
}

/// create the given kubernetes object and return it completed by kubernetes,
/// this function should be avoid in favor of the [`upsert`] one
pub async fn create<T>(client: Client, obj: &T) -> Result<T, kube::Error>
where
    T: ResourceExt + Serialize + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    let (namespace, name) = namespaced_name(obj);

    trace!("execute a request to create a resource"; "namespace" => &namespace, "name" => &name);
    Api::namespaced(client, &namespace)
        .create(&PostParams::default(), obj)
        .await
}

/// upsert the given kubernetes object, get it and create it, if it does not
/// exist or else patch it
pub async fn upsert<T>(client: Client, obj: &T, status: bool) -> Result<T, kube::Error>
where
    T: ResourceExt + Serialize + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    let (ns, name) = namespaced_name(obj);
    if let Some(o) = get(client.to_owned(), &ns, &name).await? {
        let p = diff(&o, obj)?;
        let mut obj = patch(client.to_owned(), obj, p.to_owned()).await?;

        // todo: change this boolean to a polymorphic implementation instead
        if status {
            obj = patch_status(client, obj, p).await?;
        }

        return Ok(obj);
    }

    create(client, obj).await
}

/// returns a owner reference object pointing to the given resource
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

/// returns a object reference pointing to the given resource
pub fn object_reference<T>(obj: &T) -> ObjectReference
where
    T: ResourceExt + CustomResourceExt,
{
    let api_resource = T::api_resource();

    ObjectReference {
        api_version: Some(api_resource.api_version),
        kind: Some(api_resource.kind),
        name: Some(obj.name()),
        uid: obj.uid(),
        namespace: obj.namespace(),
        resource_version: obj.resource_version(),
        ..Default::default()
    }
}
