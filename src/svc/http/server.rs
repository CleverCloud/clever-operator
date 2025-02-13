//! # Server module
//!
//! This module provides a server implementation with a router based on the
//! crate [`axum`].

use std::net::SocketAddr;

use axum::routing::{any, get};
use axum::{middleware, Router};
use tokio::net::TcpListener;
use tracing::info;

#[cfg(feature = "metrics")]
use crate::svc::http::metrics;
use crate::svc::http::{healthz, layer, not_found};

// -----------------------------------------------------------------------------
// Error

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to bind on socket '{0}', {1}")]
    Bind(SocketAddr, std::io::Error),
    #[error("failed to listen on socket '{0}', {1}")]
    Serve(SocketAddr, std::io::Error),
}

// -----------------------------------------------------------------------------
// router

#[cfg(feature = "metrics")]
#[tracing::instrument]
pub fn router() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/livez", get(healthz))
        .route("/readyz", get(healthz))
        .route("/status", get(healthz))
        .route("/metrics", get(metrics::handler))
        .fallback(any(not_found))
        .layer(middleware::from_fn(layer::access))
}

#[cfg(not(feature = "metrics"))]
#[tracing::instrument]
pub fn router() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/livez", get(healthz))
        .route("/readyz", get(healthz))
        .route("/status", get(healthz))
        .fallback(any(not_found))
        .layer(middleware::from_fn(layer::access))
}

// -----------------------------------------------------------------------------
// helpers

#[tracing::instrument(skip(router))]
pub async fn serve(router: Router, addr: SocketAddr) -> Result<(), Error> {
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|err| Error::Bind(addr.to_owned(), err))?;

    info!(addr = addr.to_string(), "Begin to listen on address");
    axum::serve(listener, router.into_make_service())
        .await
        .map_err(|err| Error::Serve(addr, err))?;

    Ok(())
}
