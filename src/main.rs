//! # Clever operator
//!
//! A kubernetes operator that expose clever cloud's resources through custom
//! resource definition
use std::{convert::TryFrom, error::Error, sync::Arc};

#[cfg(feature = "trace")]
use opentelemetry::global;
#[cfg(feature = "trace")]
use opentelemetry_jaeger::Propagator;
use slog::{o, Drain, Level, LevelFilter, Logger};
use slog_async::Async;
use slog_scope::{crit, debug, info, set_global_logger, GlobalLoggerGuard as Guard};
use slog_term::{FullFormat, TermDecorator};
#[cfg(feature = "trace")]
use tracing_subscriber::{layer::SubscriberExt, Registry};

use crate::{
    cmd::{daemon, Args, Executor},
    svc::cfg::Configuration,
};

pub mod cmd;
pub mod svc;

pub(crate) fn initialize(verbosity: &usize) -> Guard {
    let level = Level::from_usize(Level::Critical.as_usize() + verbosity).unwrap_or(Level::Trace);

    let decorator = TermDecorator::new().build();
    let drain = FullFormat::new(decorator).build().fuse();
    let drain = LevelFilter::new(drain, level).fuse();
    let drain = Async::new(drain).build().fuse();

    set_global_logger(Logger::root(drain, o!()))
}

#[paw::main]
#[tokio::main]
pub(crate) async fn main(args: Args) -> Result<(), Box<dyn Error + Send + Sync>> {
    let _guard = initialize(&args.verbosity);

    #[cfg(feature = "logging")]
    if let Err(err) = slog_stdlog::init() {
        crit!("Could not initialize standard logger"; "error" => err.to_string());
        return Err(err.into());
    }

    let config = Arc::new(match &args.config {
        Some(path) => Configuration::try_from(path.to_owned())?,
        None => Configuration::try_default()?,
    });

    if args.check {
        debug!("{:#?}", config);
        info!("{} configuration is healthy!", env!("CARGO_PKG_NAME"));
        return Ok(());
    }

    #[cfg(feature = "tracker")]
    let _sguard = config.sentry.dsn.as_ref().map(|dsn| {
        sentry::init((
            dsn.to_owned(),
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ))
    });

    #[cfg(feature = "trace")]
    if let Some(jaeger) = &config.jaeger {
        info!("Start to trace using jaeger with opentelemetry compatibility"; "endpoint" => jaeger.endpoint.to_owned());
        global::set_text_map_propagator(Propagator::new());

        let mut builder = opentelemetry_jaeger::new_pipeline()
            .with_collector_endpoint(jaeger.endpoint.to_owned())
            .with_service_name(env!("CARGO_PKG_NAME"));

        if let Some(user) = &jaeger.user {
            builder = builder.with_collector_username(user);
        }

        if let Some(password) = &jaeger.password {
            builder = builder.with_collector_password(password);
        }

        let layer = tracing_opentelemetry::layer().with_tracer(builder.install_simple()?);

        tracing::subscriber::set_global_default(Registry::default().with(layer))?;
    }

    let result: Result<_, Box<dyn Error + Send + Sync>> = match &args.command {
        Some(cmd) => cmd.execute(config).await.map_err(Into::into),
        None => daemon(args.kubeconfig, config).await.map_err(Into::into),
    };

    if let Err(err) = result {
        crit!("could not execute {} properly", env!("CARGO_PKG_NAME"); "error" => err.to_string());
        return Err(err);
    }

    #[cfg(feature = "trace")]
    global::shutdown_tracer_provider();

    info!("{} halted!", env!("CARGO_PKG_NAME"));
    Ok(())
}
