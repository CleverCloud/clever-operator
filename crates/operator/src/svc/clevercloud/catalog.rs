//! # Catalog module
//!
//! This module compares the add-ons the operator supports with the ones the
//! api exposes, and reports the drift between them when the daemon starts.
//!
//! The list of supported add-ons is fixed when the operator is built, while a
//! self-hosted api does not necessarily expose the same catalog as the public
//! platform: a custom resource whose provider the api does not expose fails on
//! every reconciliation, and an add-on the api exposes may have no custom
//! resource yet. Reporting both once, at startup, is a diagnostic: nothing here
//! changes the behaviour of the operator or prevents it from starting.

use std::{
    collections::BTreeSet,
    fmt::{self, Display, Formatter},
    time::Duration,
};

use clevercloud_sdk::{
    v2::{addon::Provider, plan},
    v4::addon_provider::AddonProviderId,
};
use tracing::{debug, warn};

use crate::svc::clevercloud::client::Client;

// -----------------------------------------------------------------------------
// Constants

/// How long the daemon waits for the catalog of the api before giving up on the
/// check: the api of an air-gapped installation may be slow or unreachable, and
/// this must not hold the controllers back.
pub const TIMEOUT: Duration = Duration::from_secs(10);

// -----------------------------------------------------------------------------
// Supported structure

/// An add-on the operator supports: the kind of its custom resource and the
/// provider the add-on is provisioned from.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Supported {
    pub kind: &'static str,
    pub provider: AddonProviderId,
}

impl Display for Supported {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.kind, self.provider)
    }
}

// -----------------------------------------------------------------------------
// Drift structure

/// The add-ons the api and the operator disagree on.
#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub struct Drift {
    /// Identifiers of the providers the api exposes that the operator has no
    /// custom resource for, sorted and deduplicated.
    pub unsupported: Vec<String>,
    /// Add-ons the operator supports whose provider the api does not expose,
    /// in the order they were registered.
    pub missing: Vec<Supported>,
}

impl Drift {
    pub fn is_empty(&self) -> bool {
        self.unsupported.is_empty() && self.missing.is_empty()
    }
}

// -----------------------------------------------------------------------------
// Helpers

/// Returns the drift between the providers the api exposes and the add-ons the
/// operator supports.
///
/// Identifiers are compared exactly, the way the plan resolution looks a
/// provider up: an add-on reported as missing here is one the reconciliation
/// would fail to find the provider of. An empty catalog therefore marks every
/// supported add-on as missing.
pub fn compare(providers: &[Provider], supported: &[Supported]) -> Drift {
    let exposed: BTreeSet<&str> = providers
        .iter()
        .map(|provider| provider.id.as_str())
        .collect();

    let known: BTreeSet<&str> = supported
        .iter()
        .map(|addon| addon.provider.as_str())
        .collect();

    Drift {
        unsupported: exposed
            .difference(&known)
            .map(ToString::to_string)
            .collect(),
        missing: supported
            .iter()
            .filter(|addon| !exposed.contains(addon.provider.as_str()))
            .cloned()
            .collect(),
    }
}

/// Fetches the catalog of the api and logs how it drifts from the supported
/// add-ons: at the `warn` level when they disagree, `debug` when they match.
///
/// This is a diagnostic and never fails: an api that cannot be reached, answers
/// something unexpected or does not answer within [`TIMEOUT`] is reported at the
/// `warn` level as well, and the daemon starts regardless.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub async fn check(client: &Client, supported: &[Supported]) {
    check_within(client, supported, TIMEOUT).await;
}

/// Same as [`check`], with the time to wait for the api.
async fn check_within(client: &Client, supported: &[Supported], timeout: Duration) {
    let providers = match tokio::time::timeout(timeout, plan::list(client)).await {
        Ok(Ok(providers)) => providers,
        Ok(Err(err)) => {
            warn!(
                error = %err,
                "Skipping the addon catalog check, the api could not be queried"
            );
            return;
        }
        Err(_) => {
            warn!(
                timeout = ?timeout,
                "Skipping the addon catalog check, the api did not answer in time"
            );
            return;
        }
    };

    let drift = compare(&providers, supported);

    if drift.is_empty() {
        debug!(
            providers = supported.len(),
            "The addon catalog of the api matches the add-ons the operator supports"
        );
        return;
    }

    if !drift.unsupported.is_empty() {
        warn!(
            providers = %drift.unsupported.join(", "),
            "The api exposes addon providers the operator does not support, no custom resource maps to them"
        );
    }

    if !drift.missing.is_empty() {
        warn!(
            addons = %join(&drift.missing),
            "The api does not expose the provider of supported add-ons, custom resources of these kinds will fail to reconcile"
        );
    }
}

