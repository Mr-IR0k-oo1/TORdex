use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::StreamExt;
use std::fmt;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::bus::{EventBus, EventBusError, EventEnvelope, EventStream};

const DEFAULT_CAPACITY: usize = 1024;

#[derive(Clone)]
pub struct InMemoryEventBus {
    sender: Arc<broadcast::Sender<Bytes>>,
    capacity: usize,
}

impl fmt::Debug for InMemoryEventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemoryEventBus")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl InMemoryEventBus {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self {
            sender: Arc::new(sender),
            capacity,
        }
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, envelope: &EventEnvelope<serde_json::Value>) -> Result<(), EventBusError> {
        let body = envelope.encode()?;
        let _ = self.sender.send(body);
        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<EventStream, EventBusError> {
        let mut rx = self.sender.subscribe();
        let topic = topic.to_string();
        let stream = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(bytes) => match EventEnvelope::<serde_json::Value>::decode(&bytes) {
                        Ok(env) if env.topic == topic => yield Ok(env),
                        Ok(_) => continue,
                        Err(e) => yield Err(e),
                    },
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("in-memory event bus subscriber lagged by {n} events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };
        Ok(stream.boxed())
    }
}
