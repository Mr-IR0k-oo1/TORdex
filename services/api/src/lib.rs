use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use sha2::Digest;
use tower_http::trace::TraceLayer;
use tracing::info;

use tordex_core::processor::ProcessorRegistry;
use tordex_types::ArtifactStore;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub store: Arc<dyn ArtifactStore>,
    pub registry: Arc<dyn ProcessorRegistry>,
}

#[derive(Deserialize)]
struct CollectRequest {
    url: String,
}

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/collect", post(collect_handler))
        .route("/services", get(services_handler))
        .route("/artifacts", get(artifacts_handler))
        .route("/events", get(events_handler))
        .route("/health", get(health_handler))
        .route("/processors", get(processors_handler))
        .route("/process", post(process_handler))
        .route("/intel/analyze", post(intel_analyze_handler))
        .route("/intel/report", get(intel_report_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ─── POST /collect ───────────────────────────────────────────────────────────

async fn collect_handler(
    State(state): State<AppState>,
    Json(req): Json<CollectRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let now = time::OffsetDateTime::now_utc();
    let session_id = ulid::Ulid::new().to_string();
    let artifact_id = ulid::Ulid::new().to_string();
    let event_id = ulid::Ulid::new().to_string();

    // 1. Create or find service
    let service_id: String = sqlx::query_scalar(
        "INSERT INTO services (id, display_name, locator, kind)
         VALUES ($1, $2, $3, 'website')
         ON CONFLICT (locator) DO UPDATE SET locator = EXCLUDED.locator
         RETURNING id",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(&req.url)
    .bind(&req.url)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(format!("failed to create service: {e}")))?;

    // 2. Create session
    sqlx::query(
        "INSERT INTO collection_sessions (id, service_id, status, started_at)
         VALUES ($1, $2, 'running', $3)",
    )
    .bind(&session_id)
    .bind(&service_id)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(format!("failed to create session: {e}")))?;

    // 3. Download resource
    let download = match download_url(&req.url).await {
        Ok(d) => d,
        Err(e) => {
            fail_session(&state.pool, &session_id, &e.to_string());
            return Err(e);
        }
    };

    // 4. Compute SHA256
    let hash = sha2::Sha256::digest(&download.body);
    let hash_hex = hex::encode(hash);

    // 5. Store in MinIO
    let storage_path = format!("sessions/{session_id}/{hash_hex}");
    if let Err(e) = state
        .store
        .put(&storage_path, &download.body, download.content_type.as_deref())
        .await
    {
        fail_session(&state.pool, &session_id, &e.to_string());
        return Err(ApiError::Internal(format!("failed to store artifact: {e}")));
    }

    // 6. Save artifact
    let result = sqlx::query(
        "INSERT INTO artifacts (id, session_id, kind, content_type, byte_count, sha256, storage_path)
         VALUES ($1, $2, 'html', $3, $4, $5, $6)",
    )
    .bind(&artifact_id)
    .bind(&session_id)
    .bind(&download.content_type)
    .bind(download.body.len() as i64)
    .bind(&hash_hex)
    .bind(&storage_path)
    .execute(&state.pool)
    .await;
    if let Err(e) = result {
        fail_session(&state.pool, &session_id, &e.to_string());
        return Err(ApiError::Internal(format!("failed to save artifact: {e}")));
    }

    // 7. Save event
    sqlx::query(
        "INSERT INTO events (id, session_id, topic, payload)
         VALUES ($1, $2, 'collection.completed', $3)",
    )
    .bind(&event_id)
    .bind(&session_id)
    .bind(serde_json::json!({
        "url": req.url,
        "sha256": hash_hex,
        "byte_count": download.body.len(),
        "content_type": download.content_type,
        "final_url": download.final_url,
    }))
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(format!("failed to save event: {e}")))?;

    // 8. Mark session completed
    sqlx::query(
        "UPDATE collection_sessions SET status = 'completed', completed_at = $1 WHERE id = $2",
    )
    .bind(time::OffsetDateTime::now_utc())
    .bind(&session_id)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(format!("failed to update session: {e}")))?;

    info!(
        session = %session_id,
        url = %req.url,
        bytes = download.body.len(),
        sha256 = %hash_hex,
        "collection completed"
    );

    Ok(Json(serde_json::json!({
        "session_id": session_id,
        "service_id": service_id,
        "artifact_id": artifact_id,
        "sha256": hash_hex,
        "byte_count": download.body.len(),
    })))
}

struct DownloadedResource {
    body: Vec<u8>,
    content_type: Option<String>,
    final_url: String,
}

async fn download_url(url: &str) -> Result<DownloadedResource, ApiError> {
    let resp = reqwest::get(url)
        .await
        .map_err(|e| ApiError::Collect(format!("request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(ApiError::Collect(format!(
            "HTTP {} from {}",
            status.as_u16(),
            url
        )));
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let final_url = resp.url().to_string();
    let body = resp
        .bytes()
        .await
        .map_err(|e| ApiError::Collect(format!("failed to read body: {e}")))?
        .to_vec();

    Ok(DownloadedResource {
        body,
        content_type,
        final_url,
    })
}

