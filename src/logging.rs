//! # Logging module
//!
//! This module provides logging facilities and helpers

use tracing::Level;
use tracing_subscriber::{filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::svc::cfg::Configuration;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to set initialize registry globally, {0}")]
    InitializeRegistry(tracing_subscriber::util::TryInitError),
    #[error("failed to create a jaeger tracer, {0}")]
    CreateJaegerTracer(tracer::Error),
}

// -----------------------------------------------------------------------------
// Layer

pub mod tracer {
    #[cfg(feature = "trace")]
    use opentelemetry::{
        sdk::trace::{self, RandomIdGenerator, Sampler, Tracer},
        trace::TraceError,
    };

    #[cfg(feature = "trace")]
    use crate::svc::cfg::Configuration;

    // -------------------------------------------------------------------------
    // Error

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[cfg(feature = "trace")]
        #[error("failed to configure jaeger collector (agent), {0}")]
        ConfigureJaeger(TraceError),
    }

    // -------------------------------------------------------------------------
    // helpers

    #[cfg(feature = "trace")]
    pub fn jaeger(config: &Configuration) -> Result<Tracer, Error> {
        let mut builder = opentelemetry_jaeger::new_collector_pipeline()
            .with_endpoint(config.jaeger.endpoint.to_string())
            .with_service_name(env!("CARGO_PKG_NAME"))
            .with_instrumentation_library_tags(true)
            .with_timeout(std::time::Duration::from_secs(600))
            .with_reqwest()
            .with_trace_config(
                trace::config()
                    .with_sampler(Sampler::AlwaysOn)
                    .with_id_generator(RandomIdGenerator::default())
                    .with_max_events_per_span(64)
                    .with_max_attributes_per_span(16)
                    .with_max_events_per_span(16),
            );

        if let Some(user) = &config.jaeger.user {
            builder = builder.with_username(user);
        }

        if let Some(password) = &config.jaeger.password {
            builder = builder.with_password(password);
        }

        builder
            .install_batch(opentelemetry::runtime::Tokio)
            .map_err(Error::ConfigureJaeger)
    }
}

// -----------------------------------------------------------------------------
// helpers

pub const fn level(verbosity: usize) -> Level {
    match verbosity {
        0 => Level::ERROR,
        1 => Level::WARN,
        2 => Level::INFO,
        3 => Level::DEBUG,
        _ => Level::TRACE,
    }
}

#[cfg(all(not(feature = "trace"), not(feature = "tracker")))]
pub fn initialize(_config: &Configuration, verbosity: usize) -> Result<(), Error> {
    let filter = LevelFilter::from_level(level(verbosity));
    let registry = tracing_subscriber::registry().with(filter).with(
        fmt::Layer::new()
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_line_number(true)
            .with_target(true),
    );

    registry.try_init().map_err(Error::InitializeRegistry)
}

#[cfg(all(feature = "tracker", not(feature = "trace")))]
pub fn initialize(_config: &Configuration, verbosity: usize) -> Result<(), Error> {
    let filter = LevelFilter::from_level(level(verbosity));

    tracing_subscriber::registry()
        .with(filter)
        .with(sentry_tracing::layer())
        .with(
            fmt::Layer::new()
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_line_number(true)
                .with_target(true),
        )
        .try_init()
        .map_err(Error::InitializeRegistry)
}

#[cfg(all(feature = "trace", not(feature = "tracker")))]
pub fn initialize(config: &Configuration, verbosity: usize) -> Result<(), Error> {
    let filter = LevelFilter::from_level(level(verbosity));
    let registry = tracing_subscriber::registry().with(filter).with(
        fmt::Layer::new()
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_line_number(true)
            .with_target(true),
    );

    if !config.jaeger.endpoint.is_empty() {
        tracing::debug!(
            endpoint = &config.jaeger.endpoint,
            "Configure jaeger integration for tracing crate"
        );

        opentelemetry::global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());

        let layer = tracing_opentelemetry::layer()
            .with_tracer(tracer::jaeger(config).map_err(Error::CreateJaegerTracer)?)
            .with_location(true)
            .with_threads(true)
            .with_tracked_inactivity(true)
            .with_exception_fields(true)
            .with_exception_field_propagation(true);

        registry.with(layer).try_init()
    } else {
        registry.try_init()
    }
    .map_err(Error::InitializeRegistry)
}

#[cfg(all(feature = "trace", feature = "tracker"))]
pub fn initialize(config: &Configuration, verbosity: usize) -> Result<(), Error> {
    let filter = LevelFilter::from_level(level(verbosity));
    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(sentry_tracing::layer())
        .with(
            fmt::Layer::new()
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_line_number(true)
                .with_target(true),
        );

    if !config.jaeger.endpoint.is_empty() {
        tracing::debug!(
            endpoint = &config.jaeger.endpoint,
            "Configure jaeger integration for tracing crate"
        );

        opentelemetry::global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());

        let layer = tracing_opentelemetry::layer()
            .with_tracer(tracer::jaeger(config).map_err(Error::CreateJaegerTracer)?)
            .with_location(true)
            .with_threads(true)
            .with_tracked_inactivity(true)
            .with_exception_fields(true)
            .with_exception_field_propagation(true);

        registry.with(layer).try_init()
    } else {
        registry.try_init()
    }
    .map_err(Error::InitializeRegistry)
}
