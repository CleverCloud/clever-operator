//! # HTTP module
//!
//! This module provides utilities to interact using HTTP protocol

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};

pub mod layer;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod server;

// -----------------------------------------------------------------------------
// Not found

#[tracing::instrument(skip_all)]
pub async fn not_found(_req: Request<Body>) -> Response<Body> {
    let mut res = Response::default();

    *res.status_mut() = StatusCode::NOT_FOUND;
    res
}

// -----------------------------------------------------------------------------
// Healthz

#[tracing::instrument(skip_all)]
pub async fn healthz(_req: Request<Body>) -> Response<Body> {
    let mut res = Response::default();

    let message = serde_json::json!({"messaging": "Everything is fine! 🚀"}).to_string();

    *res.status_mut() = StatusCode::OK;
    *res.body_mut() = Body::from(message);

    res
}
