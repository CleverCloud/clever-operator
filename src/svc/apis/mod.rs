//! # Api module
//!
//! This module provide the clever-cloud api client, resources models and helpers

use std::{
    collections::BTreeMap,
    convert::TryFrom,
    error::Error,
    fmt::{self, Display, Formatter},
    sync::Arc,
    time::{SystemTime, SystemTimeError},
};

use async_trait::async_trait;
use bytes::Buf;
use hmac::{crypto_mac::InvalidKeyLength, Hmac, Mac, NewMac};
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    header, Body, Method, StatusCode,
};
use hyper_tls::HttpsConnector;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::Sha512;
use slog_scope::{error, trace};
use uuid::Uuid;

use crate::svc::cfg::{Api, Configuration};

pub mod addon;
pub mod myself;

// -----------------------------------------------------------------------------
// Types
type HmacSha512 = Hmac<Sha512>;

// -----------------------------------------------------------------------------
// Request trait

#[async_trait]
pub trait Request {
    type Error;

    async fn request<T, U>(
        &self,
        method: &Method,
        endpoint: &str,
        payload: &T,
    ) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync;
}

// -----------------------------------------------------------------------------
// RestClient trait

#[async_trait]
pub trait RestClient {
    type Error;

    async fn get<T>(&self, endpoint: &str) -> Result<T, Self::Error>
    where
        T: DeserializeOwned + Send + Sync;

    async fn post<T, U>(&self, endpoint: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync;

    async fn put<T, U>(&self, endpoint: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync;

    async fn patch<T, U>(&self, endpoint: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync;

    async fn delete(&self, endpoint: &str) -> Result<(), Self::Error>;
}

// -----------------------------------------------------------------------------
// ClientCredentials structure

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ClientCredentials {
    pub token: String,
    pub secret: String,
    pub consumer_key: String,
    pub consumer_secret: String,
}

impl From<&Api> for ClientCredentials {
    fn from(api: &Api) -> Self {
        Self {
            token: api.token.to_owned(),
            secret: api.secret.to_owned(),
            consumer_key: api.consumer_key.to_owned(),
            consumer_secret: api.consumer_secret.to_owned(),
        }
    }
}

impl From<Arc<Configuration>> for ClientCredentials {
    fn from(configuration: Arc<Configuration>) -> Self {
        Self::from(&configuration.api)
    }
}

// -----------------------------------------------------------------------------
// OAuth1 trait

pub const OAUTH1_CONSUMER_KEY: &str = "oauth_consumer_key";
pub const OAUTH1_NONCE: &str = "oauth_nonce";
pub const OAUTH1_SIGNATURE: &str = "oauth_signature";
pub const OAUTH1_SIGNATURE_METHOD: &str = "oauth_signature_method";
pub const OAUTH1_SIGNATURE_HMAC_SHA512: &str = "HMAC-SHA512";
pub const OAUTH1_TIMESTAMP: &str = "oauth_timestamp";
pub const OAUTH1_VERSION: &str = "oauth_version";
pub const OAUTH1_VERSION_1: &str = "1.0";
pub const OAUTH1_TOKEN: &str = "oauth_token";

pub trait OAuth1 {
    type Error;

    // `params` returns OAuth1 parameters without the signature one
    fn params(&self) -> BTreeMap<String, String>;

    // `signature` returns the computed signature from given parameters
    fn signature(&self, method: &str, endpoint: &str) -> Result<String, Self::Error>;

    // `signing_key` returns the key that is used to signed the signature
    fn signing_key(&self) -> String;

    // `sign` returns OAuth1 formatted Authorization header value
    fn sign(&self, method: &str, endpoint: &str) -> Result<String, Self::Error> {
        let signature = self.signature(method, endpoint)?;
        let mut params = self.params();

        params.insert(OAUTH1_SIGNATURE.to_string(), signature);

        let mut base = params
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, urlencoding::encode(v)))
            .collect::<Vec<_>>();

        base.sort();

        Ok(format!("OAuth {}", base.join(", ")))
    }
}

// -----------------------------------------------------------------------------
// ResponseError structure

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResponseError {
    #[serde(rename = "id")]
    pub id: u32,
    #[serde(rename = "message")]
    pub message: String,
    #[serde(rename = "type")]
    pub kind: String,
}

impl Display for ResponseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "got response {} {}, {}",
            self.kind, self.id, self.message
        )
    }
}

impl Error for ResponseError {}

// -----------------------------------------------------------------------------
// SignerError enum

#[derive(thiserror::Error, Debug)]
pub enum SignerError {
    #[error("failed to compute invalid key length, {0}")]
    Digest(InvalidKeyLength),
    #[error("failed to compute time since unix epoch, {0}")]
    UnixEpochTime(SystemTimeError),
    #[error("failed to parse signature paramater, {0}")]
    Parse(String),
}

