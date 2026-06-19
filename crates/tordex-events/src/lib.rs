//! Event transport for TORdex.
//!
//! The transport is intentionally thin: it moves bytes over a topic and lets
//! producers/consumers agree on the payload schema. Replay, retention, schema
//! evolution and audit queries belong to Layer 4 (Event Platform), built later.

#![allow(clippy::module_name_repetitions)]

pub mod bus;
pub mod events;
pub mod in_memory;
pub mod redis_bus;

pub use bus::{EventBus, EventBusError, EventEnvelope, EventStream};
pub use events::{CollectionCompleted, CollectionFailed};
pub use in_memory::InMemoryEventBus;
pub use redis_bus::RedisEventBus;