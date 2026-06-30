use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ulid::Ulid;

pub type MessageId = Ulid;
pub type EndpointId = Ulid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub source: EndpointId,
    pub target: EndpointId,
    pub kind: String,
    pub payload: Vec<u8>,
    pub reply_to: Option<MessageId>,
}

pub trait Ipc: Send + Sync {
    fn send(&self, msg: Message) -> Result<MessageId, String>;
    fn receive(&self, endpoint: EndpointId) -> Vec<Message>;
    fn register_endpoint(&self) -> EndpointId;
    fn unregister_endpoint(&self, id: EndpointId) -> bool;
    fn pending_count(&self, endpoint: EndpointId) -> usize;
}

pub struct InMemoryIpc {
    mailboxes: Arc<Mutex<HashMap<EndpointId, Vec<Message>>>>,
    endpoints: Arc<Mutex<HashMap<EndpointId, bool>>>,
}

impl InMemoryIpc {
    #[must_use]
    pub fn new() -> Self {
        Self {
            mailboxes: Arc::new(Mutex::new(HashMap::new())),
            endpoints: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryIpc {
    fn default() -> Self {
        Self::new()
    }
}

impl Ipc for InMemoryIpc {
    fn send(&self, msg: Message) -> Result<MessageId, String> {
        {
            let endpoints = self.endpoints.lock().unwrap();
            if !endpoints.contains_key(&msg.target) {
                return Err(format!("unknown endpoint: {}", msg.target));
            }
        }
        let id = msg.id;
        self.mailboxes
            .lock()
            .unwrap()
            .entry(msg.target)
            .or_default()
            .push(msg);
        Ok(id)
    }

    fn receive(&self, endpoint: EndpointId) -> Vec<Message> {
        let mut mailboxes = self.mailboxes.lock().unwrap();
        mailboxes.remove(&endpoint).unwrap_or_default()
    }

    fn register_endpoint(&self) -> EndpointId {
        let id = EndpointId::new();
        self.endpoints.lock().unwrap().insert(id, true);
        self.mailboxes.lock().unwrap().entry(id).or_default();
        id
    }

    fn unregister_endpoint(&self, id: EndpointId) -> bool {
        self.endpoints.lock().unwrap().remove(&id).is_some()
    }

    fn pending_count(&self, endpoint: EndpointId) -> usize {
        self.mailboxes
            .lock()
            .unwrap()
            .get(&endpoint)
            .map_or(0, Vec::len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_and_receive() {
        let ipc = InMemoryIpc::new();
        let ep = ipc.register_endpoint();
        let msg = Message {
            id: MessageId::new(),
            source: EndpointId::new(),
            target: ep,
            kind: "ping".into(),
            payload: vec![1, 2, 3],
            reply_to: None,
        };
        ipc.send(msg.clone()).unwrap();
        let received = ipc.receive(ep);
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].payload, vec![1, 2, 3]);
    }

    #[test]
    fn unknown_endpoint_errors() {
        let ipc = InMemoryIpc::new();
        let msg = Message {
            id: MessageId::new(),
            source: EndpointId::new(),
            target: EndpointId::new(),
            kind: "test".into(),
            payload: Vec::new(),
            reply_to: None,
        };
        assert!(ipc.send(msg).is_err());
    }

    #[test]
    fn register_and_unregister() {
        let ipc = InMemoryIpc::new();
        let ep = ipc.register_endpoint();
        assert!(ipc.unregister_endpoint(ep));
        assert!(!ipc.unregister_endpoint(ep));
    }

    #[test]
    fn pending_count() {
        let ipc = InMemoryIpc::new();
        let ep = ipc.register_endpoint();
        assert_eq!(ipc.pending_count(ep), 0);
        let msg = Message {
            id: MessageId::new(),
            source: EndpointId::new(),
            target: ep,
            kind: "test".into(),
            payload: Vec::new(),
            reply_to: None,
        };
        ipc.send(msg).unwrap();
        assert_eq!(ipc.pending_count(ep), 1);
    }
}
