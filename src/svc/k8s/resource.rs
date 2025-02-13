//! # Resource module
//!
//! This module provide helpers on kubernetes [`Resource`]

use std::fmt::Debug;
#[cfg(feature = "metrics")]
use std::{sync::LazyLock, time::Instant};

use k8s_openapi::{
    api::core::v1::ObjectReference, apimachinery::pkg::apis::meta::v1::OwnerReference,
    NamespaceResourceScope,
};
use kube::{
    api::{ListParams, Patch, PatchParams, PostParams},
    Api, Client, CustomResourceExt, Resource, ResourceExt,
};
#[cfg(feature = "metrics")]
use prometheus::{opts, register_counter_vec, CounterVec};
use serde::{de::DeserializeOwned, Serialize};
#[cfg(feature = "tracing")]
use tracing::Instrument;
use tracing::{debug, level_enabled, trace, Level};

// -----------------------------------------------------------------------------
// Telemetry

#[cfg(feature = "metrics")]
static CLIENT_REQUEST_SUCCESS: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_client_request_success",
            "number of successful kubernetes request",
        ),
        &["action", "namespace"]
    )
    .expect("metrics 'kubernetes_client_request_success' to not be already registered")
});

#[cfg(feature = "metrics")]
static CLIENT_REQUEST_FAILURE: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_client_request_failure",
            "number of failed kubernetes request",
        ),
        &["action", "namespace"]
    )
    .expect("metrics 'kubernetes_client_request_failure' to not be already registered")
});

#[cfg(feature = "metrics")]
static CLIENT_REQUEST_DURATION: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_client_request_duration",
            "duration of kubernetes request",
        ),
        &["action", "namespace", "unit"]
    )
    .expect("metrics 'kubernetes_client_request_duration' to not be already registered")
});

// -----------------------------------------------------------------------------
// Helpers functions

#[cfg_attr(feature = "tracing", tracing::instrument)]
/// returns if the resource is considered from kubernetes point of view as deleted
pub fn deleted<T>(obj: &T) -> bool
where
    T: Resource<Scope = NamespaceResourceScope> + Debug,
{
    obj.meta().deletion_timestamp.is_some()
}

#[cfg_attr(feature = "tracing", tracing::instrument)]
/// returns the namespace and name of the kubernetes resource.
///
/// # Panic
///
/// panic if the namespace or name is null which is impossible, if the resource
/// is created
pub fn namespaced_name<T>(obj: &T) -> (String, String)
where
    T: ResourceExt + Debug,
{
    (
        obj.namespace()
            .expect("resource to be owned by a namespace"),
        obj.name_any(),
    )
}

#[cfg_attr(feature = "tracing", tracing::instrument)]
/// returns differnce between the two given object serialize as json patch
pub fn diff<T>(origin: &T, modified: &T) -> Result<json_patch::Patch, serde_json::Error>
where
    T: Serialize + Debug,
{
    Ok(json_patch::diff(
        &serde_json::to_value(origin)?,
        &serde_json::to_value(modified)?,
    ))
}

#[cfg(not(feature = "tracing"))]
/// make a patch request on the given resource using the given patch
pub async fn patch<T>(client: Client, obj: &T, patch: json_patch::Patch) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Serialize + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    ipatch(client, obj, patch).await
}

#[cfg(feature = "tracing")]
/// make a patch request on the given resource using the given patch
pub async fn patch<T>(client: Client, obj: &T, patch: json_patch::Patch) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Serialize + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    ipatch(client, obj, patch)
        .instrument(tracing::info_span!("resource::patch"))
        .await
}

