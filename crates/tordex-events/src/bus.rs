//! Generic event transport.
//!
//! Provides an [`EventBus`] trait with two implementations:
//!
//! - [`RedisEventBus`](crate::redis_bus::RedisEventBus) — production transport
//!   using Redis Streams (`XADD`/`XREAD`).
//! - [`InMemoryEventBus`](crate::in_memory::InMemoryEventBus) — `tokio::sync::broadcast`
//!   backed, used in tests and as a fallback when Redis is unavailable.
//!
//! The transport is intentionally thin. Replay, retention, schema evolution
//! and audit-query logic belong to Layer 4 (Event Platform), built later.

use async_trait::async_trait;
use bytes::Bytes;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;
use thiserror::Error;
use time::OffsetDateTime;
use ulid::Ulid;

use tordex_core::error::CoreError;

/// Errors that can occur while publishing or consuming events.
#[derive(Debug, Error)]
pub enum EventBusError {
    #[error("event bus is unavailable: {0}")]
    Unavailable(String),

    #[error("event bus encoding error: {0}")]
    Encoding(String),

    #[error(transparent)]
    Core(#[from] CoreError),
}

impl From<serde_json::Error> for EventBusError {
    fn from(err: serde_json::Error) -> Self {
        Self::Encoding(err.to_string())
    }
}

/// Wraps a payload with metadata so consumers can route and time-order events
/// without parsing the payload body.
#[derive(Debug, Clone)]
pub struct EventEnvelope<T> {
    pub id: Ulid,
    pub topic: String,
    pub occurred_at: OffsetDateTime,
    pub payload: T,
}

impl<T> EventEnvelope<T> {
    /// Wrap a payload in a fresh envelope addressed to `topic`.
    pub fn new(topic: impl Into<String>, payload: T) -> Self {
        Self {
            id: Ulid::new(),
            topic: topic.into(),
            occurred_at: OffsetDateTime::now_utc(),
            payload,
        }
    }

    /// Encode the envelope to JSON bytes.
    pub fn encode(&self) -> Result<Bytes, EventBusError>
    where
        T: Serialize,
    {
        let value = serde_json::json!({
            "id": self.id.to_string(),
            "topic": self.topic,
            "occurred_at": self.occurred_at,
            "payload": self.payload,
        });
        Ok(Bytes::from(serde_json::to_vec(&value)?))
    }

    /// Decode an envelope from JSON bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, EventBusError>
    where
        T: DeserializeOwned,
    {
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        let id_str = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| EventBusError::Encoding("missing id".into()))?;
        let id = Ulid::from_string(id_str)
            .map_err(|e| EventBusError::Encoding(format!("invalid id: {e}")))?;
        let topic = value
            .get("topic")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| EventBusError::Encoding("missing topic".into()))?
            .to_string();
        let occurred_at = value
            .get("occurred_at")
            .ok_or_else(|| EventBusError::Encoding("missing occurred_at".into()))?;
        let occurred_at: OffsetDateTime = serde_json::from_value(occurred_at.clone())?;
        let payload: T = serde_json::from_value(
            value
                .get("payload")
                .cloned()
                .ok_or_else(|| EventBusError::Encoding("missing payload".into()))?,
        )?;
        Ok(Self {
            id,
            topic,
            occurred_at,
            payload,
        })
    }
}

/// Stream of events from a subscription.
pub type EventStream<T> =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<EventEnvelope<T>, EventBusError>> + Send>>;

/// Transport for events. Implementations must be `Send + Sync` so they can be
/// shared across tokio tasks.
#[async_trait]
pub trait EventBus: Send + Sync + fmt::Debug {
    /// Publish a single envelope.
    async fn publish<T>(&self, envelope: &EventEnvelope<T>) -> Result<(), EventBusError>
    where
        T: Serialize + Send + Sync;

    /// Subscribe to a topic. Returns a stream that yields envelopes matching
    /// the topic prefix (the topic is matched exactly today; hierarchical
    /// topics are a Layer 4 feature).
    async fn subscribe<T>(&self, topic: &str) -> Result<EventStream<T>, EventBusError>
    where
        T: DeserializeOwned + Send + 'static;
}