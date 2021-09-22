//! # Secret module
//!
//! This module provide helpers to generate secrets from a custom resource

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::Secret;
use kube::{
    api::{ObjectMeta, PostParams},
    Api, Client, CustomResourceExt, ResourceExt,
};
use slog_scope::debug;

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

pub async fn get<T>(client: Client, obj: &T) -> Result<Option<Secret>, kube::Error>
where
    T: ResourceExt,
{
    let (namespace, _) = resource::namespaced_name(obj);
    let api: Api<Secret> = Api::namespaced(client, &namespace);

    debug!("execute a request to retrieve secret"; "kind" => "Secret", "namespace" => &namespace, "name" => &name(obj));
    match api.get(&name(obj)).await {
        Ok(secret) => Ok(Some(secret)),
        Err(kube::Error::Api(err)) if err.code == 404 => Ok(None),
        Err(err) => Err(err),
    }
}

pub async fn create(client: Client, secret: &Secret) -> Result<Secret, kube::Error> {
    let (namespace, name) = resource::namespaced_name(secret);
    let api: Api<Secret> = Api::namespaced(client, &namespace);

    debug!("execute a request to create a secret"; "kind" => "Secret", "namespace" => &namespace, "name" => &name);
    api.create(&PostParams::default(), secret).await
}

pub async fn upsert<T>(client: Client, obj: &T, secret: &Secret) -> Result<Secret, kube::Error>
where
    T: ResourceExt,
{
    if let Some(s) = get(client.to_owned(), obj).await? {
        let patch = resource::diff(&s, secret)?;
        return resource::patch(client, secret, patch).await;
    }

    create(client, secret).await
}
