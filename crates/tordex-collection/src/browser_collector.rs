//! Browser-based collector (feature-gated).
//!
//! Built on top of `chromiumoxide`. Two backends are supported:
//!
//! - `Lightpanda`: connect to an externally-running Lightpanda via its CDP
//!   WebSocket URL (`TORDEX_LIGHTPANDA_CDP_URL`).
//! - `Chromium`: launch a local Chromium via `Browser::launch`.
//!
//! This stub compiles without the `browser` feature so that `cargo build`
//! succeeds even when no browser is available. The actual CDP integration
//! is non-trivial and lands in a follow-up.

use crate::collector::{CollectionError, Collector, CollectorKind};
use async_trait::async_trait;
use tordex_sources::SourceDescriptor;

use crate::collector::{CollectionContext, CollectionResult};

/// Which browser backend to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBackend {
    /// Connect to an externally-running Lightpanda CDP server.
    Lightpanda,
    /// Launch a local Chromium.
    Chromium,
}

/// A handle to a browser backend. Construct via [`BrowserCollector::connect`]
/// or [`BrowserCollector::launch`].
#[derive(Debug, Clone)]
pub struct BrowserCollector {
    backend: BrowserBackend,
}

impl BrowserCollector {
    /// Connect to a Lightpanda instance at the given WebSocket URL.
    #[must_use]
    pub const fn lightpanda() -> Self {
        Self {
            backend: BrowserBackend::Lightpanda,
        }
    }

    /// Launch a local Chromium.
    #[must_use]
    pub const fn chromium() -> Self {
        Self {
            backend: BrowserBackend::Chromium,
        }
    }
}

#[async_trait]
impl Collector for BrowserCollector {
    fn kind(&self) -> CollectorKind {
        match self.backend {
            BrowserBackend::Lightpanda => CollectorKind::BrowserLightpanda,
            BrowserBackend::Chromium => CollectorKind::BrowserChromium,
        }
    }

    async fn collect(
        &self,
        ctx: &CollectionContext,
    ) -> Result<CollectionResult, CollectionError> {
        // Real implementation will use `chromiumoxide::Browser` to:
        //   1. Page.navigate to the source URL
        //   2. Wait for Page.loadEventFired
        //   3. Capture body text and final URL
        // For now, we mark the attempt as failed with a clear message so
        // the rest of the system continues to function.
        let _ = ctx;
        Ok(CollectionResult::failure(
            ctx,
            self.kind(),
            format!(
                "browser backend {:?} is not yet wired (requires `browser` feature and a real CDP target)",
                self.backend
            ),
        ))
    }
}

/// Returns a default source locator that the browser would visit. Helper
/// kept for future use; equivalent to `http_collector::resolve_url`.
#[allow(dead_code)]
pub(crate) fn resolve_url(source: &SourceDescriptor) -> Result<String, CollectionError> {
    match source.kind {
        tordex_sources::SourceKind::LocalFile | tordex_sources::SourceKind::Repository => {
            Ok(source.locator.clone())
        }
        _ => url::Url::parse(&source.locator)
            .map(|u| u.to_string())
            .map_err(|e| CollectionError::InvalidResponse(format!("invalid URL: {e}"))),
    }
}