async fn ipatch<T>(client: Client, obj: &T, patch: json_patch::Patch) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Serialize + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    let (namespace, name) = namespaced_name(obj);

    if patch.0.is_empty() {
        debug!(
            namespace = &namespace,
            name = &name,
            "skip patch request on resource, no operation to apply",
        );

        return Ok(obj.to_owned());
    }

    if level_enabled!(Level::TRACE) {
        trace!(
            namespace = &namespace,
            name = &name,
            payload = serde_json::to_string(&patch)
                .expect("Serialize patch as JSON string without error"),
            "execute patch request on resource",
        );
    }

    #[cfg(feature = "metrics")]
    let instant = Instant::now();
    let result = Api::namespaced(client, &namespace)
        .patch(&name, &PatchParams::default(), &Patch::Json::<T>(patch))
        .await;

    #[cfg(feature = "metrics")]
    if result.is_ok() {
        CLIENT_REQUEST_SUCCESS
            .with_label_values(&["PATCH", &namespace])
            .inc();
    } else {
        CLIENT_REQUEST_FAILURE
            .with_label_values(&["PATCH", &namespace])
            .inc();
    }

    #[cfg(feature = "metrics")]
    CLIENT_REQUEST_DURATION
        .with_label_values(&["PATCH", &namespace, "us"])
        .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

    result
}

#[cfg(not(feature = "tracing"))]
/// make a patch request on the given resource's status using the given patch
pub async fn patch_status<T>(
    client: Client,
    obj: T,
    patch: json_patch::Patch,
) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Serialize + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    ipatch_status(client, obj, patch).await
}

#[cfg(feature = "tracing")]
/// make a patch request on the given resource's status using the given patch
pub async fn patch_status<T>(
    client: Client,
    obj: T,
    patch: json_patch::Patch,
) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Serialize + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    ipatch_status(client, obj, patch)
        .instrument(tracing::info_span!("resource::patch_status"))
        .await
}

/// make a patch request on the given resource's status using the given patch
async fn ipatch_status<T>(
    client: Client,
    obj: T,
    patch: json_patch::Patch,
) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Serialize + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    let (namespace, name) = namespaced_name(&obj);

    if patch.0.is_empty() {
        debug!(
            namespace = &namespace,
            name = &name,
            "skip patch request on resource's status, no operation to apply",
        );

        return Ok(obj.to_owned());
    }

    if level_enabled!(Level::TRACE) {
        trace!(
            namespace = &namespace,
            name = &name,
            payload = serde_json::to_string(&patch)
                .expect("Serialize patch as JSON string without error"),
            "execute patch request status on resource",
        );
    }

    #[cfg(feature = "metrics")]
    let instant = Instant::now();
    let result = Api::namespaced(client, &namespace)
        .patch_status(&name, &PatchParams::default(), &Patch::Json::<T>(patch))
        .await;

    #[cfg(feature = "metrics")]
    if result.is_ok() {
        CLIENT_REQUEST_SUCCESS
            .with_label_values(&["PATCH", &namespace])
            .inc();
    } else {
        CLIENT_REQUEST_FAILURE
            .with_label_values(&["PATCH", &namespace])
            .inc();
    }

    #[cfg(feature = "metrics")]
    CLIENT_REQUEST_DURATION
        .with_label_values(&["PATCH", &namespace, "us"])
        .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

    result
}

#[cfg(not(feature = "tracing"))]
/// returns the list of resources matching the query
pub async fn find_by_labels<T>(client: Client, ns: &str, query: &str) -> Result<Vec<T>, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    ifind_by_labels(client, ns, query).await
}

#[cfg(feature = "tracing")]
/// returns the list of resources matching the query
pub async fn find_by_labels<T>(client: Client, ns: &str, query: &str) -> Result<Vec<T>, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    ifind_by_labels(client, ns, query)
        .instrument(tracing::info_span!("resource::find_by_labels"))
        .await
}

