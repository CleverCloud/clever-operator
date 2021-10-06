//! # Resource module
//!
//! This module provide helpers on kubernetes [`Resource`]

use std::{fmt::Debug, time::Instant};

use k8s_openapi::{
    api::core::v1::ObjectReference, apimachinery::pkg::apis::meta::v1::OwnerReference,
};
use kube::{
    api::{ListParams, Patch, PatchParams, PostParams},
    Api, Client, CustomResourceExt, Resource, ResourceExt,
};
use lazy_static::lazy_static;

use prometheus::{opts, register_counter_vec, CounterVec};
use serde::{de::DeserializeOwned, Serialize};
use slog_scope::{debug, trace};

// -----------------------------------------------------------------------------
// Telemetry

lazy_static! {
    static ref CLIENT_REQUEST_SUCCESS: CounterVec = register_counter_vec!(
        opts!(
            "kubernetes_client_request_success",
            "number of successful kubernetes request",
        ),
        &["action", "namespace"]
    )
    .expect("metrics 'kubernetes_client_request_success' to not be already registered");
    static ref CLIENT_REQUEST_FAILURE: CounterVec = register_counter_vec!(
        opts!(
            "kubernetes_client_request_failure",
            "number of failed kubernetes request",
        ),
        &["action", "namespace"]
    )
    .expect("metrics 'kubernetes_client_request_failure' to not be already registered");
    static ref CLIENT_REQUEST_DURATION: CounterVec = register_counter_vec!(
        opts!(
            "kubernetes_client_request_duration",
            "duration of kubernetes request",
        ),
        &["action", "namespace", "unit"]
    )
    .expect("metrics 'kubernetes_client_request_duration' to not be already registered");
}

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
    let instant = Instant::now();
    let result = Api::namespaced(client, &namespace)
        .patch(&name, &PatchParams::default(), &Patch::Json::<T>(patch))
        .await;

    if result.is_ok() {
        CLIENT_REQUEST_SUCCESS
            .with_label_values(&["PATCH", &namespace])
            .inc();
    } else {
        CLIENT_REQUEST_FAILURE
            .with_label_values(&["PATCH", &namespace])
            .inc();
    }

    CLIENT_REQUEST_DURATION
        .with_label_values(&["PATCH", &namespace, "us"])
        .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

    result
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
    let instant = Instant::now();
    let result = Api::namespaced(client, &namespace)
        .patch_status(&name, &PatchParams::default(), &Patch::Json::<T>(patch))
        .await;

    if result.is_ok() {
        CLIENT_REQUEST_SUCCESS
            .with_label_values(&["PATCH", &namespace])
            .inc();
    } else {
        CLIENT_REQUEST_FAILURE
            .with_label_values(&["PATCH", &namespace])
            .inc();
    }

    CLIENT_REQUEST_DURATION
        .with_label_values(&["PATCH", &namespace, "us"])
        .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

    result
}

/// returns the list of resources matching the query
pub async fn find_by_labels<T>(client: Client, ns: &str, query: &str) -> Result<Vec<T>, kube::Error>
where
    T: Resource + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    trace!("execute a request to find by labels resources"; "namespace" => ns, "query" => query);
    let instant = Instant::now();
    let result = Api::namespaced(client, ns)
        .list(&ListParams::default().labels(query))
        .await;

    if result.is_ok() {
        CLIENT_REQUEST_SUCCESS
            .with_label_values(&["LIST", ns])
            .inc();
    } else {
        CLIENT_REQUEST_FAILURE
            .with_label_values(&["LIST", ns])
            .inc();
    }

    CLIENT_REQUEST_DURATION
        .with_label_values(&["LIST", ns, "us"])
        .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

    Ok(result?.items)
}

/// returns the object using namespace and name by asking kubernetes
pub async fn get<T>(client: Client, ns: &str, name: &str) -> Result<Option<T>, kube::Error>
where
    T: Resource + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    let api: Api<T> = Api::namespaced(client, ns);

    trace!("execute a request to retrieve resource"; "namespace" => ns, "name" => name);
    let instant = Instant::now();
    match api.get(name).await {
        Ok(r) => {
            CLIENT_REQUEST_SUCCESS.with_label_values(&["GET", ns]).inc();
            CLIENT_REQUEST_DURATION
                .with_label_values(&["GET", ns, "us"])
                .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

            Ok(Some(r))
        }
        Err(kube::Error::Api(err)) if err.code == 404 => {
            CLIENT_REQUEST_SUCCESS.with_label_values(&["GET", ns]).inc();
            CLIENT_REQUEST_DURATION
                .with_label_values(&["GET", ns, "us"])
                .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

            Ok(None)
        }
        Err(err) => {
            CLIENT_REQUEST_FAILURE.with_label_values(&["GET", ns]).inc();
            CLIENT_REQUEST_DURATION
                .with_label_values(&["GET", ns, "us"])
                .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

            Err(err)
        }
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
    let instant = Instant::now();
    let result = Api::namespaced(client, &namespace)
        .create(&PostParams::default(), obj)
        .await;

    if result.is_ok() {
        CLIENT_REQUEST_SUCCESS
            .with_label_values(&["POST", &namespace])
            .inc();
    } else {
        CLIENT_REQUEST_FAILURE
            .with_label_values(&["POST", &namespace])
            .inc();
    }

    CLIENT_REQUEST_DURATION
        .with_label_values(&["POST", &namespace, "us"])
        .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

    result
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
