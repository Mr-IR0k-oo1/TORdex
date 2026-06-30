//! Layer 2 — Collection Sessions.
//!
//! Stub crate. Real implementation lands in the next build step. The crate
//! exists today so the workspace compiles and the dependency graph for higher
//! layers can be designed against a stable name.

#![allow(dead_code)]

/// Return the version of this crate. The first build delivers only this
/// function; richer behaviour lands as Layer 2 is fully implemented.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}