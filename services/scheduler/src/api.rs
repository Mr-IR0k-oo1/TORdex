use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;

use tordex_core::id::{CollectionId, SourceId};
use tordex_events::EventBus;
use tordex_sources::SourceRegistry;
use tordex_types::CollectionContext;

use crate::router::CollectionRouter;
use crate::store::{CollectionRecord, CollectionStore, CollectionStoreError};

#[derive(Clone)]
pub struct CollectionsState {
    pub router: CollectionRouter,
    pub sources: Arc<dyn SourceRegistry>,
    pub store: Arc<dyn CollectionStore>,
    #[allow(dead_code)]
    pub events: Arc<dyn EventBus>,
}

impl CollectionsState {
    #[must_use]
    pub fn new(
        router: CollectionRouter,
        sources: Arc<dyn SourceRegistry>,
        store: Arc<dyn CollectionStore>,
        events: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            router,
            sources,
            store,
            events,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateCollectionRequest {
    pub source_id: String,
}

#[derive(Debug, Serialize)]
pub struct CreateCollectionResponse {
    pub collection_id: String,
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct CollectionResponse {
    pub id: String,
    pub source_id: String,
    pub status: String,
    pub collector_used: String,
    pub started_at: time::OffsetDateTime,
    pub completed_at: Option<time::OffsetDateTime>,
    pub final_url: Option<String>,
    pub content_type: Option<String>,
    pub byte_count: i64,
    pub http_status: Option<i32>,
    pub error_message: Option<String>,
}

impl From<CollectionRecord> for CollectionResponse {
    fn from(r: CollectionRecord) -> Self {
        Self {
            id: r.id.to_string(),
            source_id: r.source_id.to_string(),
            status: r.status,
            collector_used: r.collector_used,
            started_at: r.started_at,
            completed_at: r.completed_at,
            final_url: r.final_url,
            content_type: r.content_type,
            byte_count: r.byte_count,
            http_status: r.http_status,
            error_message: r.error_message,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub source_id: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub collections: Vec<CollectionResponse>,
    pub next_cursor: Option<String>,
}

pub fn router(state: CollectionsState) -> Router {
    Router::new()
        .route("/collections", post(create_collection))
        .route("/collections", get(list_collections))
        .route("/collections/{id}", get(get_collection))
        .with_state(state)
}

async fn create_collection(
    State(state): State<CollectionsState>,
    headers: HeaderMap,
    Json(req): Json<CreateCollectionRequest>,
) -> Result<(StatusCode, Json<CreateCollectionResponse>), ApiError> {
    let source_id = SourceId::from_str(&req.source_id)
        .ok_or_else(|| ApiError::Invalid(format!("invalid source_id: {}", req.source_id)))?;
    let source = state.sources.get(source_id).await.map_err(map_source_err)?;

    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    if let Some(key) = idempotency_key.as_deref() {
        if let Some(existing) = state
            .store
            .find_idempotent(&source.id, key)
            .await
            .map_err(map_store_err)?
        {
            return Ok((
                StatusCode::ACCEPTED,
                Json(CreateCollectionResponse {
                    collection_id: existing.id.to_string(),
                    status: "duplicate",
                }),
            ));
        }
    }

    let collection_id = CollectionId::generate();
    let ctx = CollectionContext {
        collection_id,
        source_id: source.id,
        url: source.locator.clone(),
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    let router = state.router.clone();
    let store = state.store.clone();
    let source_clone = source.clone();
    let key = idempotency_key;
    let key_for_task = key.clone();
    tokio::spawn(async move {
        let res = router.run(ctx, &source_clone).await;
        if let Some(k) = key_for_task {
            if let Err(e) = store.attach_idempotency_key(collection_id, &k).await {
                error!("failed to record idempotency_key: {e}");
            }
        }
        if let Err(e) = res {
            error!(?e, "collection run errored");
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateCollectionResponse {
            collection_id: collection_id.to_string(),
            status: "accepted",
        }),
    ))
}

async fn get_collection(
    State(state): State<CollectionsState>,
    Path(id): Path<String>,
) -> Result<Json<CollectionResponse>, ApiError> {
    let id = CollectionId::from_str(&id)
        .ok_or_else(|| ApiError::Invalid(format!("invalid collection id: {id}")))?;
    let record = state.store.get(id).await.map_err(map_store_err)?;
    Ok(Json(record.into()))
}

async fn list_collections(
    State(state): State<CollectionsState>,
    Query(params): Query<ListParams>,
) -> Result<Json<ListResponse>, ApiError> {
    let source_id_str = params
        .source_id
        .as_deref()
        .ok_or_else(|| ApiError::Invalid("source_id query parameter is required".into()))?;
    let source_id = SourceId::from_str(source_id_str)
        .ok_or_else(|| ApiError::Invalid(format!("invalid source_id: {source_id_str}")))?;
    let limit = params.limit.unwrap_or(50);
    let cursor = params
        .cursor
        .as_deref()
        .map(|s| {
            CollectionId::from_str(s)
                .ok_or_else(|| ApiError::Invalid(format!("invalid cursor: {s}")))
        })
        .transpose()?;

    let records = state
        .store
        .list_for_source(&source_id, limit, cursor)
        .await
        .map_err(map_store_err)?;
    let next_cursor = records.last().map(|r| r.id.to_string());
    let collections: Vec<CollectionResponse> = records.into_iter().map(Into::into).collect();
    Ok(Json(ListResponse {
        collections,
        next_cursor,
    }))
}

fn map_source_err(err: tordex_sources::SourceRegistryError) -> ApiError {
    use tordex_sources::SourceRegistryError;
    match err {
        SourceRegistryError::NotFound(id) => ApiError::NotFound(id.to_string()),
        SourceRegistryError::Duplicate { kind, locator } => {
            ApiError::Conflict(format!("{kind:?} with locator {locator:?} already exists"))
        }
        SourceRegistryError::Invalid(msg) => ApiError::Invalid(msg),
        SourceRegistryError::Core(e) => ApiError::Internal(e.to_string()),
    }
}

fn map_store_err(err: CollectionStoreError) -> ApiError {
    match err {
        CollectionStoreError::NotFound(id) => ApiError::NotFound(id.to_string()),
        CollectionStoreError::Core(e) => ApiError::Internal(e.to_string()),
    }
}

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    Conflict(String),
    Invalid(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Self::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                serde_json::json!({ "error": "not_found", "message": msg }),
            ),
            Self::Conflict(msg) => (
                StatusCode::CONFLICT,
                serde_json::json!({ "error": "conflict", "message": msg }),
            ),
            Self::Invalid(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                serde_json::json!({ "error": "invalid", "message": msg }),
            ),
            Self::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({ "error": "internal", "message": msg }),
            ),
        };
        (status, Json(body)).into_response()
    }
}
