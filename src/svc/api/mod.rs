//! # Api module
//!
//! This module provide the clever-cloud api client, resources models and helpers

use async_trait::async_trait;
use bytes::Buf;
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    Body, Method, StatusCode,
};
use hyper_tls::HttpsConnector;
use serde::{de::DeserializeOwned, Serialize};

#[async_trait]
pub trait Request {
    type Error;

    async fn request<T, U>(
        &self,
        method: &Method,
        path: &str,
        payload: &T,
    ) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync;
}

#[async_trait]
pub trait RestClient {
    type Error;

    async fn get<T>(&self, path: &str) -> Result<T, Self::Error>
    where
        T: DeserializeOwned + Send + Sync;

    async fn post<T, U>(&self, path: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync;

    async fn put<T, U>(&self, path: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync;

    async fn patch<T, U>(&self, path: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync;

    async fn delete(&self, path: &str) -> Result<(), Self::Error>;
}

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("failed to build request, {0}")]
    RequestBuilder(hyper::http::Error),
    #[error("failed to execute request, {0}")]
    Request(hyper::Error),
    #[error("failed to execute request, got response code {0}")]
    StatusCode(StatusCode),
    #[error("failed to aggregate body, {0}")]
    BodyAggregation(hyper::Error),
    #[error("failed to serialize body, {0}")]
    Serialize(serde_json::Error),
    #[error("failed to deserialize body, {0}")]
    Deserialize(serde_json::Error),
}

#[derive(Clone, Debug)]
pub struct Client {
    inner: hyper::Client<HttpsConnector<HttpConnector<GaiResolver>>, Body>,
}

#[async_trait]
impl Request for Client {
    type Error = ClientError;

    async fn request<T, U>(
        &self,
        method: &Method,
        path: &str,
        payload: &T,
    ) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync,
    {
        let buf = serde_json::to_vec(payload).map_err(ClientError::Serialize)?;
        let req = hyper::Request::builder()
            .method(method)
            .uri(path)
            .body(Body::from(buf))
            .map_err(ClientError::RequestBuilder)?;

        let res = self
            .inner
            .request(req)
            .await
            .map_err(ClientError::Request)?;

        let status = res.status();
        if !status.is_success() {
            return Err(ClientError::StatusCode(status));
        }

        let buf = hyper::body::aggregate(res.into_body())
            .await
            .map_err(ClientError::BodyAggregation)?;

        Ok(serde_json::from_reader(buf.reader()).map_err(ClientError::Deserialize)?)
    }
}

#[async_trait]
impl RestClient for Client {
    type Error = ClientError;

    async fn get<T>(&self, path: &str) -> Result<T, Self::Error>
    where
        T: DeserializeOwned + Send + Sync,
    {
        let req = hyper::Request::builder()
            .method(Method::GET)
            .uri(path)
            .body(Body::empty())
            .map_err(ClientError::RequestBuilder)?;

        let res = self
            .inner
            .request(req)
            .await
            .map_err(ClientError::Request)?;

        let status = res.status();
        if !status.is_success() {
            return Err(ClientError::StatusCode(status));
        }

        let buf = hyper::body::aggregate(res.into_body())
            .await
            .map_err(ClientError::BodyAggregation)?;

        Ok(serde_json::from_reader(buf.reader()).map_err(ClientError::Deserialize)?)
    }

    async fn post<T, U>(&self, path: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync,
    {
        self.request(&Method::POST, path, payload).await
    }

    async fn put<T, U>(&self, path: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync,
    {
        self.request(&Method::PUT, path, payload).await
    }

    async fn patch<T, U>(&self, path: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync,
    {
        self.request(&Method::PATCH, path, payload).await
    }

    async fn delete(&self, path: &str) -> Result<(), Self::Error> {
        let req = hyper::Request::builder()
            .method(Method::GET)
            .uri(path)
            .body(Body::empty())
            .map_err(ClientError::RequestBuilder)?;

        let res = self
            .inner
            .request(req)
            .await
            .map_err(ClientError::Request)?;

        let status = res.status();
        if !status.is_success() {
            return Err(ClientError::StatusCode(status));
        }

        Ok(())
    }
}
