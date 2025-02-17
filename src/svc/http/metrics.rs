//! # Prometheus module
//!
//! This module provides a handler to export telemetry using prometheus format

use axum::{
    body::Body,
    http::{header, HeaderValue, Request, Response, StatusCode},
};

use prometheus::{Encoder, TextEncoder};

// -----------------------------------------------------------------------------
// handler

#[tracing::instrument(skip_all)]
pub async fn handler(_req: Request<Body>) -> Response<Body> {
    let mut res = Response::default();
    let headers = res.headers_mut();

    let encoder = TextEncoder::new();
    let metrics = prometheus::gather();

    let mut buf = vec![];
    match encoder.encode(&metrics, &mut buf) {
        Ok(_) => {
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime::TEXT_PLAIN_UTF_8.as_ref())
                    .expect("constant to be iso8859-1 compliant"),
            );

            headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&buf.len().to_string())
                    .expect("content-length to be iso8859-1 compliant"),
            );

            *res.status_mut() = StatusCode::OK;
            *res.body_mut() = Body::from(buf);
        }
        Err(err) => {
            let message = serde_json::json!({"error": err.to_string() }).to_string();

            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime::APPLICATION_JSON.as_ref())
                    .expect("constant to be iso8859-1 compliant"),
            );

            headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&message.len().to_string())
                    .expect("content length to be iso8859-1 compliant"),
            );

            *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            *res.body_mut() = Body::from(message);
        }
    }

    res
}
