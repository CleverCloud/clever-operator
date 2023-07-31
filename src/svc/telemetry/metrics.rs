//! # Metrics module
//!
//! This module expose metrics integrations, structures and helpers

use hyper::{
    header::{self, HeaderValue, InvalidHeaderValue},
    Body, Request, Response, StatusCode,
};
use prometheus::{gather, Encoder, TextEncoder};

// -----------------------------------------------------------------------------
// Error enum

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to serialize metrics, {0}")]
    PrometheusSerialize(prometheus::Error),
    #[error("failed to parse header value given by prometheus, {0}")]
    PrometheusInvalidHeader(InvalidHeaderValue),
}

// -----------------------------------------------------------------------------
// Helper methods

#[cfg_attr(feature = "trace", tracing::instrument)]
/// returns in the [`Response`] object the encoded metrics gathered from the
/// application
pub async fn handler(_req: &Request<Body>) -> Result<Response<Body>, Error> {
    // -------------------------------------------------------------------------
    // Step 1: gather and encode metrics

    let families = gather();
    let encoder = TextEncoder;
    let mut buf = vec![];
    encoder
        .encode(&families, &mut buf)
        .map_err(Error::PrometheusSerialize)?;

    // -------------------------------------------------------------------------
    // Step 2: awnser with encoded metrics

    let mut res = Response::default();

    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(encoder.format_type()).map_err(Error::PrometheusInvalidHeader)?,
    );

    *res.status_mut() = StatusCode::OK;
    *res.body_mut() = Body::from(buf);

    Ok(res)
}
