#![allow(dead_code)]

#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