// -----------------------------------------------------------------------------
// Signer structure

pub struct Signer {
    pub nonce: String,
    pub timestamp: u64,
    pub credentials: ClientCredentials,
}

impl OAuth1 for Signer {
    type Error = SignerError;

    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();

        params.insert(
            OAUTH1_CONSUMER_KEY.to_string(),
            self.credentials.consumer_key.to_string(),
        );
        params.insert(OAUTH1_NONCE.to_string(), self.nonce.to_string());
        params.insert(
            OAUTH1_SIGNATURE_METHOD.to_string(),
            OAUTH1_SIGNATURE_HMAC_SHA512.to_string(),
        );
        params.insert(OAUTH1_TIMESTAMP.to_string(), self.timestamp.to_string());
        params.insert(OAUTH1_VERSION.to_string(), OAUTH1_VERSION_1.to_string());
        params.insert(OAUTH1_TOKEN.to_string(), self.credentials.token.to_string());
        params
    }

    fn signing_key(&self) -> String {
        format!(
            "{}&{}",
            urlencoding::encode(&self.credentials.consumer_secret.to_owned()),
            urlencoding::encode(&self.credentials.secret.to_owned())
        )
    }

    fn signature(&self, method: &str, endpoint: &str) -> Result<String, Self::Error> {
        let (host, query) = match endpoint.find(|c| '?' == c) {
            None => (endpoint, ""),
            // split one character further to not get the '?' character
            Some(position) => endpoint.split_at(position),
        };

        let query = query.strip_prefix('?').unwrap_or(query);
        let mut params = self.params();

        if !query.is_empty() {
            for qparam in query.split('&') {
                let (k, v) = qparam.split_at(qparam.find('=').ok_or_else(|| {
                    SignerError::Parse(format!("failed to parse query parameter, {}", qparam))
                })?);

                if !params.contains_key(k) {
                    params.insert(k.to_string(), v.strip_prefix('=').unwrap_or(v).to_owned());
                }
            }
        }

        let mut params = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>();

        params.sort();

        let base = format!(
            "{}&{}&{}",
            urlencoding::encode(method),
            urlencoding::encode(host),
            urlencoding::encode(&params.join("&"))
        );

        let mut hasher = HmacSha512::new_from_slice(self.signing_key().as_bytes())
            .map_err(SignerError::Digest)?;

        hasher.update(base.as_bytes());

        let digest = hasher.finalize().into_bytes();
        Ok(base64::encode(digest.as_slice()))
    }
}

impl TryFrom<ClientCredentials> for Signer {
    type Error = SignerError;

    fn try_from(credentials: ClientCredentials) -> Result<Self, Self::Error> {
        let nonce = Uuid::new_v4().to_string();
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(SignerError::UnixEpochTime)?
            .as_secs();

        Ok(Self {
            nonce,
            timestamp,
            credentials,
        })
    }
}

// -----------------------------------------------------------------------------
// ClientError enum

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("failed to build request, {0}")]
    RequestBuilder(hyper::http::Error),
    #[error("failed to execute request, {0}")]
    Request(hyper::Error),
    #[error("failed to execute request, got status code {0}, {1}")]
    StatusCode(StatusCode, ResponseError),
    #[error("failed to aggregate body, {0}")]
    BodyAggregation(hyper::Error),
    #[error("failed to serialize body, {0}")]
    Serialize(serde_json::Error),
    #[error("failed to deserialize body, {0}")]
    Deserialize(serde_json::Error),
    #[error("failed to create request signer, {0}")]
    Signer(SignerError),
    #[error("failed to compute request digest, {0}")]
    Digest(SignerError),
}

// -----------------------------------------------------------------------------
// Client structure

#[derive(Clone, Debug)]
pub struct Client {
    inner: hyper::Client<HttpsConnector<HttpConnector<GaiResolver>>, Body>,
    credentials: Option<ClientCredentials>,
}

#[async_trait]
impl Request for Client {
    type Error = ClientError;

    async fn request<T, U>(
        &self,
        method: &Method,
        endpoint: &str,
        payload: &T,
    ) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync,
    {
        let buf = serde_json::to_vec(payload).map_err(ClientError::Serialize)?;
        let mut builder = hyper::Request::builder();
        if let Some(credentials) = &self.credentials {
            let signer = Signer::try_from(credentials.to_owned()).map_err(ClientError::Signer)?;

            builder = builder.header(
                header::AUTHORIZATION,
                signer
                    .sign(method.as_str(), endpoint)
                    .map_err(ClientError::Digest)?,
            );
        }

        let req = builder
            .method(method)
            .uri(endpoint)
            .body(Body::from(buf.to_owned()))
            .map_err(ClientError::RequestBuilder)?;

        trace!("execute request"; "endpoint" => endpoint, "method" => method.to_string(), "body" => String::from_utf8_lossy(&buf).to_string());
        let res = self
            .inner
            .request(req)
            .await
            .map_err(ClientError::Request)?;

        let status = res.status();
        let buf = hyper::body::aggregate(res.into_body())
            .await
            .map_err(ClientError::BodyAggregation)?;

        trace!("got response"; "endpoint" => endpoint, "method" => method.to_string(), "status" => status.as_u16());
        if !status.is_success() {
            return Err(ClientError::StatusCode(
                status,
                serde_json::from_reader(buf.reader()).map_err(ClientError::Deserialize)?,
            ));
        }

        Ok(serde_json::from_reader(buf.reader()).map_err(ClientError::Deserialize)?)
    }
}