/// returns the list of resources matching the query
async fn ifind_by_labels<T>(client: Client, ns: &str, query: &str) -> Result<Vec<T>, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    trace!(
        namespace = &ns,
        query = query,
        "execute a request to find by labels resources on namespace",
    );

    #[cfg(feature = "metrics")]
    let instant = Instant::now();
    let result = Api::namespaced(client, ns)
        .list(&ListParams::default().labels(query))
        .await;

    #[cfg(feature = "metrics")]
    if result.is_ok() {
        CLIENT_REQUEST_SUCCESS
            .with_label_values(&["LIST", ns])
            .inc();
    } else {
        CLIENT_REQUEST_FAILURE
            .with_label_values(&["LIST", ns])
            .inc();
    }

    #[cfg(feature = "metrics")]
    CLIENT_REQUEST_DURATION
        .with_label_values(&["LIST", ns, "us"])
        .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

    Ok(result?.items)
}

#[cfg(not(feature = "tracing"))]
/// returns the object using namespace and name by asking kubernetes
pub async fn get<T>(client: Client, ns: &str, name: &str) -> Result<Option<T>, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    iget(client, ns, name).await
}

#[cfg(feature = "tracing")]
/// returns the object using namespace and name by asking kubernetes
pub async fn get<T>(client: Client, ns: &str, name: &str) -> Result<Option<T>, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    iget(client, ns, name)
        .instrument(tracing::info_span!("resource::get"))
        .await
}

async fn iget<T>(client: Client, ns: &str, name: &str) -> Result<Option<T>, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope> + DeserializeOwned + Clone + Debug,
    <T as Resource>::DynamicType: Default,
{
    let api: Api<T> = Api::namespaced(client, ns);

    trace!(
        namespace = &ns,
        name = &name,
        "execute a request to retrieve resource"
    );

    #[cfg(feature = "metrics")]
    let instant = Instant::now();
    match api.get(name).await {
        Ok(r) => {
            #[cfg(feature = "metrics")]
            CLIENT_REQUEST_SUCCESS.with_label_values(&["GET", ns]).inc();
            #[cfg(feature = "metrics")]
            CLIENT_REQUEST_DURATION
                .with_label_values(&["GET", ns, "us"])
                .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

            Ok(Some(r))
        }
        Err(kube::Error::Api(err)) if err.code == 404 => {
            #[cfg(feature = "metrics")]
            CLIENT_REQUEST_SUCCESS.with_label_values(&["GET", ns]).inc();
            #[cfg(feature = "metrics")]
            CLIENT_REQUEST_DURATION
                .with_label_values(&["GET", ns, "us"])
                .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

            Ok(None)
        }
        Err(err) => {
            #[cfg(feature = "metrics")]
            CLIENT_REQUEST_FAILURE.with_label_values(&["GET", ns]).inc();
            #[cfg(feature = "metrics")]
            CLIENT_REQUEST_DURATION
                .with_label_values(&["GET", ns, "us"])
                .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

            Err(err)
        }
    }
}

#[cfg(not(feature = "tracing"))]
/// create the given kubernetes object and return it completed by kubernetes,
/// this function should be avoid in favor of the [`upsert`] one
pub async fn create<T>(client: Client, obj: &T) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope>
        + ResourceExt
        + Serialize
        + DeserializeOwned
        + Clone
        + Debug,
    <T as Resource>::DynamicType: Default,
{
    icreate(client, obj).await
}

#[cfg(feature = "tracing")]
/// create the given kubernetes object and return it completed by kubernetes,
/// this function should be avoid in favor of the [`upsert`] one
pub async fn create<T>(client: Client, obj: &T) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope>
        + ResourceExt
        + Serialize
        + DeserializeOwned
        + Clone
        + Debug,
    <T as Resource>::DynamicType: Default,
{
    icreate(client, obj)
        .instrument(tracing::info_span!("resource::create"))
        .await
}

