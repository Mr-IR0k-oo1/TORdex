use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ulid::Ulid;
use serde::{Deserialize, Serialize};

pub type EventId = Ulid;
pub type SubscriptionId = Ulid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub topic: String,
    pub payload: Vec<u8>,
    pub occurred_at: i64,
}

pub trait EventManager: Send + Sync {
    fn publish(&self, topic: &str, payload: &[u8]) -> EventId;
    fn subscribe(&self, topic: &str) -> SubscriptionId;
    fn unsubscribe(&self, id: SubscriptionId) -> bool;
    fn poll(&self, id: SubscriptionId) -> Vec<Event>;
}

type Topic = String;
type Subscribers = Arc<Mutex<HashMap<SubscriptionId, Topic>>>;
type Events = Arc<Mutex<Vec<Event>>>;

pub struct InMemoryEventManager {
    subscribers: Subscribers,
    events: Events,
}

impl InMemoryEventManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for InMemoryEventManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EventManager for InMemoryEventManager {
    fn publish(&self, topic: &str, payload: &[u8]) -> EventId {
        let id = Ulid::new();
        let event = Event {
            id,
            topic: topic.to_string(),
            payload: payload.to_vec(),
            occurred_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        };
        self.events.lock().unwrap().push(event);
        id
    }

    fn subscribe(&self, topic: &str) -> SubscriptionId {
        let id = Ulid::new();
        self.subscribers
            .lock()
            .unwrap()
            .insert(id, topic.to_string());
        id
    }

    fn unsubscribe(&self, id: SubscriptionId) -> bool {
        self.subscribers.lock().unwrap().remove(&id).is_some()
    }

    fn poll(&self, id: SubscriptionId) -> Vec<Event> {
        let topic = match self.subscribers.lock().unwrap().get(&id) {
            Some(t) => t.clone(),
            None => return Vec::new(),
        };
        let events = self.events.lock().unwrap();
        events
            .iter()
            .filter(|e| e.topic == topic)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_and_poll() {
        let mgr = InMemoryEventManager::new();
        let sub = mgr.subscribe("test.topic");
        mgr.publish("test.topic", b"hello");
        let events = mgr.poll(sub);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload, b"hello");
    }

    #[test]
    fn unsubscribe() {
        let mgr = InMemoryEventManager::new();
        let sub = mgr.subscribe("test");
        assert!(mgr.unsubscribe(sub));
        assert!(!mgr.unsubscribe(sub));
    }

    #[test]
    fn different_topics() {
        let mgr = InMemoryEventManager::new();
        let sub_a = mgr.subscribe("a");
        let sub_b = mgr.subscribe("b");
        mgr.publish("a", b"only a");
        assert_eq!(mgr.poll(sub_a).len(), 1);
        assert_eq!(mgr.poll(sub_b).len(), 0);
    }
}
