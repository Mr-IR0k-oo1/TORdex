use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};
use url::Url;

use tordex_events::{CollectionCompleted, CollectionFailed, EventBus, EventEnvelope};
use tordex_sources::{RoutingPolicy, SourceDescriptor};
use tordex_types::{
    CollectionContext, CollectionError, CollectionResult, CollectionStatus, Collector, CollectorKind,
};

use crate::http_collector::needs_browser_escalation;
use crate::store::CollectionStore;

type HostLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

#[derive(Clone)]
pub struct CollectionRouter {
    collectors: HashMap<CollectorKind, Arc<dyn Collector>>,
    host_limiters: Arc<Mutex<HashMap<String, Arc<HostLimiter>>>>,
    quota: Quota,
    events: Arc<dyn EventBus>,
    store: Arc<dyn CollectionStore>,
}

impl CollectionRouter {
    pub fn new(
        http: impl Collector + 'static,
        events: Arc<dyn EventBus>,
        store: Arc<dyn CollectionStore>,
        rate_per_second: u32,
        rate_burst: u32,
    ) -> Self {
        let per_second = NonZeroU32::new(rate_per_second.max(1)).unwrap_or(nonzero!(1u32));
        let burst = NonZeroU32::new(rate_burst.max(1)).unwrap_or(nonzero!(1u32));
        let quota = Quota::per_second(per_second).allow_burst(burst);
        let mut collectors: HashMap<CollectorKind, Arc<dyn Collector>> = HashMap::new();
        collectors.insert(http.kind(), Arc::new(http));
        Self {
            collectors,
            host_limiters: Arc::new(Mutex::new(HashMap::new())),
            quota,
            events,
            store,
        }
    }

    #[must_use]
    pub fn with_collector(mut self, kind: CollectorKind, collector: Arc<dyn Collector>) -> Self {
        self.collectors.insert(kind, collector);
        self
    }

    pub async fn run(
        &self,
        ctx: CollectionContext,
        source: &SourceDescriptor,
    ) -> Result<CollectionResult, CollectionError> {
        let id = ctx.collection_id;
        let initial_kind = self.pick_collector_kind(source);

        self.store
            .record_started(&source.id, id, initial_kind)
            .await
            .map_err(|e| CollectionError::Other(Box::new(e)))?;

        if let Some(host) = host_of(source) {
            let limiter = self.limiter_for(host).await;
            if limiter.check().is_err() {
                let result = CollectionResult {
                    id,
                    source_id: source.id,
                    collector: initial_kind,
                    status: CollectionStatus::RateLimited,
                    started_at: tordex_core::now(),
                    completed_at: tordex_core::now(),
                    final_url: None,
                    content_type: None,
                    byte_count: 0,
                    http_status: None,
                    body: None,
                    error: Some("rate_limited".into()),
                };
                self.store
                    .record_finished(&result)
                    .await
                    .map_err(|e| CollectionError::Other(Box::new(e)))?;
                self.emit_failed(&result, "rate_limited").await;
                return Ok(result);
            }
        }

        let result = match self.dispatch(&ctx).await {
            Ok(r) => r,
            Err(err) => {
                let result = CollectionResult::failure(&ctx, initial_kind, err.to_string());
                self.store
                    .record_finished(&result)
                    .await
                    .map_err(|e| CollectionError::Other(Box::new(e)))?;
                self.emit_failed(&result, result.error.as_deref().unwrap_or("failed"))
                    .await;
                return Ok(result);
            }
        };

        let final_result = if matches!(result.collector, CollectorKind::Http)
            && matches!(source.routing_policy, RoutingPolicy::Auto)
            && needs_browser_escalation(result.content_type.as_deref(), result.body.as_deref().unwrap_or_default())
        {
            let browser = self
                .collectors
                .get(&CollectorKind::BrowserLightpanda)
                .or_else(|| self.collectors.get(&CollectorKind::BrowserChromium))
                .cloned();
            match browser {
                Some(b) => {
                    warn!(source_id = %source.id, "auto-escalating to browser");
                    b.collect(&ctx).await.unwrap_or_else(|e| {
                        CollectionResult::failure(&ctx, b.kind(), e.to_string())
                    })
                }
                None => result,
            }
        } else {
            result
        };

        self.store
            .record_finished(&final_result)
            .await
            .map_err(|e| CollectionError::Other(Box::new(e)))?;

        if matches!(final_result.status, CollectionStatus::Succeeded) {
            self.emit_completed(&final_result).await;
        } else {
            self.emit_failed(
                &final_result,
                final_result.error.as_deref().unwrap_or("failed"),
            )
            .await;
        }

        Ok(final_result)
    }