/// Renders the add-ons as a comma-separated list of `Kind (provider)`.
fn join(addons: &[Supported]) -> String {
    addons
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use clevercloud_sdk::{
        Credentials, oauth10a::reqwest, v2::addon::Provider, v4::addon_provider::AddonProviderId,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::timeout,
    };

    use super::{Client, Drift, Supported, TIMEOUT, check_within, compare};

    /// Builds a provider as the api describes it, `id` being the only field
    /// the comparison reads.
    fn provider(id: &str) -> Provider {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "name": id,
            "website": "https://www.clever-cloud.com",
            "supportEmail": "support@clever-cloud.com",
            "googlePlusName": "",
            "twitterName": "",
            "analyticsId": "",
            "shortDesc": "",
            "longDesc": "",
            "logoUrl": "",
            "status": "RELEASE",
            "openInNewTab": false,
            "canUpgrade": false,
            "regions": ["par"],
            "plans": []
        }))
        .expect("to deserialize a provider")
    }

    /// Builds the catalog the api answers with.
    fn providers(ids: &[&str]) -> Vec<Provider> {
        ids.iter().map(|id| provider(id)).collect()
    }

    fn addon(kind: &'static str, provider: AddonProviderId) -> Supported {
        Supported { kind, provider }
    }

    /// The add-ons the operator supports in these tests.
    fn supported() -> Vec<Supported> {
        vec![
            addon("PostgreSql", AddonProviderId::PostgreSql),
            addon("Redis", AddonProviderId::Redis),
            addon("KV", AddonProviderId::KV),
        ]
    }

    #[test]
    fn identical_catalogs_have_no_drift() {
        let drift = compare(
            &providers(&["postgresql-addon", "redis-addon", "kv"]),
            &supported(),
        );

        assert!(drift.is_empty(), "unexpected drift: {drift:?}");
        assert_eq!(drift, Drift::default());
    }

    #[test]
    fn a_provider_the_operator_does_not_support_is_reported() {
        let drift = compare(
            &providers(&["postgresql-addon", "redis-addon", "kv", "addon-new"]),
            &supported(),
        );

        assert_eq!(
            drift,
            Drift {
                unsupported: vec!["addon-new".to_string()],
                missing: vec![],
            }
        );
    }

    #[test]
    fn a_supported_addon_the_api_does_not_expose_is_reported_with_its_kind() {
        let drift = compare(
            &providers(&["postgresql-addon", "redis-addon"]),
            &supported(),
        );

        assert_eq!(
            drift,
            Drift {
                unsupported: vec![],
                missing: vec![addon("KV", AddonProviderId::KV)],
            }
        );
        assert_eq!(drift.missing[0].to_string(), "KV (kv)");
    }

    #[test]
    fn both_directions_are_reported_at_once() {
        let drift = compare(
            &providers(&["postgresql-addon", "kv", "addon-new"]),
            &supported(),
        );

        assert_eq!(
            drift,
            Drift {
                unsupported: vec!["addon-new".to_string()],
                missing: vec![addon("Redis", AddonProviderId::Redis)],
            }
        );
    }

    /// An empty catalog is a valid answer of the api, every supported add-on is
    /// then missing and nothing is unsupported.
    #[test]
    fn an_empty_catalog_marks_every_supported_addon_as_missing() {
        let drift = compare(&[], &supported());

        assert_eq!(
            drift,
            Drift {
                unsupported: vec![],
                missing: supported(),
            }
        );
    }

    /// The order of the api and duplicates in its answer must not leak into the
    /// report, while the supported add-ons keep the order they were registered
    /// in.
    #[test]
    fn the_report_is_ordered_and_deduplicated() {
        let drift = compare(
            &providers(&["addon-zed", "addon-alpha", "addon-zed"]),
            &supported(),
        );

        assert_eq!(
            drift.unsupported,
            vec!["addon-alpha".to_string(), "addon-zed".to_string()]
        );
        assert_eq!(drift.missing, supported());
    }

    /// Binds a listener answering `response` to the first request it receives
    /// and returns its endpoint.
    async fn api(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("to bind a local listener");

        let addr = listener.local_addr().expect("to get the listener address");

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("to accept a connection");
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];

            // The request has no body: the end of the headers is the end of it.
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
                .write_all(response.as_bytes())
                .await
                .expect("to write the response");
        });

        format!("http://{addr}")
    }

    /// Builds a client targeting `endpoint`, free of the ambient proxy
    /// configuration which would divert its requests away from the listener.
    fn client(endpoint: String) -> Client {
        Client::builder()
            .with_credentials(Credentials::Bearer {
                token: "token".to_string(),
            })
            .with_endpoint(endpoint)
            .build(
                reqwest::Client::builder()
                    .no_proxy()
                    .build()
                    .expect("to build an http client"),
            )
    }

    /// An api answering an empty catalog is reported, not treated as a failure.
    #[tokio::test]
    async fn an_empty_answer_of_the_api_does_not_abort() {
        let endpoint =
            api("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\n\r\n[]")
                .await;

        timeout(
            Duration::from_secs(10),
            check_within(&client(endpoint), &supported(), TIMEOUT),
        )
        .await
        .expect("the check to complete");
    }

    /// An api failing to answer the catalog is reported, not treated as a
    /// failure.
    #[tokio::test]
    async fn an_error_of_the_api_does_not_abort() {
        let endpoint = api("HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\n\r\n").await;

        timeout(
            Duration::from_secs(10),
            check_within(&client(endpoint), &supported(), TIMEOUT),
        )
        .await
        .expect("the check to complete");
    }

    /// An api that does not answer must not hold the daemon back beyond the
    /// time the check is granted.
    #[tokio::test]
    async fn an_api_that_does_not_answer_does_not_hold_the_daemon_back() {
        // The connection is never accepted: it completes in the backlog of the
        // listener and the request is never answered.
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("to bind a local listener");

        let addr = listener.local_addr().expect("to get the listener address");

        timeout(
            Duration::from_secs(10),
            check_within(
                &client(format!("http://{addr}")),
                &supported(),
                Duration::from_millis(100),
            ),
        )
        .await
        .expect("the check to have given up on the api");
    }
}
