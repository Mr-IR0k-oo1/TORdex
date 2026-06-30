use async_trait::async_trait;
use bytes::Bytes;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;

use tordex_core::id::{CollectionId, SourceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorKind {
    Http,
    BrowserLightpanda,
    BrowserChromium,
}

impl CollectorKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::BrowserLightpanda => "browser_lightpanda",
            Self::BrowserChromium => "browser_chromium",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "http" => Self::Http,
            "browser_lightpanda" => Self::BrowserLightpanda,
            "browser_chromium" => Self::BrowserChromium,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CollectionContext {
    pub collection_id: CollectionId,
    pub source_id: SourceId,
    pub url: String,
    pub cancel: CancellationToken,
}

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

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "rate_limited" => Self::RateLimited,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CollectionResult {
    pub id: CollectionId,
    pub source_id: SourceId,
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
    #[must_use]
    pub fn failure(
        ctx: &CollectionContext,
        collector: CollectorKind,
        error: impl Into<String>,
    ) -> Self {
        let now = tordex_core::now();
        Self {
            id: ctx.collection_id,
            source_id: ctx.source_id,
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

#[async_trait]
pub trait Collector: Send + Sync {
    fn kind(&self) -> CollectorKind;

    async fn collect(
        &self,
        ctx: &CollectionContext,
    ) -> Result<CollectionResult, CollectionError>;
}