fn fail_session(pool: &sqlx::PgPool, session_id: &str, error: &str) {
    let pool = pool.clone();
    let sid = session_id.to_string();
    let msg = error.to_string();
    tokio::spawn(async move {
        let _ = sqlx::query(
            "UPDATE collection_sessions SET status = 'failed', completed_at = $1, error_message = $2 WHERE id = $3",
        )
        .bind(time::OffsetDateTime::now_utc())
        .bind(&msg)
        .bind(&sid)
        .execute(&pool)
        .await;
    });
}

// ─── GET /services ───────────────────────────────────────────────────────────

async fn services_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, String, String, time::OffsetDateTime)>(
        "SELECT id, display_name, locator, kind, created_at FROM services ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(format!("query failed: {e}")))?
    .into_iter()
    .map(|(id, name, locator, kind, created_at)| {
        serde_json::json!({
            "id": id,
            "display_name": name,
            "locator": locator,
            "kind": kind,
            "created_at": created_at,
        })
    })
    .collect::<Vec<_>>();

    Ok(Json(serde_json::json!({ "services": rows })))
}

// ─── GET /artifacts ──────────────────────────────────────────────────────────

async fn artifacts_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, i64, String, time::OffsetDateTime)>(
        "SELECT id, session_id, kind, content_type, byte_count, sha256, created_at
         FROM artifacts ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(format!("query failed: {e}")))?
    .into_iter()
    .map(|(id, session_id, kind, content_type, byte_count, sha256, created_at)| {
        serde_json::json!({
            "id": id,
            "session_id": session_id,
            "kind": kind,
            "content_type": content_type,
            "byte_count": byte_count,
            "sha256": sha256,
            "created_at": created_at,
        })
    })
    .collect::<Vec<_>>();

    Ok(Json(serde_json::json!({ "artifacts": rows })))
}

// ─── GET /events ─────────────────────────────────────────────────────────────

async fn events_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, (String, Option<String>, String, serde_json::Value, time::OffsetDateTime)>(
        "SELECT id, session_id, topic, payload, occurred_at
         FROM events ORDER BY occurred_at DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(format!("query failed: {e}")))?
    .into_iter()
    .map(|(id, session_id, topic, payload, occurred_at)| {
        serde_json::json!({
            "id": id,
            "session_id": session_id,
            "topic": topic,
            "payload": payload,
            "occurred_at": occurred_at,
        })
    })
    .collect::<Vec<_>>();

    Ok(Json(serde_json::json!({ "events": rows })))
}

// ─── GET /health ─────────────────────────────────────────────────────────────

async fn health_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "phase": 1 })),
    )
}

// ─── GET /processors ─────────────────────────────────────────────────────────

async fn processors_handler(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let names = state.registry.list();
    let mut processors = Vec::new();
    for name in &names {
        if let Some(p) = state.registry.get(name) {
            processors.push(serde_json::json!({
                "name": p.name(),
                "description": p.description(),
                "content_types": p.content_types(),
            }));
        }
    }
    Json(serde_json::json!({ "processors": processors }))
}

// ─── POST /process ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ProcessRequest {
    content_type: Option<String>,
    data: serde_json::Value,
    metadata: Option<HashMap<String, String>>,
    processor: Option<String>,
}

async fn process_handler(
    State(state): State<AppState>,
    Json(req): Json<ProcessRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = ulid::Ulid::new().to_string();
    let data_bytes = serde_json::to_vec(&req.data)
        .map_err(|e| ApiError::Internal(format!("serialization error: {e}")))?;
    let metadata = req.metadata.unwrap_or_default();

    let results = if let Some(processor_name) = &req.processor {
        // Route to specific processor
        let p = state
            .registry
            .get(processor_name)
            .ok_or_else(|| ApiError::Internal(format!("processor not found: {processor_name}")))?;
        p.process(&id, &data_bytes, req.content_type.as_deref(), metadata)
            .map_err(|e| ApiError::Internal(format!("processing error: {e}")))?
    } else {
        // Route by content type
        state
            .registry
            .process(&id, &data_bytes, req.content_type.as_deref(), metadata)
    };

    let observations: Vec<serde_json::Value> = results
        .into_iter()
        .map(|obs| {
            let data: serde_json::Value =
                serde_json::from_slice(&obs.data).unwrap_or(serde_json::Value::Null);
            serde_json::json!({
                "id": obs.id,
                "kind": obs.kind,
                "data": data,
                "content_type": obs.content_type,
                "metadata": obs.metadata,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "observations": observations })))
}

// ─── POST /intel/analyze ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct IntelAnalyzeRequest {
    path: String,
    content: String,
    action: Option<String>,
}

