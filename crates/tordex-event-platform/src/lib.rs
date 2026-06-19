//! Layer 4 — Event Platform.
//!
//! Stub crate. The transport (`tordex-events`) is built and used by Layer 1
//! already. Full event-platform features (replay, retention, audit queries,
//! schema registry) will land here.

#![allow(dead_code)]

#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}