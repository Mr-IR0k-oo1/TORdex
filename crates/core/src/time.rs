//! Time helpers.

use time::OffsetDateTime;

/// Returns the current UTC time. Centralised so that tests can later override it.
#[must_use]
pub fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}