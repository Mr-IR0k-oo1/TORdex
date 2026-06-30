//! Layer 1 — Collection Fabric.
//!
//! Decides which collector to use for each source, executes it, records the
//! result, and emits a completion event. The current build supports the HTTP
//! collector and reserves a feature-gated browser path for sites that need JS
//! rendering.

#![allow(clippy::module_name_repetitions)]

pub mod api;
pub mod browser_collector;
pub mod collector;
pub mod http_collector;
pub mod router;
pub mod store;

pub use api::{router, CollectionsState, CreateCollectionRequest};
pub use collector::{
    CollectionContext, CollectionError, CollectionResult, CollectionStatus, Collector, CollectorKind,
};
pub use http_collector::{HttpCollector, HttpCollectorConfig};
pub use router::CollectionRouter;
pub use store::{CollectionRecord, CollectionStore, PgCollectionStore};