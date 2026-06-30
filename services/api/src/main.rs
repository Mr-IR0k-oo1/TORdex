use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use tokio::signal;
use tracing::info;

use tordex_api::build_app;
use tordex_api::AppState;
use tordex_config::AppConfig;
use tordex_core::processor::InMemoryProcessorRegistry;
use tordex_evidence::MinioArtifactStore;
use tordex_types::ArtifactStore;

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::from_env().context("loading configuration")?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    info!(bind = %config.http_bind, "starting TORdex Phase 1");

    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .min_connections(config.database.min_connections)
        .acquire_timeout(config.database.acquire_timeout)
        .connect(&config.database.url)
        .await
        .context("connecting to PostgreSQL")?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("running migrations")?;
    info!("migrations applied");

    let store: Arc<dyn ArtifactStore> = Arc::new(
        MinioArtifactStore::connect(
            &config.minio.endpoint,
            &config.minio.region,
            &config.minio.access_key,
            &config.minio.secret_key,
            &config.minio.bucket,
        )
        .await
        .context("connecting to MinIO")?,
    );

    let registry = Arc::new(InMemoryProcessorRegistry::new());
    tordex_processors::register_all(&*registry);

    let state = AppState {
        pool,
        store,
        registry,
    };
    let app = build_app(state);

    let addr: SocketAddr = config
        .http_bind
        .parse()
        .context("invalid TORDEX_HTTP_BIND")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to {addr}"))?;
    info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    info!("shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("install ctrl_c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("ctrl_c received, shutting down"),
        _ = terminate => info!("SIGTERM received, shutting down"),
    }
}
