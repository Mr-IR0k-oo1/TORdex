//! Source registry abstractions.

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

use tordex_core::error::CoreError;
use tordex_core::id::SourceId;

use crate::descriptor::{SourceDescriptor, SourceInput, SourceKind, SourceStatus};

/// Errors produced by the source registry.
#[derive(Debug, Error)]
pub enum SourceRegistryError {
    #[error("source {0} not found")]
    NotFound(SourceId),
    #[error("source with kind {kind:?} and locator {locator:?} already exists")]
    Duplicate {
        kind: SourceKind,
        locator: String,
    },
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("invalid input: {0}")]
    Invalid(String),
}

impl From<sqlx::Error> for SourceRegistryError {
    fn from(err: sqlx::Error) -> Self {
        if matches!(err, sqlx::Error::RowNotFound) {
            // Surface as Core so callers can decide how to map it.
            return Self::Core(CoreError::infra(err.to_string()));
        }
        Self::Core(CoreError::infra(err.to_string()))
    }
}

/// Filter for listing sources.
#[derive(Debug, Clone, Default)]
pub struct SourceFilter {
    pub kind: Option<SourceKind>,
    pub status: Option<SourceStatus>,
    pub limit: Option<u32>,
    pub cursor: Option<SourceId>,
}

/// Result of a list query: rows plus an optional cursor for the next page.
#[derive(Debug, Clone)]
pub struct SourcePage {
    pub sources: Vec<SourceDescriptor>,
    pub next_cursor: Option<SourceId>,
}

/// The source registry. Persistence is opaque — implementations may be
/// in-memory (tests) or backed by Postgres.
#[async_trait]
pub trait SourceRegistry: Send + Sync {
    async fn insert(&self, input: &SourceInput) -> Result<SourceDescriptor, SourceRegistryError>;

    async fn get(&self, id: SourceId) -> Result<SourceDescriptor, SourceRegistryError>;

    async fn list(&self, filter: &SourceFilter) -> Result<SourcePage, SourceRegistryError>;

    async fn update(
        &self,
        id: SourceId,
        input: &SourceInput,
    ) -> Result<SourceDescriptor, SourceRegistryError>;

    async fn delete(&self, id: SourceId) -> Result<(), SourceRegistryError>;

    /// Lookup time helper used by implementations for the `created_at` /
    /// `updated_at` columns. Defaults to `tordex_core::now()` but exposed
    /// here so tests can pin a clock.
    fn clock(&self) -> OffsetDateTime {
        tordex_core::now()
    }
}