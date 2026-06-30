use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ulid::Ulid;

pub type ObjectId = Ulid;
pub type LinkId = Ulid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: ObjectId,
    pub kind: String,
    pub label: String,
    pub data: Vec<u8>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: LinkId,
    pub source_id: ObjectId,
    pub target_id: ObjectId,
    pub kind: String,
    pub created_at: i64,
}

pub trait ObjectManager: Send + Sync {
    fn create(&self, kind: &str, label: &str, data: &[u8]) -> ObjectId;
    fn read(&self, id: ObjectId) -> Option<Object>;
    fn update(&self, id: ObjectId, data: &[u8]) -> bool;
    fn delete(&self, id: ObjectId) -> bool;
    fn link(&self, source: ObjectId, target: ObjectId, kind: &str) -> LinkId;
    fn unlink(&self, id: LinkId) -> bool;
    fn links(&self, object: ObjectId) -> Vec<Link>;
    fn find_by_kind(&self, kind: &str) -> Vec<Object>;
    fn find_by_label(&self, label: &str) -> Vec<Object>;
}

pub struct InMemoryObjectManager {
    objects: Arc<Mutex<HashMap<ObjectId, Object>>>,
    links: Arc<Mutex<HashMap<LinkId, Link>>>,
}

impl InMemoryObjectManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            objects: Arc::new(Mutex::new(HashMap::new())),
            links: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryObjectManager {
    fn default() -> Self {
        Self::new()
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

impl ObjectManager for InMemoryObjectManager {
    fn create(&self, kind: &str, label: &str, data: &[u8]) -> ObjectId {
        let id = ObjectId::new();
        let now = now_millis();
        let object = Object {
            id,
            kind: kind.to_string(),
            label: label.to_string(),
            data: data.to_vec(),
            created_at: now,
            updated_at: now,
        };
        self.objects.lock().unwrap().insert(id, object);
        id
    }

    fn read(&self, id: ObjectId) -> Option<Object> {
        self.objects.lock().unwrap().get(&id).cloned()
    }

    fn update(&self, id: ObjectId, data: &[u8]) -> bool {
        let mut objects = self.objects.lock().unwrap();
        if let Some(obj) = objects.get_mut(&id) {
            obj.data = data.to_vec();
            obj.updated_at = now_millis();
            true
        } else {
            false
        }
    }

    fn delete(&self, id: ObjectId) -> bool {
        self.objects.lock().unwrap().remove(&id).is_some()
    }

    fn link(&self, source: ObjectId, target: ObjectId, kind: &str) -> LinkId {
        let id = LinkId::new();
        let link = Link {
            id,
            source_id: source,
            target_id: target,
            kind: kind.to_string(),
            created_at: now_millis(),
        };
        self.links.lock().unwrap().insert(id, link);
        id
    }

    fn unlink(&self, id: LinkId) -> bool {
        self.links.lock().unwrap().remove(&id).is_some()
    }

    fn links(&self, object: ObjectId) -> Vec<Link> {
        self.links
            .lock()
            .unwrap()
            .values()
            .filter(|l| l.source_id == object || l.target_id == object)
            .cloned()
            .collect()
    }

    fn find_by_kind(&self, kind: &str) -> Vec<Object> {
        self.objects
            .lock()
            .unwrap()
            .values()
            .filter(|o| o.kind == kind)
            .cloned()
            .collect()
    }

    fn find_by_label(&self, label: &str) -> Vec<Object> {
        self.objects
            .lock()
            .unwrap()
            .values()
            .filter(|o| o.label == label)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_read() {
        let om = InMemoryObjectManager::new();
        let id = om.create("document", "test", b"content");
        let obj = om.read(id).unwrap();
        assert_eq!(obj.kind, "document");
        assert_eq!(obj.label, "test");
    }

    #[test]
    fn update_existing() {
        let om = InMemoryObjectManager::new();
        let id = om.create("doc", "original", b"old");
        assert!(om.update(id, b"new"));
        let obj = om.read(id).unwrap();
        assert_eq!(obj.data, b"new");
    }

    #[test]
    fn update_missing() {
        let om = InMemoryObjectManager::new();
        assert!(!om.update(ObjectId::new(), b"data"));
    }

    #[test]
    fn delete() {
        let om = InMemoryObjectManager::new();
        let id = om.create("temp", "delete-me", b"");
        assert!(om.delete(id));
        assert!(om.read(id).is_none());
    }

    #[test]
    fn create_link_and_find_links() {
        let om = InMemoryObjectManager::new();
        let a = om.create("node", "A", b"");
        let b = om.create("node", "B", b"");
        om.link(a, b, "connected_to");
        let links = om.links(a);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].kind, "connected_to");
    }

    #[test]
    fn find_by_kind() {
        let om = InMemoryObjectManager::new();
        om.create("user", "alice", b"");
        om.create("user", "bob", b"");
        om.create("config", "settings", b"");
        assert_eq!(om.find_by_kind("user").len(), 2);
        assert_eq!(om.find_by_kind("config").len(), 1);
    }

    #[test]
    fn find_by_label() {
        let om = InMemoryObjectManager::new();
        om.create("type", "unique-name", b"");
        assert_eq!(om.find_by_label("unique-name").len(), 1);
        assert_eq!(om.find_by_label("nonexistent").len(), 0);
    }
}
