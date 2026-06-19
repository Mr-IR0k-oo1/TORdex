//! HTTP API for the source registry.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use tordex_core::id::SourceId;

use crate::descriptor::{SourceDescriptor, SourceInput, SourceValidationError};
use crate::registry::{SourceFilter, SourceRegistry, SourceRegistryError};

/// Shared state for the sources router.
#[derive(Clone)]
pub struct SourcesState {
    pub registry: Arc<dyn SourceRegistry>,
}

impl SourcesState {
    #[must_use]
    pub fn new(registry: Arc<dyn SourceRegistry>) -> Self {
        Self { registry }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub kind: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub sources: Vec<SourceDescriptor>,
    pub next_cursor: Option<String>,
}

/// Routes mounted under `/sources`.
pub fn router(state: SourcesState) -> Router {
    Router::new()
        .route("/sources", post(create_source))
        .route("/sources", get(list_sources))
        .route("/sources/{id}", get(get_source))
        .route("/sources/{id}", patch(update_source))
        .route("/sources/{id}", delete(delete_source))
        .with_state(state)
}

async fn create_source(
    State(state): State<SourcesState>,
    Json(input): Json<SourceInput>,
) -> Result<(StatusCode, Json<SourceDescriptor>), ApiError> {
    input.validate().map_err(ApiError::from)?;
    let descriptor = state.registry.insert(&input).await?;
    Ok((StatusCode::CREATED, Json(descriptor)))
}

async fn get_source(
    State(state): State<SourcesState>,
    Path(id): Path<String>,
) -> Result<Json<SourceDescriptor>, ApiError> {
    let id = parse_id(&id)?;
    let descriptor = state.registry.get(id).await?;
    Ok(Json(descriptor))
}

async fn list_sources(
    State(state): State<SourcesState>,
    Query(params): Query<ListParams>,
) -> Result<Json<ListResponse>, ApiError> {
    let kind = params.kind.as_deref().map(parse_kind).transpose()?;
    let status = params.status.as_deref().map(parse_status).transpose()?;
    let cursor = params
        .cursor
        .as_deref()
        .map(|s| {
            SourceId::from_str(s)
                .ok_or_else(|| format!("invalid cursor: {s}"))
        })
        .transpose()?;

    let page = state
        .registry
        .list(&SourceFilter {
            kind,
            status,
            limit: params.limit,
            cursor,
        })
        .await?;

    Ok(Json(ListResponse {
        next_cursor: page.next_cursor.map(|c| c.to_string()),
        sources: page.sources,
    }))
}

async fn update_source(
    State(state): State<SourcesState>,
    Path(id): Path<String>,
    Json(input): Json<SourceInput>,
) -> Result<Json<SourceDescriptor>, ApiError> {
    let id = parse_id(&id)?;
    input.validate().map_err(ApiError::from)?;
    let descriptor = state.registry.update(id, &input).await?;
    Ok(Json(descriptor))
}

async fn delete_source(
    State(state): State<SourcesState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let id = parse_id(&id)?;
    state.registry.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_id(s: &str) -> Result<SourceId, ApiError> {
    SourceId::from_str(s).ok_or_else(|| ApiError::Invalid(format!("invalid source id: {s}")))
}

fn parse_kind(s: &str) -> Result<crate::descriptor::SourceKind, String> {
    Ok(match s {
        "website" => crate::descriptor::SourceKind::Website,
        "onion_service" => crate::descriptor::SourceKind::OnionService,
        "api" => crate::descriptor::SourceKind::Api,
        "repository" => crate::descriptor::SourceKind::Repository,
        "document" => crate::descriptor::SourceKind::Document,
        "rss_feed" => crate::descriptor::SourceKind::RssFeed,
        "local_file" => crate::descriptor::SourceKind::LocalFile,
        "paper" => crate::descriptor::SourceKind::Paper,
        other => return Err(format!("unknown kind: {other}")),
    })
}

fn parse_status(s: &str) -> Result<crate::descriptor::SourceStatus, String> {
    Ok(match s {
        "active" => crate::descriptor::SourceStatus::Active,
        "paused" => crate::descriptor::SourceStatus::Paused,
        "errored" => crate::descriptor::SourceStatus::Errored,
        other => return Err(format!("unknown status: {other}")),
    })
}

/// API error type. Maps cleanly to HTTP responses with appropriate status codes.
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

impl From<SourceRegistryError> for ApiError {
    fn from(err: SourceRegistryError) -> Self {
        match err {
            SourceRegistryError::NotFound(id) => Self::NotFound(id.to_string()),
            SourceRegistryError::Duplicate { kind, locator } => {
                Self::Conflict(format!("{kind:?} with locator {locator:?} already exists"))
            }
            SourceRegistryError::Invalid(msg) => Self::Invalid(msg),
            SourceRegistryError::Core(e) => Self::Internal(e.to_string()),
        }
    }
}

impl From<SourceValidationError> for ApiError {
    fn from(err: SourceValidationError) -> Self {
        Self::Invalid(err.to_string())
    }
}

impl From<String> for ApiError {
    fn from(err: String) -> Self {
        Self::Invalid(err)
    }
}