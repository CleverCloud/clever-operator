//! # Client module
//!
//! This module provides helpers to create a clever-cloud client

use clevercloud_sdk::oauth10a::reqwest;
use k8s_openapi::api::core::v1::Secret;

use std::string::FromUtf8Error;

use crate::svc::{
    cfg::{self, Api, NamespaceConfiguration},
    k8s::resource,
};

// -----------------------------------------------------------------------------
// types

pub type Client = clevercloud_sdk::Client;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to create clever cloud client, {0}")]
    CreateCleverClient(reqwest::Error),
    #[error("failed to load the certificate authority bundle, {0}")]
    CaBundle(String),
    #[error("failed to retrieve data from secret '{0}/{1}'")]
    SecretData(String, String),
    #[error("failed to find key '{0}' in secret '{1}/{2}")]
    SecretKey(&'static str, String, String),
    #[error("failed to read key '{0}' of secret '{1}/{2}' as utf-8, {3}")]
    Utf8(&'static str, String, String, FromUtf8Error),
    #[error("failed to parse configuration file, {0}")]
    Configuration(Box<cfg::Error>),
}

impl From<cfg::Error> for Error {
    fn from(err: cfg::Error) -> Self {
        Self::Configuration(Box::new(err))
    }
}

// -----------------------------------------------------------------------------
// helpers

/// Returns a clever cloud client built from the given api configuration.
///
/// When no endpoint is configured, we hand the credentials over to the sdk which
/// picks the public api that matches them.
///
/// The certificate authorities of `api.ca_bundle` are added on top of the ones
/// compiled into the http client, and not in place of them: a self-hosted
/// installation may well be served by a mixed chain.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn new(api: Api) -> Result<Client, Error> {
    let mut builder = reqwest::Client::builder();

    if let Some(path) = &api.ca_bundle {
        for certificate in cfg::read_ca_bundle(path).map_err(Error::CaBundle)? {
            builder = builder.add_root_certificate(certificate);
        }
    }

    let http = builder.build().map_err(Error::CreateCleverClient)?;

    Ok(with_http_client(api, http))
}

/// Same as [`new`], with the http client to build upon.
///
/// Only the tests need this seam: they must not inherit the ambient proxy
/// configuration, which would divert their requests away from their listener.
/// The tls configuration, `api.ca_bundle` included, belongs to the client the
/// caller hands over.
fn with_http_client(api: Api, http: reqwest::Client) -> Client {
    let Api {
        endpoint,
        ca_bundle: _,
        credentials,
    } = api;

    let mut builder = Client::builder().with_credentials(credentials);

    // Leaving the endpoint unset lets the sdk pick the public api matching the
    // credentials, which is the api bridge for a bare bearer token.
    if let Some(endpoint) = endpoint {
        builder = builder.with_endpoint(endpoint);
    }

    builder.build(http)
}

