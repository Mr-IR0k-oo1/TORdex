use async_trait::async_trait;
use bytes::Bytes;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;
use thiserror::Error;
use time::OffsetDateTime;
use ulid::Ulid;

use tordex_core::error::CoreError;

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

#[derive(Debug, Clone)]
pub struct EventEnvelope<T> {
    pub id: Ulid,
    pub topic: String,
    pub occurred_at: OffsetDateTime,
    pub payload: T,
}

impl<T> EventEnvelope<T> {
    pub fn new(topic: impl Into<String>, payload: T) -> Self {
        Self {
            id: Ulid::new(),
            topic: topic.into(),
            occurred_at: OffsetDateTime::now_utc(),
            payload,
        }
    }

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

pub type EventStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<EventEnvelope<serde_json::Value>, EventBusError>> + Send>>;

#[async_trait]
pub trait EventBus: Send + Sync + fmt::Debug {
    async fn publish(&self, envelope: &EventEnvelope<serde_json::Value>) -> Result<(), EventBusError>;

    async fn subscribe(&self, topic: &str) -> Result<EventStream, EventBusError>;
}