#[async_trait]
impl RestClient for Client {
    type Error = ClientError;

    async fn get<T>(&self, endpoint: &str) -> Result<T, Self::Error>
    where
        T: DeserializeOwned + Send + Sync,
    {
        let method = &Method::GET;
        let mut builder = hyper::Request::builder();
        if let Some(credentials) = &self.credentials {
            let signer = Signer::try_from(credentials.to_owned()).map_err(ClientError::Signer)?;

            builder = builder.header(
                header::AUTHORIZATION,
                signer
                    .sign(method.as_str(), endpoint)
                    .map_err(ClientError::Digest)?,
            );
        }

        let req = builder
            .method(method)
            .uri(endpoint)
            .body(Body::empty())
            .map_err(ClientError::RequestBuilder)?;

        trace!("execute request"; "endpoint" => endpoint, "method" => method.to_string(), "body" => "<none>");
        let res = self
            .inner
            .request(req)
            .await
            .map_err(ClientError::Request)?;

        let status = res.status();
        let buf = hyper::body::aggregate(res.into_body())
            .await
            .map_err(ClientError::BodyAggregation)?;

        trace!("got response"; "endpoint" => endpoint, "method" => method.to_string(), "status" => status.as_u16());
        if !status.is_success() {
            return Err(ClientError::StatusCode(
                status,
                serde_json::from_reader(buf.reader()).map_err(ClientError::Deserialize)?,
            ));
        }

        Ok(serde_json::from_reader(buf.reader()).map_err(ClientError::Deserialize)?)
    }

    async fn post<T, U>(&self, endpoint: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync,
    {
        self.request(&Method::POST, endpoint, payload).await
    }

    async fn put<T, U>(&self, endpoint: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync,
    {
        self.request(&Method::PUT, endpoint, payload).await
    }

    async fn patch<T, U>(&self, endpoint: &str, payload: &T) -> Result<U, Self::Error>
    where
        T: Serialize + Send + Sync,
        U: DeserializeOwned + Send + Sync,
    {
        self.request(&Method::PATCH, endpoint, payload).await
    }

    async fn delete(&self, endpoint: &str) -> Result<(), Self::Error> {
        let method = &Method::DELETE;
        let mut builder = hyper::Request::builder();
        if let Some(credentials) = &self.credentials {
            let signer = Signer::try_from(credentials.to_owned()).map_err(ClientError::Signer)?;

            builder = builder.header(
                header::AUTHORIZATION,
                signer
                    .sign(method.as_str(), endpoint)
                    .map_err(ClientError::Digest)?,
            );
        }

        let req = builder
            .method(method)
            .uri(endpoint)
            .body(Body::empty())
            .map_err(ClientError::RequestBuilder)?;

        trace!("execute request"; "endpoint" => endpoint, "method" => method.to_string(), "body" => "<none>");
        let res = self
            .inner
            .request(req)
            .await
            .map_err(ClientError::Request)?;

        let status = res.status();
        let buf = hyper::body::aggregate(res.into_body())
            .await
            .map_err(ClientError::BodyAggregation)?;

        trace!("got response"; "endpoint" => endpoint, "method" => method.to_string(), "status" => status.as_u16());
        if !status.is_success() {
            return Err(ClientError::StatusCode(
                status,
                serde_json::from_reader(buf.reader()).map_err(ClientError::Deserialize)?,
            ));
        }

        Ok(())
    }
}

impl Default for Client {
    fn default() -> Self {
        let connector = HttpsConnector::new();
        let inner = hyper::Client::builder().build(connector);

        Self {
            inner,
            credentials: None,
        }
    }
}

impl From<Arc<Configuration>> for Client {
    fn from(configuration: Arc<Configuration>) -> Self {
        Self::from(ClientCredentials::from(configuration))
    }
}

impl From<ClientCredentials> for Client {
    fn from(credentials: ClientCredentials) -> Self {
        let mut client = Self::default();
        client.set_credentials(Some(credentials));
        client
    }
}

impl Client {
    pub fn set_credentials(&mut self, credentials: Option<ClientCredentials>) {
        self.credentials = credentials;
    }
}