/// create the given kubernetes object and return it completed by kubernetes,
/// this function should be avoid in favor of the [`upsert`] one
async fn icreate<T>(client: Client, obj: &T) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope>
        + ResourceExt
        + Serialize
        + DeserializeOwned
        + Clone
        + Debug,
    <T as Resource>::DynamicType: Default,
{
    let (namespace, name) = namespaced_name(obj);

    trace!(
        namespace = &namespace,
        name = &name,
        "execute a request to create a resource",
    );

    #[cfg(feature = "metrics")]
    let instant = Instant::now();
    let result = Api::namespaced(client, &namespace)
        .create(&PostParams::default(), obj)
        .await;

    #[cfg(feature = "metrics")]
    if result.is_ok() {
        CLIENT_REQUEST_SUCCESS
            .with_label_values(&["POST", &namespace])
            .inc();
    } else {
        CLIENT_REQUEST_FAILURE
            .with_label_values(&["POST", &namespace])
            .inc();
    }

    #[cfg(feature = "metrics")]
    CLIENT_REQUEST_DURATION
        .with_label_values(&["POST", &namespace, "us"])
        .inc_by(Instant::now().duration_since(instant).as_micros() as f64);

    result
}

#[cfg(not(feature = "tracing"))]
/// upsert the given kubernetes object, get it and create it, if it does not
/// exist or else patch it
pub async fn upsert<T>(client: Client, obj: &T, status: bool) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope>
        + ResourceExt
        + Serialize
        + DeserializeOwned
        + Clone
        + Debug,
    <T as Resource>::DynamicType: Default,
{
    iupsert(client, obj, status).await
}

#[cfg(feature = "tracing")]
/// upsert the given kubernetes object, get it and create it, if it does not
/// exist or else patch it
pub async fn upsert<T>(client: Client, obj: &T, status: bool) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope>
        + ResourceExt
        + Serialize
        + DeserializeOwned
        + Clone
        + Debug,
    <T as Resource>::DynamicType: Default,
{
    iupsert(client, obj, status)
        .instrument(tracing::info_span!("resource::upsert"))
        .await
}

/// upsert the given kubernetes object, get it and create it, if it does not
/// exist or else patch it
async fn iupsert<T>(client: Client, obj: &T, status: bool) -> Result<T, kube::Error>
where
    T: Resource<Scope = NamespaceResourceScope>
        + ResourceExt
        + Serialize
        + DeserializeOwned
        + Clone
        + Debug,
    <T as Resource>::DynamicType: Default,
{
    let (ns, name) = namespaced_name(obj);
    if let Some(o) = get(client.to_owned(), &ns, &name).await? {
        let p = diff(&o, obj).map_err(kube::Error::SerdeError)?;
        let mut obj = patch(client.to_owned(), obj, p.to_owned()).await?;

        // todo: change this boolean to a polymorphic implementation instead
        if status {
            obj = patch_status(client, obj, p).await?;
        }

        return Ok(obj);
    }

    create(client, obj).await
}

#[cfg_attr(feature = "tracing", tracing::instrument)]
/// returns a owner reference object pointing to the given resource
pub fn owner_reference<T>(obj: &T) -> OwnerReference
where
    T: Resource<Scope = NamespaceResourceScope> + ResourceExt + CustomResourceExt + Debug,
{
    let api_resource = T::api_resource();

    OwnerReference {
        api_version: api_resource.api_version,
        block_owner_deletion: Some(true),
        controller: None,
        kind: api_resource.kind,
        name: obj.name_any(),
        uid: obj
            .uid()
            .expect("to have an unique identifier provided by kubernetes"),
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument)]
/// returns a object reference pointing to the given resource
pub fn object_reference<T>(obj: &T) -> ObjectReference
where
    T: Resource<Scope = NamespaceResourceScope> + ResourceExt + CustomResourceExt + Debug,
{
    let api_resource = T::api_resource();

    ObjectReference {
        api_version: Some(api_resource.api_version),
        kind: Some(api_resource.kind),
        name: Some(obj.name_any()),
        uid: obj.uid(),
        namespace: obj.namespace(),
        resource_version: obj.resource_version(),
        ..Default::default()
    }
}