    async fn dispatch(&self, ctx: &CollectionContext) -> Result<CollectionResult, CollectionError> {
        let collector = self
            .collectors
            .get(&CollectorKind::Http)
            .cloned()
            .ok_or_else(|| CollectionError::BrowserUnavailable("HTTP collector not registered".into()))?;
        collector.collect(ctx).await
    }

    fn pick_collector_kind(&self, source: &SourceDescriptor) -> CollectorKind {
        match source.routing_policy {
            RoutingPolicy::Http => CollectorKind::Http,
            RoutingPolicy::Browser => {
                if self.collectors.contains_key(&CollectorKind::BrowserLightpanda) {
                    CollectorKind::BrowserLightpanda
                } else if self.collectors.contains_key(&CollectorKind::BrowserChromium) {
                    CollectorKind::BrowserChromium
                } else {
                    CollectorKind::Http
                }
            }
            RoutingPolicy::Auto => CollectorKind::Http,
        }
    }

    async fn limiter_for(&self, host: String) -> Arc<HostLimiter> {
        let mut map = self.host_limiters.lock().await;
        map.entry(host)
            .or_insert_with(|| Arc::new(RateLimiter::direct(self.quota)))
            .clone()
    }

    async fn emit_completed(&self, result: &CollectionResult) {
        let duration_ms = (result.completed_at - result.started_at)
            .whole_milliseconds()
            .max(0) as u64;
        let event = CollectionCompleted {
            collection_id: result.id,
            source_id: result.source_id.to_string(),
            collector: result.collector.as_str().to_string(),
            status_code: result.http_status.map(|s| s.as_u16()),
            bytes: result.byte_count,
            duration_ms,
            final_url: result.final_url.clone(),
            occurred_at: result.completed_at,
        };
        match serde_json::to_value(&event) {
            Ok(value) => {
                let envelope = EventEnvelope::new("collection.completed", value);
                if let Err(e) = self.events.publish(&envelope).await {
                    warn!("failed to publish CollectionCompleted: {e}");
                } else {
                    info!(collection_id = %result.id, "CollectionCompleted emitted");
                }
            }
            Err(e) => warn!("failed to serialize CollectionCompleted: {e}"),
        }
    }

    async fn emit_failed(&self, result: &CollectionResult, error: &str) {
        let duration_ms = (result.completed_at - result.started_at)
            .whole_milliseconds()
            .max(0) as u64;
        let event = CollectionFailed {
            collection_id: result.id,
            source_id: result.source_id.to_string(),
            collector: result.collector.as_str().to_string(),
            error: error.to_string(),
            duration_ms,
            occurred_at: result.completed_at,
        };
        match serde_json::to_value(&event) {
            Ok(value) => {
                let envelope = EventEnvelope::new("collection.failed", value);
                if let Err(e) = self.events.publish(&envelope).await {
                    warn!("failed to publish CollectionFailed: {e}");
                }
            }
            Err(e) => warn!("failed to serialize CollectionFailed: {e}"),
        }
    }
}

fn host_of(source: &SourceDescriptor) -> Option<String> {
    match source.kind {
        tordex_sources::SourceKind::Website
        | tordex_sources::SourceKind::OnionService
        | tordex_sources::SourceKind::Api
        | tordex_sources::SourceKind::RssFeed
        | tordex_sources::SourceKind::Document
        | tordex_sources::SourceKind::Paper => Url::parse(&source.locator)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string)),
        _ => None,
    }
}
