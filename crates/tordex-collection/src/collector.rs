//! Collector trait and supporting types.

use async_trait::async_trait;
use bytes::Bytes;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;

use tordex_core::id::CollectionId;
use tordex_sources::SourceDescriptor;

/// Identity of a concrete collector implementation. Persisted on the
/// `collections` row so that downstream layers can attribute results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorKind {
    Http,
    #[cfg(feature = "browser")]
    BrowserLightpanda,
    #[cfg(feature = "browser")]
    BrowserChromium,
}

impl CollectorKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "http",
            #[cfg(feature = "browser")]
            Self::BrowserLightpanda => "browser_lightpanda",
            #[cfg(feature = "browser")]
            Self::BrowserChromium => "browser_chromium",
        }
    }
}

/// Per-attempt context passed to a collector.
#[derive(Debug, Clone)]
pub struct CollectionContext {
    pub collection_id: CollectionId,
    pub source: SourceDescriptor,
    pub cancel: CancellationToken,
}

/// Final status of a collection attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionStatus {
    Succeeded,
    Failed,
    Cancelled,
    RateLimited,
}

impl CollectionStatus {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::RateLimited => "rate_limited",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "rate_limited" => Self::RateLimited,
            _ => return None,
        })
    }
}

/// Result of a successful or failed collection attempt.
#[derive(Debug, Clone)]
pub struct CollectionResult {
    pub id: CollectionId,
    pub source_id: tordex_core::id::SourceId,
    pub collector: CollectorKind,
    pub status: CollectionStatus,
    pub started_at: OffsetDateTime,
    pub completed_at: OffsetDateTime,
    pub final_url: Option<String>,
    pub content_type: Option<String>,
    pub byte_count: u64,
    pub http_status: Option<StatusCode>,
    pub body: Option<Bytes>,
    pub error: Option<String>,
}

impl CollectionResult {
    /// Build a failed result for a given context.
    #[must_use]
    pub fn failure(
        ctx: &CollectionContext,
        collector: CollectorKind,
        error: impl Into<String>,
    ) -> Self {
        let now = tordex_core::now();
        Self {
            id: ctx.collection_id,
            source_id: ctx.source.id,
            collector,
            status: CollectionStatus::Failed,
            started_at: now,
            completed_at: now,
            final_url: None,
            content_type: None,
            byte_count: 0,
            http_status: None,
            body: None,
            error: Some(error.into()),
        }
    }
}

/// Errors produced by collectors.
#[derive(Debug, Error)]
pub enum CollectionError {
    #[error("network error: {0}")]
    Network(String),
    #[error("timeout")]
    Timeout,
    #[error("rate limited")]
    RateLimited,
    #[error("cancelled")]
    Cancelled,
    #[error("browser backend unavailable: {0}")]
    BrowserUnavailable(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// A collector turns a `Source` into a `CollectionResult`.
#[async_trait]
pub trait Collector: Send + Sync {
    /// Identity of this implementation.
    fn kind(&self) -> CollectorKind;

    /// Execute the collection. Implementations should respect `ctx.cancel`.
    async fn collect(
        &self,
        ctx: &CollectionContext,
    ) -> Result<CollectionResult, CollectionError>;
}