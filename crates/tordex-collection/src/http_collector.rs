//! HTTP collector backed by `reqwest`.
//!
//! This is the default collector. It supports redirect following, configurable
//! timeouts, and a max-bytes cap. A future improvement is to stream the body
//! to the Evidence Lake (Layer 3) instead of buffering in memory.

use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use http::StatusCode;
use reqwest::Client;
use time::OffsetDateTime;
use tracing::{debug, warn};

use tordex_sources::SourceDescriptor;

use crate::collector::{
    CollectionContext, CollectionError, CollectionResult, CollectionStatus, Collector, CollectorKind,
};

/// Settings for [`HttpCollector`].
#[derive(Debug, Clone)]
pub struct HttpCollectorConfig {
    pub user_agent: String,
    pub timeout: std::time::Duration,
    pub max_redirects: u8,
    pub max_bytes: u64,
}

impl Default for HttpCollectorConfig {
    fn default() -> Self {
        Self {
            user_agent: "TORdex/0.1".into(),
            timeout: std::time::Duration::from_secs(30),
            max_redirects: 5,
            max_bytes: 32 * 1024 * 1024, // 32 MiB
        }
    }
}

/// HTTP collector. Cheap to clone.
#[derive(Debug, Clone)]
pub struct HttpCollector {
    client: Client,
    config: HttpCollectorConfig,
}

impl HttpCollector {
    /// Build a new HTTP collector using the provided configuration.
    ///
    /// # Errors
    /// Returns an error if the underlying `reqwest::Client` cannot be built.
    pub fn new(config: HttpCollectorConfig) -> Result<Self, CollectionError> {
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.timeout)
            .redirect(reqwest::redirect::Policy::limited(config.max_redirects as usize))
            .build()
            .map_err(|e| CollectionError::Network(e.to_string()))?;
        Ok(Self { client, config })
    }
}

#[async_trait]
impl Collector for HttpCollector {
    fn kind(&self) -> CollectorKind {
        CollectorKind::Http
    }

    async fn collect(
        &self,
        ctx: &CollectionContext,
    ) -> Result<CollectionResult, CollectionError> {
        let started_at = tordex_core::now();
        let url = resolve_url(&ctx.source)?;

        debug!(?url, source_id = %ctx.source.id, "HTTP collector fetching");

        let response = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return Ok(cancelled(ctx, started_at, CollectorKind::Http)),
            res = self.client.get(&url).send() => res,
        }
        .map_err(|e| {
            if e.is_timeout() {
                CollectionError::Timeout
            } else {
                CollectionError::Network(e.to_string())
            }
        })?;

        let status = response.status();
        let final_url = Some(response.url().to_string());
        let headers: HeaderMap = response.headers().clone();
        let content_type = headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);

        let body = response.bytes().await.map_err(|e| {
            if e.is_timeout() {
                CollectionError::Timeout
            } else {
                CollectionError::Network(e.to_string())
            }
        })?;

        let byte_count = body.len() as u64;
        if byte_count > self.config.max_bytes {
            warn!(byte_count, max = self.config.max_bytes, "HTTP response exceeds max_bytes cap");
        }

        let body = if body.len() as u64 <= self.config.max_bytes {
            Some(body)
        } else {
            // Truncate to max_bytes to keep memory bounded.
            Some(Bytes::copy_from_slice(&body[..self.config.max_bytes as usize]))
        };

        let status_enum = if status.is_success() {
            CollectionStatus::Succeeded
        } else {
            CollectionStatus::Failed
        };

        Ok(CollectionResult {
            id: ctx.collection_id,
            source_id: ctx.source.id,
            collector: CollectorKind::Http,
            status: status_enum,
            started_at,
            completed_at: tordex_core::now(),
            final_url,
            content_type,
            byte_count,
            http_status: Some(status),
            body,
            error: if status.is_success() {
                None
            } else {
                Some(format!("HTTP {status}"))
            },
        })
    }
}

fn cancelled(
    ctx: &CollectionContext,
    started_at: OffsetDateTime,
    collector: CollectorKind,
) -> CollectionResult {
    CollectionResult {
        id: ctx.collection_id,
        source_id: ctx.source.id,
        collector,
        status: CollectionStatus::Cancelled,
        started_at,
        completed_at: tordex_core::now(),
        final_url: None,
        content_type: None,
        byte_count: 0,
        http_status: None,
        body: None,
        error: Some("cancelled".into()),
    }
}

/// Resolve the URL the HTTP collector should hit. Supports all locator
/// formats that are valid URLs; non-URL locators return an `InvalidResponse`
/// error so the caller can decide whether to escalate to a browser backend.
fn resolve_url(source: &SourceDescriptor) -> Result<String, CollectionError> {
    match source.kind {
        tordex_sources::SourceKind::Website
        | tordex_sources::SourceKind::OnionService
        | tordex_sources::SourceKind::Api
        | tordex_sources::SourceKind::RssFeed
        | tordex_sources::SourceKind::Document
        | tordex_sources::SourceKind::Paper => {
            url::Url::parse(&source.locator)
                .map(|u| u.to_string())
                .map_err(|e| CollectionError::InvalidResponse(format!("invalid URL: {e}")))
        }
        tordex_sources::SourceKind::Repository => Ok(source.locator.clone()),
        tordex_sources::SourceKind::LocalFile => Ok(source.locator.clone()),
    }
}

/// Heuristic used by the Auto router to decide whether to escalate from HTTP
/// to a browser backend.
pub fn needs_browser_escalation(content_type: Option<&str>, body: &[u8]) -> bool {
    let is_html = content_type
        .map(|c| c.to_ascii_lowercase().contains("html"))
        .unwrap_or(false);
    if !is_html {
        return false;
    }
    // SPA markers: empty body, <head> with lots of script tags, or a known
    // hydration root.
    if body.is_empty() {
        return true;
    }
    let script_tags = count_subsequence(body, b"<script");
    let body_len = body.len();
    if body_len < 2048 && script_tags >= 2 {
        return true;
    }
    // <div id="root"> or similar hydration roots.
    body.windows(5).any(|w| w.eq_ignore_ascii_case(b"id=\"r"))
        || body.windows(7).any(|w| w.eq_ignore_ascii_case(b"id=\"app"))
}

fn count_subsequence(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|w| w.eq_ignore_ascii_case(needle))
        .count()
}