async fn intel_analyze_handler(
    State(state): State<AppState>,
    Json(req): Json<IntelAnalyzeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = ulid::Ulid::new().to_string();
    let data = serde_json::json!({
        "path": req.path,
        "content": req.content,
    });
    let data_bytes = serde_json::to_vec(&data)
        .map_err(|e| ApiError::Internal(format!("serialization error: {e}")))?;
    let mut metadata = HashMap::new();
    metadata.insert(
        "action".to_string(),
        req.action.unwrap_or_else(|| "analyze_file".to_string()),
    );

    let p = state
        .registry
        .get("RepoIntelProcessor")
        .ok_or_else(|| ApiError::Internal("RepoIntelProcessor not registered".to_string()))?;
    let results = p
        .process(&id, &data_bytes, Some("application/x-repo-intel"), metadata)
        .map_err(|e| ApiError::Internal(format!("processing error: {e}")))?;

    let observations: Vec<serde_json::Value> = results
        .into_iter()
        .map(|obs| {
            let data: serde_json::Value =
                serde_json::from_slice(&obs.data).unwrap_or(serde_json::Value::Null);
            serde_json::json!({
                "id": obs.id,
                "kind": obs.kind,
                "data": data,
                "content_type": obs.content_type,
                "metadata": obs.metadata,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "observations": observations })))
}

// ─── GET /intel/report ───────────────────────────────────────────────────────

async fn intel_report_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = ulid::Ulid::new().to_string();
    let p = state
        .registry
        .get("RepoIntelProcessor")
        .ok_or_else(|| ApiError::Internal("RepoIntelProcessor not registered".to_string()))?;
    let results = p
        .process(
            &id,
            b"{}",
            Some("application/x-repo-intel"),
            HashMap::from([("action".into(), "report".into())]),
        )
        .map_err(|e| ApiError::Internal(format!("processing error: {e}")))?;

    let observations: Vec<serde_json::Value> = results
        .into_iter()
        .map(|obs| {
            let data: serde_json::Value =
                serde_json::from_slice(&obs.data).unwrap_or(serde_json::Value::Null);
            serde_json::json!({
                "id": obs.id,
                "kind": obs.kind,
                "data": data,
                "content_type": obs.content_type,
                "metadata": obs.metadata,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "observations": observations })))
}

// ─── Errors ──────────────────────────────────────────────────────────────────

enum ApiError {
    Collect(String),
    Internal(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Collect(msg) => write!(f, "collect error: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Self::Collect(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use std::time::Duration;
    use tower::ServiceExt;

    fn db_url() -> String {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://tordex:tordex@localhost:5432/tordex".to_string())
    }

    fn minio_config() -> (String, String, String, String, String) {
        (
            std::env::var("MINIO_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".into()),
            std::env::var("MINIO_REGION").unwrap_or_else(|_| "us-east-1".into()),
            std::env::var("MINIO_ACCESS_KEY").unwrap_or_else(|_| "tordex".into()),
            std::env::var("MINIO_SECRET_KEY").unwrap_or_else(|_| "tordex-secret".into()),
            std::env::var("MINIO_BUCKET").unwrap_or_else(|_| "tordex-evidence".into()),
        )
    }

    async fn try_pool() -> Option<sqlx::PgPool> {
        tokio::time::timeout(Duration::from_secs(3), sqlx::PgPool::connect(&db_url()))
            .await
            .ok()?
            .ok()
    }

    async fn try_store() -> Option<Arc<dyn ArtifactStore>> {
        let (endpoint, region, key, secret, bucket) = minio_config();
        tokio::time::timeout(
            Duration::from_secs(3),
            tordex_evidence::MinioArtifactStore::connect(
                &endpoint, &region, &key, &secret, &bucket,
            ),
        )
        .await
        .ok()?
        .ok()
        .map(|s| Arc::new(s) as Arc<dyn ArtifactStore>)
    }

    #[tokio::test]
    async fn test_health() {
        let pool = try_pool()
            .await
            .unwrap_or_else(|| panic!("PostgreSQL not available at {}", db_url()));
        let store = try_store()
            .await
            .unwrap_or_else(|| panic!("MinIO not available at {}", minio_config().0));
        let app = build_app(AppState {
            pool,
            store,
            registry: Arc::new(InMemoryProcessorRegistry::new()),
        });

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();

        assert_eq!(body["status"], "ok");
        assert_eq!(body["phase"], 1);
    }

    #[tokio::test]
    async fn test_list_services_empty() {
        let pool = try_pool()
            .await
            .unwrap_or_else(|| panic!("PostgreSQL not available at {}", db_url()));
        let store = try_store()
            .await
            .unwrap_or_else(|| panic!("MinIO not available at {}", minio_config().0));
        let app = build_app(AppState {
            pool,
            store,
            registry: Arc::new(InMemoryProcessorRegistry::new()),
        });

        let response = app
            .oneshot(Request::builder().uri("/services").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();

        assert_eq!(body["services"], serde_json::json!([]));
    }
}
