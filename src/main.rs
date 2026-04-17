mod auth;
mod config;
mod db;
mod encoding;
mod refresh;
mod server;

use std::sync::Arc;

use axum::{Router, middleware};
use clap::Parser;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    auth::{BearerToken, require_bearer},
    config::{Config, Mode},
    db::Database,
    server::RocksDbServer,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::parse();
    cfg.validate()?;

    init_tracing();

    tracing::info!(
        db_path = %cfg.db_path.display(),
        mode = ?cfg.mode,
        bind = %cfg.bind_addr(),
        auth = cfg.api_token.is_some(),
        "opening rocksdb-mcp",
    );

    let database = Arc::new(Database::open(&cfg)?);
    let shutdown = CancellationToken::new();

    if cfg.mode == Mode::Secondary {
        refresh::spawn(
            database.handle(),
            cfg.refresh_interval,
            shutdown.child_token(),
        );
    }

    let handler_factory = {
        let database = Arc::clone(&database);
        move || Ok(RocksDbServer::new(Arc::clone(&database)))
    };

    let service = StreamableHttpService::new(
        handler_factory,
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default().with_cancellation_token(shutdown.child_token()),
    );

    let mut app: Router<()> = Router::new().nest_service("/mcp", service);
    if let Some(token) = cfg.api_token.as_deref() {
        let state = BearerToken::new(token)?;
        app = app.layer(middleware::from_fn_with_state(state, require_bearer));
    }

    let listener = tokio::net::TcpListener::bind(cfg.bind_addr()).await?;
    let local = listener.local_addr()?;
    tracing::info!(%local, "listening on http://{local}/mcp");

    let shutdown_signal = shutdown.clone();
    tokio::spawn(async move {
        let reason = wait_for_shutdown_signal().await;
        tracing::info!(%reason, "shutdown signal received");
        shutdown_signal.cancel();
    });

    let serve = axum::serve(listener, app).with_graceful_shutdown({
        let shutdown = shutdown.clone();
        async move { shutdown.cancelled().await }
    });
    serve.await?;
    Ok(())
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() -> &'static str {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to install SIGTERM handler; relying on SIGINT only");
            tokio::signal::ctrl_c().await.ok();
            return "SIGINT";
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => "SIGINT",
        _ = sigterm.recv() => "SIGTERM",
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() -> &'static str {
    tokio::signal::ctrl_c().await.ok();
    "ctrl-c"
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}
