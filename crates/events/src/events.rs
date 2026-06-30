//! Concrete event types emitted in this build.
//!
//! Event schemas will evolve as higher layers (Collection Sessions, Evidence
//! Lake, Agent Runtime) are implemented. The transport (see `bus.rs`) is
//! payload-agnostic, so adding new event variants is a non-breaking change.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use tordex_core::id::CollectionId;

/// A collection finished successfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionCompleted {
    pub collection_id: CollectionId,
    pub source_id: String,
    pub collector: String,
    pub status_code: Option<u16>,
    pub bytes: u64,
    pub duration_ms: u64,
    pub final_url: Option<String>,
    pub occurred_at: OffsetDateTime,
}

/// A collection attempt failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionFailed {
    pub collection_id: CollectionId,
    pub source_id: String,
    pub collector: String,
    pub error: String,
    pub duration_ms: u64,
    pub occurred_at: OffsetDateTime,
}