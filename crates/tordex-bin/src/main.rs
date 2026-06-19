//! TORdex — main binary.
//!
//! Wires the configuration, observability, backing services, registries and
//! routers into a single Axum app. Run with `--help` for command-line options
//! (none today; everything is driven by environment variables).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use metrics_exporter_prometheus::PrometheusBuilder;
use sqlx::postgres::PgPoolOptions;
use tokio::signal;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use tordex_collection::router::CollectionRouter;
use tordex_collection::{
    CollectionsState, HttpCollector, HttpCollectorConfig, PgCollectionStore,
};
use tordex_core::AppConfig;
use tordex_events::{EventBus, RedisEventBus};
use tordex_sources::{PgSourceRegistry, SourcesState};

#[derive(Clone)]
struct AppState {
    health: Arc<HealthState>,
}

struct HealthState {
    postgres_ok: Arc<std::sync::atomic::AtomicBool>,
    redis_ok: Arc<std::sync::atomic::AtomicBool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // === Config ===
    let config = AppConfig::from_env().context("loading configuration")?;

    // === Tracing ===
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    info!(bind = %config.http_bind, "starting TORdex");

    // === Metrics ===
    let metrics_addr: SocketAddr = "0.0.0.0:9100"
        .parse()
        .expect("static parse never fails");
    PrometheusBuilder::new()
        .with_http_listener(metrics_addr)
        .install()
        .context("installing prometheus exporter")?;

    // === Postgres ===
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .min_connections(config.database.min_connections)
        .acquire_timeout(config.database.acquire_timeout)
        .connect(&config.database.url)
        .await
        .context("connecting to PostgreSQL")?;
    info!("connected to PostgreSQL");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("running migrations")?;
    info!("migrations applied");

    // === Redis ===
    let events_bus: Arc<dyn EventBus> = match RedisEventBus::connect(
        &config.redis.url,
        config.redis.stream_key.clone(),
    )
    .await
    {
        Ok(bus) => {
            info!("connected to Redis");
            Arc::new(bus)
        }
        Err(e) => {
            error!("failed to connect to Redis: {e}; falling back to in-memory bus");
            Arc::new(tordex_events::InMemoryEventBus::default())
        }
    };

    // === S3 / MinIO ===
    // Connect only; the Evidence Lake (Layer 3) will issue the real calls.
    let _minio_client = s3::Client::new(
        &s3::Region::Custom {
            name: config.minio.region.clone(),
            endpoint: config.minio.endpoint.clone(),
        },
        s3::Credentials::new(
            Some(&config.minio.access_key),
            Some(&config.minio.secret_key),
            None,
            None,
            None,
        )
        .context("constructing MinIO credentials")?,
    );
    info!("MinIO client configured");

    // === Qdrant ===
    // Connect only; the Search Engine (Layer 12) will issue the real calls.
    let _qdrant = qdrant_client::Qdrant::from_url(&config.qdrant.url).build();
    info!("Qdrant client configured");

    // === Registries / stores ===
    let sources_registry: Arc<dyn tordex_sources::SourceRegistry> =
        Arc::new(PgSourceRegistry::new(pool.clone()));
    let collections_store: Arc<dyn tordex_collection::CollectionStore> =
        Arc::new(PgCollectionStore::new(pool.clone()));

    // === Collectors / router ===
    let http_collector = HttpCollector::new(HttpCollectorConfig {
        user_agent: config.collection.http.user_agent.clone(),
        timeout: config.collection.http.timeout,
        max_redirects: config.collection.http.max_redirects,
        max_bytes: 32 * 1024 * 1024,
    })
    .context("building HTTP collector")?;
    let collection_router = CollectionRouter::new(
        http_collector,
        events_bus.clone(),
        collections_store.clone(),
        config.collection.rate_limit.per_second,
        config.collection.rate_limit.burst,
    );

    // === Routers ===
    let sources_state = SourcesState::new(sources_registry.clone());
    let sources_router = tordex_sources::router(sources_state);

    let collections_state = CollectionsState::new(
        collection_router,
        sources_registry,
        collections_store,
        events_bus,
    );
    let collections_router = tordex_collection::router(collections_state);

    let health = Arc::new(HealthState {
        postgres_ok: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        redis_ok: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .merge(sources_router)
        .merge(collections_router)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .layer(TimeoutLayer::new(Duration::from_secs(60)))
        .with_state(AppState { health: health.clone() });

    // === Periodic readiness probe ===
    {
        let health = health.clone();
        let pool = pool.clone();
        let redis_client = redis::Client::open(config.redis.url.as_str()).ok();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(5));
            loop {
                ticker.tick().await;
                let pg_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
                    .fetch_one(&pool)
                    .await
                    .is_ok();
                health.postgres_ok.store(pg_ok, std::sync::atomic::Ordering::Relaxed);

                if let Some(client) = &redis_client {
                    let redis_ok = match client.get_multiplexed_async_connection().await {
                        Ok(mut conn) => redis::cmd("PING")
                            .query_async::<String>(&mut conn)
                            .await
                            .map(|s| s == "PONG")
                            .unwrap_or(false),
                        Err(_) => false,
                    };
                    health.redis_ok.store(redis_ok, std::sync::atomic::Ordering::Relaxed);
                }
            }
        });
    }

    // === Serve ===
    let addr: SocketAddr = config
        .http_bind
        .parse()
        .context("invalid TORDEX_HTTP_BIND")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to {addr}"))?;
    info!(%addr, "TORdex listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    info!("TORdex stopped");
    Ok(())
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let pg = state.health.postgres_ok.load(std::sync::atomic::Ordering::Relaxed);
    let rd = state.health.redis_ok.load(std::sync::atomic::Ordering::Relaxed);
    if pg && rd {
        (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ready", "postgres": pg, "redis": rd })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "status": "not_ready", "postgres": pg, "redis": rd })),
        )
    }
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