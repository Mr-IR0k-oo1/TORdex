//! Re-exports the kernel event types for convenience.
//!
//! All event types are defined in `tordex-core` to avoid circular dependencies.
//! This module re-exports them so consumers can use `tordex_types::events::*`.

pub use tordex_core::event_store::{EventEnvelope, SystemEvent};
