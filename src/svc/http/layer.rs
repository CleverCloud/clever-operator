//! # Layer module
//!
//! This module provides middlewares to give to the server implementation.
//! It could be seen as an interceptor in h2.

#[cfg(feature = "metrics")]
use std::sync::LazyLock;
use std::time::Instant;

use axum::{
    body::Body,
    http::{header, Request},
    middleware::Next,
};
#[cfg(feature = "metrics")]
use prometheus::{register_int_counter_vec, IntCounterVec};
use tracing::{info, info_span, Instrument};

// -----------------------------------------------------------------------------
// Telemetry

#[cfg(feature = "metrics")]
static ACCESS_REQUEST: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "http_access_requests_count",
        "Number of access request",
        &["method", "host", "status"]
    )
    .expect("'http_access_requests_count' to not be already registered")
});

#[cfg(feature = "metrics")]
static ACCESS_REQUEST_DURATION: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "http_access_requests_duration",
        "Duration of access request",
        &["method", "host", "status"]
    )
    .expect("'http_access_requests_duration' to not be already registered")
});

// -----------------------------------------------------------------------------
// Access

#[tracing::instrument(skip_all)]
pub async fn access(req: Request<Body>, next: Next) -> axum::response::Response {
    // ---------------------------------------------------------------------------------------------
    // Retrieve information
    let method = req.method().to_string();
    let uri = req.uri().to_string();
    let headers = req.headers();

    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();

    let referer = headers
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();

    let forwarded = headers
        .get(header::FORWARDED)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();

    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();

    let host = req
        .uri()
        .host()
        .unwrap_or_else(|| {
            headers
                .get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("<none>")
        })
        .to_string();

    // ---------------------------------------------------------------------------------------------
    // Call next handler
    let begin = Instant::now();
    let res = next.run(req).instrument(info_span!("next.run")).await;
    let duration = begin.elapsed().as_micros();

    // ---------------------------------------------------------------------------------------------
    // Emit the access log
    let status = res.status().as_u16();

    #[cfg(feature = "metrics")]
    ACCESS_REQUEST
        .with_label_values(&[&method.to_string(), &host.to_string(), &status.to_string()])
        .inc();

    #[cfg(feature = "metrics")]
    ACCESS_REQUEST_DURATION
        .with_label_values(&[&method.to_string(), &host.to_string(), &status.to_string()])
        .inc_by(duration as u64);

    info!(
        method = method,
        uri = uri,
        origin = origin,
        referer = referer,
        forwarded = forwarded,
        agent = agent,
        host = host,
        duration = format!("{duration}us"),
        status = status,
        "Request received"
    );

    res
}
