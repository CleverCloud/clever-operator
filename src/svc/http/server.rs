//! # Server module
//!
//! This module provide a HTTP server to handle health request

use std::{net::AddrParseError, sync::Arc};

use hyper::{
    service::{make_service_fn, service_fn},
    Server,
};
use tracing::{info, Instrument};

use crate::svc::{cfg::Configuration, telemetry::router};

// -----------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to parse listen address '{0}', {1}")]
    Listen(String, AddrParseError),
    #[error("failed to bind server, {0}")]
    Bind(hyper::Error),
    #[error("failed to serve content, {0}")]
    Serve(hyper::Error),
}

#[tracing::instrument]
pub async fn serve(config: Arc<Configuration>) -> Result<(), Error> {
    let addr = config
        .operator
        .listen
        .parse()
        .map_err(|err| Error::Listen(config.operator.listen.to_owned(), err))?;

    info!("Start to listen for http request on {}", addr);
    Server::try_bind(&addr)
        .map_err(Error::Bind)?
        .serve(make_service_fn(|_| async {
            Ok::<_, Error>(service_fn(router))
        }))
        .instrument(tracing::info_span!("Server::serve"))
        .await
        .map_err(Error::Serve)?;

    Ok(())
}
