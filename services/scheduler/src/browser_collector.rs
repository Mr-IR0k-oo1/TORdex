use async_trait::async_trait;

use tordex_types::{
    CollectionContext, CollectionError, CollectionResult, Collector, CollectorKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBackend {
    Lightpanda,
    Chromium,
}

#[derive(Debug, Clone)]
pub struct BrowserCollector {
    backend: BrowserBackend,
}

impl BrowserCollector {
    #[must_use]
    pub const fn lightpanda() -> Self {
        Self {
            backend: BrowserBackend::Lightpanda,
        }
    }

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
