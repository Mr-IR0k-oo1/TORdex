use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use tokio::signal;
use tracing::info;

mod config;
mod persistent;
mod runtime;
mod api;

use config::KernelConfig;
use persistent::{
    PgEventStore, PgObjectManager, PgSnapshotStore,
    StorageManagerWrapper,
};
use runtime::KernelRuntime;

use tordex_cluster::{ClusterMembership, ClusterScheduler, TaskQueue};

#[tokio::main]
async fn main() -> Result<()> {
    let config = KernelConfig::from_env().context("loading config")?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    info!(
        profile = %config.app_profile.name,
        bind = %config.http_bind,
        "starting TORdex Intelligence Kernel"
    );

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

    let (kernel, event_store) = build_kernel(&config, pool.clone()).await?;
    let kernel = Arc::new(kernel);

    let cluster_scheduler: Option<Arc<ClusterScheduler>> = match config.redis_url.as_deref() {
        None => {
            tracing::warn!("no REDIS_URL set, running without cluster scheduler");
            None
        }
        Some(redis_url) => match redis::Client::open(redis_url) {
            Err(e) => {
                tracing::warn!(error = %e, "Redis client creation failed, running without cluster scheduler");
                None
            }
            Ok(client) => match client.get_connection_manager().await {
                Err(e) => {
                    tracing::warn!(error = %e, "Redis unavailable, running without cluster scheduler");
                    None
                }
                Ok(manager) => {
                    let membership = ClusterMembership::new(manager.clone());
                    let task_queue = TaskQueue::new(manager);
                    let scheduler = ClusterScheduler::new(task_queue, membership);
                    info!("cluster scheduler initialised with Redis");
                    let scheduler_arc = Arc::new(scheduler);
                    let scheduler_clone = scheduler_arc.clone();
                    let tick_ms = config.tick_interval_ms;
                    tokio::spawn(async move {
                        let mut interval = tokio::time::interval(std::time::Duration::from_millis(tick_ms));
                        loop {
                            interval.tick().await;
                            if let Ok(results) = scheduler_clone.collect_pending_results(50).await {
                                for r in results {
                                    tracing::debug!(
                                        task_id = %r.task_id,
                                        success = r.success,
                                        worker = %r.worker_id,
                                        "cluster task result collected"
                                    );
                                }
                            }
                        }
                    });
                    Some(scheduler_arc)
                }
            },
        },
    };

    let runtime = KernelRuntime::new(
        kernel.clone(),
        event_store,
        config.tick_interval_ms,
    );
    let runtime_handle = tokio::spawn(async move {
        runtime.run().await;
    });

    let app = api::build_router(kernel.clone(), cluster_scheduler);
    let addr: SocketAddr = config
        .http_bind
        .parse()
        .context("invalid TORDEX_HTTP_BIND")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to {addr}"))?;
    info!(%addr, "kernel API listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    runtime_handle.abort();
    info!("kernel shutdown complete");
    Ok(())
}

async fn build_kernel(
    config: &KernelConfig,
    pool: sqlx::PgPool,
) -> Result<(tordex_core::Kernel, Box<dyn tordex_core::event_store::EventStore>)> {
    let mut kernel = tordex_core::Kernel::new();

    let pg_event_store = PgEventStore::new(pool.clone());
    let pg_snapshot_store = PgSnapshotStore::new(pool.clone());
    let pg_object_manager = PgObjectManager::new(pool.clone());
    let storage_manager = if let Some(minio) = &config.minio {
        let store = tordex_evidence::MinioArtifactStore::connect(
            &minio.endpoint,
            &minio.region,
            &minio.access_key,
            &minio.secret_key,
            &minio.bucket,
        )
        .await
        .context("connecting to MinIO")?;
        Box::new(StorageManagerWrapper::new(Arc::new(store)))
            as Box<dyn tordex_core::StorageManager>
    } else {
        Box::new(StorageManagerWrapper::new_file_based(
            &config.storage_dir,
        ))
    };

    kernel.event_store = Box::new(pg_event_store.clone());
    kernel.snapshots = Box::new(pg_snapshot_store);
    kernel.objects = Box::new(pg_object_manager);
    kernel.storage = storage_manager;

    for &name in config.app_profile.agents {
        match name {
            "research" => {
                kernel
                    .agents
                    .register(Box::new(tordex_agents::ResearchAgent::new()))
                    .ok();
            }
            "architecture" => {
                kernel
                    .agents
                    .register(Box::new(tordex_agents::ArchitectureAgent::new()))
                    .ok();
            }
            "malware" => {
                kernel
                    .agents
                    .register(Box::new(tordex_agents::MalwareAgent::new()))
                    .ok();
            }
            "monitoring" => {
                kernel
                    .agents
                    .register(Box::new(tordex_agents::MonitoringAgent::new()))
                    .ok();
            }
            "documentation" => {
                kernel
                    .agents
                    .register(Box::new(tordex_agents::DocumentationAgent::new()))
                    .ok();
            }
            "forensics" => {
                kernel
                    .agents
                    .register(Box::new(tordex_agents::ForensicsAgent::new()))
                    .ok();
            }
            other => {
                tracing::warn!(agent = other, "unknown agent in profile");
            }
        }
    }

    if config.app_profile.drivers.contains(&"http") || config.app_profile.drivers.contains(&"all") {
        kernel
            .drivers
            .register(Box::new(tordex_drivers::http::HttpDriver::new()))
            .ok();
    }
    if config.app_profile.drivers.contains(&"dns") || config.app_profile.drivers.contains(&"all") {
        kernel
            .drivers
            .register(Box::new(tordex_drivers::dns::DnsDriver::new()))
            .ok();
    }
    if config.app_profile.drivers.contains(&"filesystem")
        || config.app_profile.drivers.contains(&"all")
    {
        kernel
            .drivers
            .register(Box::new(tordex_drivers::filesystem::FilesystemDriver::new()))
            .ok();
    }

    if !config.app_profile.watchers.is_empty() {
        let engine = Arc::new(tordex_monitoring::MonitoringEngine::new());
        for &w in config.app_profile.watchers {
            match w {
                "repository" => {
                    engine.register(Box::new(
                        tordex_monitoring::watchers::RepositoryWatcher::new(),
                    ));
                }
                "onion" => {
                    engine.register(Box::new(
                        tordex_monitoring::watchers::OnionWatcher::new(),
                    ));
                }
                "api" => {
                    engine.register(Box::new(
                        tordex_monitoring::watchers::ApiWatcher::new(),
                    ));
                }
                "organization" => {
                    engine.register(Box::new(
                        tordex_monitoring::watchers::OrganizationWatcher::new(),
                    ));
                }
                "package" => {
                    engine.register(Box::new(
                        tordex_monitoring::watchers::PackageWatcher::new(),
                    ));
                }
                "cve" => {
                    engine.register(Box::new(
                        tordex_monitoring::watchers::CveWatcher::new(),
                    ));
                }
                "threat" => {
                    engine.register(Box::new(
                        tordex_monitoring::watchers::ThreatWatcher::new(),
                    ));
                }
                other => {
                    tracing::warn!(watcher = other, "unknown watcher in profile");
                }
            }
        }
        let monitor_agent = tordex_monitoring::MonitoringAgent::new(engine);
        kernel.agents.register(Box::new(monitor_agent)).ok();
    }

    let processor_registry =
        std::sync::Arc::new(tordex_core::processor::InMemoryProcessorRegistry::new());
    tordex_processors::register_all(&*processor_registry);

    info!(
        agents = kernel.agents.list().len(),
        drivers = kernel.drivers.list().len(),
        "kernel built"
    );
    Ok((kernel, Box::new(pg_event_store)))
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
