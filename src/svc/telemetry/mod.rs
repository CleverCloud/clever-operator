//! # Telemetry module
//!
//! This module expose telemetry measurements mainly metrics and tracing through
//! structures, enums and helpers

use std::{collections::BTreeMap, time::Instant};

use hyper::{
    header::{self, HeaderValue},
    Body, Method, Request, Response, StatusCode,
};
#[cfg(feature = "metrics")]
use once_cell::sync::Lazy;
#[cfg(feature = "metrics")]
use prometheus::{opts, register_counter_vec, CounterVec};
use tracing::info;

#[cfg(feature = "metrics")]
pub mod metrics;

// -----------------------------------------------------------------------------
// Telemetry

#[cfg(feature = "metrics")]
static SERVER_REQUEST_SUCCESS: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_operator_server_request_success",
            "number of successful request handled by the server",
        ),
        &["method", "path", "status"]
    )
    .expect("metrics 'kubernetes_operator_server_request_success' to not be already registered")
});

#[cfg(feature = "metrics")]
static SERVER_REQUEST_FAILURE: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_operator_server_request_failure",
            "number of failed request handled by the server",
        ),
        &["method", "path", "status"]
    )
    .expect("metrics 'kubernetes_operator_server_request_failure' to not be already registered")
});

#[cfg(feature = "metrics")]
static SERVER_REQUEST_DURATION: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        opts!(
            "kubernetes_operator_server_request_duration",
            "duration of request handled by the server",
        ),
        &["method", "path", "status", "unit"]
    )
    .expect("metrics 'kubernetes_operator_server_request_duration' to not be already registered")
});

// -----------------------------------------------------------------------------
// Error enum

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[cfg(feature = "metrics")]
    #[error("{0}")]
    Metrics(metrics::Error),
    #[error("failed to serialize payload, {0}")]
    Serialize(serde_json::Error),
}

// -----------------------------------------------------------------------------
// Helper methods

#[cfg_attr(feature = "trace", tracing::instrument)]
pub async fn router(req: Request<Body>) -> Result<Response<Body>, Error> {
    let begin = Instant::now();

    // -------------------------------------------------------------------------
    // Basic routing
    let result = match (req.method(), req.uri().path()) {
        (&Method::GET, "/healthz") => healthz(&req).await,
        #[cfg(feature = "metrics")]
        (&Method::GET, "/metrics") => metrics::handler(&req).await.map_err(Error::Metrics),
        _ => not_found(&req).await,
    };

    let duration = Instant::now().duration_since(begin).as_micros();

    // -------------------------------------------------------------------------
    // recover error
    let host = match req.uri().host() {
        Some(host) => host.to_string(),
        None => req
            .headers()
            .get("host")
            .map(|header| String::from_utf8_lossy(header.as_bytes()).to_string())
            .unwrap_or_else(|| String::from("<none>")),
    };

    match result {
        Ok(res) => {
            info!(
                method = req.method().as_str(),
                host = host,
                path = req.uri().path(),
                status = res.status().as_u16(),
                duration = duration,
                "Receive request"
            );

            #[cfg(feature = "metrics")]
            SERVER_REQUEST_SUCCESS
                .with_label_values(&[
                    req.method().as_str(),
                    req.uri().path(),
                    &res.status().as_u16().to_string(),
                ])
                .inc();

            #[cfg(feature = "metrics")]
            SERVER_REQUEST_DURATION
                .with_label_values(&[
                    req.method().as_str(),
                    req.uri().path(),
                    &res.status().as_u16().to_string(),
                    "us",
                ])
                .inc_by(duration as f64);

            Ok(res)
        }
        Err(err) => {
            // -----------------------------------------------------------------
            // Format error in a convenient way

            let mut map = BTreeMap::new();

            map.insert("error".to_string(), err.to_string());

            // -----------------------------------------------------------------
            // Serialize and send error

            let mut res = Response::default();

            res.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );

            // easter egg
            *res.status_mut() = StatusCode::IM_A_TEAPOT;
            *res.body_mut() =
                Body::from(serde_json::to_string_pretty(&map).map_err(Error::Serialize)?);

            info!(
                method = req.method().as_str(),
                host = host,
                path = req.uri().path(),
                status = res.status().as_u16(),
                duration = duration,
                "Receive request"
            );

            #[cfg(feature = "metrics")]
            SERVER_REQUEST_FAILURE
                .with_label_values(&[
                    req.method().as_str(),
                    req.uri().path(),
                    &res.status().as_u16().to_string(),
                ])
                .inc();

            #[cfg(feature = "metrics")]
            SERVER_REQUEST_DURATION
                .with_label_values(&[
                    req.method().as_str(),
                    req.uri().path(),
                    &res.status().as_u16().to_string(),
                    "us",
                ])
                .inc_by(duration as f64);

            Ok(res)
        }
    }
}

#[cfg_attr(feature = "trace", tracing::instrument)]
pub async fn healthz(_req: &Request<Body>) -> Result<Response<Body>, Error> {
    let mut res = Response::default();

    *res.status_mut() = StatusCode::NO_CONTENT;

    Ok(res)
}

#[cfg_attr(feature = "trace", tracing::instrument)]
pub async fn not_found(_req: &Request<Body>) -> Result<Response<Body>, Error> {
    let mut res = Response::default();

    *res.status_mut() = StatusCode::NOT_FOUND;

    Ok(res)
}
