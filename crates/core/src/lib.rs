//! Shared primitives for TORdex.
//!
//! Provides typed identifiers (ULID-backed), time helpers, configuration
//! loading, and the cross-cutting error type used by every crate.

#![allow(clippy::module_name_repetitions)]

pub mod error;
pub mod id;
pub mod time;

pub use error::CoreError;
pub use id::{CollectionId, EvidenceId, SessionId, SourceId};
pub use time::now;

/// Crate-wide result alias.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;