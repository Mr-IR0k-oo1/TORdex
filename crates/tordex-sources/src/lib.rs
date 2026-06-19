//! Layer 0 — Sources.
//!
//! The source registry is the catalog of "things TORdex can collect from."
//! It does not perform collection; that responsibility lives in Layer 1.
//! Keeping the descriptor declarative means a source can be re-collected with
//! different routing policies or collectors over time.

#![allow(clippy::module_name_repetitions)]

pub mod api;
pub mod descriptor;
pub mod pg_registry;
pub mod registry;

pub use api::{router, SourcesState};
pub use descriptor::{
    CollectionHints, RoutingPolicy, SourceDescriptor, SourceInput, SourceKind, SourceStatus,
    SourceValidationError,
};
pub use pg_registry::PgSourceRegistry;
pub use registry::{SourceFilter, SourcePage, SourceRegistry, SourceRegistryError};