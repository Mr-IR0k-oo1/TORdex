use async_trait::async_trait;
use futures::stream::StreamExt;
use redis::aio::ConnectionManager;
use redis::streams::{StreamReadOptions, StreamReadReply};
use redis::AsyncCommands;
use std::fmt;

use crate::bus::{EventBus, EventBusError, EventEnvelope, EventStream};
use tordex_core::error::CoreError;

#[derive(Clone)]
pub struct RedisEventBus {
    manager: ConnectionManager,
    stream_key: String,
}

impl fmt::Debug for RedisEventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisEventBus")
            .field("stream_key", &self.stream_key)
            .finish_non_exhaustive()
    }
}

impl RedisEventBus {
    pub async fn connect(url: &str, stream_key: impl Into<String>) -> Result<Self, CoreError> {
        let client = redis::Client::open(url).map_err(|e| CoreError::infra(e.to_string()))?;
        let manager = ConnectionManager::new(client)
            .await
            .map_err(|e| CoreError::infra(e.to_string()))?;
        Ok(Self {
            manager,
            stream_key: stream_key.into(),
        })
    }

    fn field_key() -> &'static str {
        "envelope"
    }
}

#[async_trait]
impl EventBus for RedisEventBus {
    async fn publish(&self, envelope: &EventEnvelope<serde_json::Value>) -> Result<(), EventBusError> {
        let body = envelope.encode()?;
        let mut conn = self.manager.clone();
        let _: String = conn
            .xadd(
                &self.stream_key,
                "*",
                &[(Self::field_key(), body.as_ref())],
            )
            .await
            .map_err(|e| EventBusError::Unavailable(e.to_string()))?;
        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<EventStream, EventBusError> {
        let mut conn = self.manager.clone();
        let stream_key = self.stream_key.clone();
        let topic_owned = topic.to_string();

        let (tx, rx) = tokio::sync::mpsc::channel::<Result<EventEnvelope<serde_json::Value>, EventBusError>>(64);

        tokio::spawn(async move {
            let mut last_id = "0".to_string();
            let opts = StreamReadOptions::default().block(1000).count(64);
            loop {
                let result: Result<StreamReadReply, redis::RedisError> = conn
                    .xread_options(&[&stream_key], &[&last_id], &opts)
                    .await;
                match result {
                    Ok(reply) => {
                        for key in reply.keys {
                            for entry in key.ids {
                                last_id = entry.id.clone();
                                if let Some(value) = entry.map.get(Self::field_key()) {
                                    let bytes: Option<Vec<u8>> = match value {
                                        redis::Value::BulkString(b) => Some(b.clone()),
                                        redis::Value::SimpleString(s) => Some(s.clone().into_bytes()),
                                        redis::Value::Int(i) => Some(i.to_string().into_bytes()),
                                        _ => None,
                                    };
                                    if let Some(value) = bytes {
                                        match EventEnvelope::<serde_json::Value>::decode(&value) {
                                            Ok(env) => {
                                                if env.topic == topic_owned {
                                                    if tx.send(Ok(env)).await.is_err() {
                                                        return;
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx.send(Err(e)).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.to_ascii_lowercase().contains("timeout") {
                            continue;
                        }
                        let _ = tx.send(Err(EventBusError::Unavailable(msg))).await;
                        return;
                    }
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx).boxed();
        Ok(stream)
    }
}