/// Returns a client built from the per-namespace override secret, falling back
/// on `api` for everything the secret does not declare.
//
// `Api` carries the credentials, whose `Debug` is not redacted: the span records
// the keys that are not secret rather than the whole section.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(skip_all, fields(endpoint = ?api.endpoint, ca_bundle = ?api.ca_bundle))
)]
pub async fn try_from(secret: Secret, api: &Api) -> Result<Client, Error> {
    let (namespace, name) = resource::namespaced_name(&secret);

    let data = match &secret.data {
        Some(data) => data,
        None => {
            return Err(Error::SecretData(namespace, name));
        }
    };

    let Some(bytestr) = data.get("config") else {
        return Err(Error::SecretKey("config", namespace, name));
    };

    // The api server is the one doing the base64 round trip: `ByteString` already
    // holds the content of the key.
    let content = String::from_utf8(bytestr.0.to_owned())
        .map_err(|err| Error::Utf8("config", namespace, name, err))?;

    let mut configuration = NamespaceConfiguration::try_from(content.as_str())?;

    configuration.api.inherit_from(api);

    new(configuration.api)
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf, time::Duration};

    use clevercloud_sdk::{Credentials, PUBLIC_ENDPOINT, oauth10a::reqwest, v2::myself};
    use k8s_openapi::{ByteString, api::core::v1::Secret};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::timeout,
    };

    use crate::svc::cfg::{CA_BUNDLE, missing_test_file, write_test_file};

    use super::{Api, new, try_from, with_http_client};

    /// Builds the api section the operator itself was configured with.
    fn api(endpoint: Option<&str>, ca_bundle: Option<PathBuf>) -> Api {
        Api {
            endpoint: endpoint.map(ToOwned::to_owned),
            ca_bundle,
            credentials: Credentials::Bearer {
                token: "token".to_string(),
            },
        }
    }

    /// Builds the per-namespace override secret holding `content`.
    fn secret(content: &[u8]) -> Secret {
        let mut secret = Secret::default();

        secret.metadata.namespace = Some("namespace".to_string());
        secret.metadata.name = Some("clever-kubernetes-operator".to_string());
        secret.data = Some(BTreeMap::from([(
            "config".to_string(),
            ByteString(content.to_vec()),
        )]));

        secret
    }

    /// A configured endpoint must be the one the requests are sent to, host
    /// included.
    #[tokio::test]
    async fn configured_endpoint_receives_the_requests() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("to bind a local listener");

        let addr = listener.local_addr().expect("to get the listener address");

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("to accept a connection");
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];

            loop {
                let len = stream.read(&mut buf).await.expect("to read the request");
                if len == 0 {
                    break;
                }

                request.extend_from_slice(&buf[..len]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }

            stream
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\n\r\n")
                .await
                .expect("to write the response");

            String::from_utf8_lossy(&request).to_string()
        });

        let client = with_http_client(
            api(Some(&format!("http://{addr}")), None),
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("to build an http client"),
        );

        // The listener answers a 500 on purpose, only the request it received
        // matters here.
        let _ = myself::get(&client).await;

        let request = timeout(Duration::from_secs(10), server)
            .await
            .expect("the listener to have received a request")
            .expect("the listener task to join");

        assert!(
            request.starts_with("GET /v2/self "),
            "unexpected request line in: {request}"
        );
        assert!(
            request
                .to_lowercase()
                .contains(&format!("host: {addr}").to_lowercase()),
            "unexpected host header in: {request}"
        );
    }

    /// Leaving the endpoint unset must keep the public api as the target.
    #[test]
    fn missing_endpoint_falls_back_on_the_public_api() {
        let client = new(Api {
            endpoint: None,
            ca_bundle: None,
            credentials: Credentials::OAuth1 {
                token: String::new(),
                secret: String::new(),
                consumer_key: String::new(),
                consumer_secret: String::new(),
            },
        })
        .expect("to build a client");

        assert!(
            format!("{client:?}").contains(PUBLIC_ENDPOINT),
            "the client does not target {PUBLIC_ENDPOINT}"
        );
    }

    /// The content of the key is what the api server decoded, building the
    /// client must not expect a second layer of encoding.
    #[tokio::test]
    async fn secret_content_is_read_as_the_api_server_hands_it_over() {
        try_from(secret(b"[api]\ntoken = \"token\"\n"), &api(None, None))
            .await
            .expect("to build a client from the secret");
    }

    /// The secret overrides the credentials of a namespace, not the installation
    /// they belong to: without an endpoint of its own it must reach the same api
    /// as the rest of the operator.
    #[tokio::test]
    async fn namespace_secret_inherits_the_global_endpoint() {
        let client = try_from(
            secret(b"[api]\ntoken = \"token\"\n"),
            &api(Some("https://api.example.com"), None),
        )
        .await
        .expect("to build a client from the secret");

        assert!(
            format!("{client:?}").contains("https://api.example.com"),
            "the client does not target the inherited endpoint"
        );
    }

    /// An endpoint declared by the secret wins over the inherited one.
    #[tokio::test]
    async fn namespace_secret_endpoint_wins_over_the_inherited_one() {
        let client = try_from(
            secret(b"[api]\nendpoint = \"https://api.namespace.example.com\"\ntoken = \"token\"\n"),
            &api(Some("https://api.example.com"), None),
        )
        .await
        .expect("to build a client from the secret");

        assert!(
            format!("{client:?}").contains("https://api.namespace.example.com"),
            "the client does not target the endpoint of the secret"
        );
    }

    /// A bundle the client cannot be built upon must be reported with the file
    /// at fault, never swallowed into a handshake failure later on, and never
    /// panic.
    #[test]
    fn unusable_ca_bundles_are_reported_with_their_path() {
        let cases = [
            ("absent", missing_test_file("client-absent-ca-bundle.pem")),
            (
                "text",
                write_test_file("client-text-ca-bundle.pem", "this is not a certificate\n"),
            ),
            (
                "truncated",
                write_test_file(
                    "client-truncated-ca-bundle.pem",
                    "-----BEGIN CERTIFICATE-----\nnot base64 !!!\n-----END CERTIFICATE-----\n",
                ),
            ),
        ];

        for (case, path) in cases {
            let err = new(api(None, Some(path.to_owned())))
                .expect_err(&format!("bundle '{case}' should have been rejected"));

            assert!(
                err.to_string().contains(&path.display().to_string()),
                "the error of '{case}' does not name the file at fault: {err}"
            );
        }
    }

    /// A usable bundle is added on top of the roots compiled into the client,
    /// which must still be built.
    #[test]
    fn a_usable_ca_bundle_builds_a_client() {
        let path = write_test_file("client-usable-ca-bundle.pem", CA_BUNDLE);

        new(api(None, Some(path))).expect("to build a client trusting the bundle");
    }

    /// The secret overrides the credentials of a namespace, not the installation
    /// they belong to: without a bundle of its own it must trust the same
    /// authorities as the rest of the operator.
    #[tokio::test]
    async fn namespace_secret_inherits_the_global_ca_bundle() {
        let path = missing_test_file("client-inherited-ca-bundle.pem");

        // The inherited bundle is not observable on the built client: pointing
        // it at a file that does not exist is what makes its use visible.
        let err = try_from(
            secret(b"[api]\ntoken = \"token\"\n"),
            &api(None, Some(path.to_owned())),
        )
        .await
        .expect_err("the inherited bundle to have been loaded");

        assert!(
            err.to_string().contains(&path.display().to_string()),
            "the secret did not inherit the bundle of the operator: {err}"
        );
    }

    /// A bundle declared by the secret wins over the inherited one.
    #[tokio::test]
    async fn namespace_secret_ca_bundle_wins_over_the_inherited_one() {
        let own = write_test_file("client-namespace-ca-bundle.pem", CA_BUNDLE);
        let inherited = missing_test_file("client-overridden-ca-bundle.pem");

        // Would the inherited bundle win, the client could not be built.
        try_from(
            secret(
                format!(
                    "[api]\nca_bundle = \"{}\"\ntoken = \"token\"\n",
                    own.display()
                )
                .as_bytes(),
            ),
            &api(None, Some(inherited)),
        )
        .await
        .expect("the bundle of the secret to have been used");
    }
}
