use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: Vec<u8>,
    pub content_type: Option<String>,
    pub created_at: i64,
}

pub trait StorageManager: Send + Sync {
    fn store(&self, key: &str, value: &[u8], content_type: Option<&str>);
    fn load(&self, key: &str) -> Option<Entry>;
    fn delete(&self, key: &str) -> bool;
    fn list(&self, prefix: &str) -> Vec<String>;
    fn exists(&self, key: &str) -> bool;
}

pub struct InMemoryStorage {
    data: Arc<Mutex<HashMap<String, Entry>>>,
}

impl InMemoryStorage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageManager for InMemoryStorage {
    fn store(&self, key: &str, value: &[u8], content_type: Option<&str>) {
        let entry = Entry {
            key: key.to_string(),
            value: value.to_vec(),
            content_type: content_type.map(String::from),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        };
        self.data.lock().unwrap().insert(key.to_string(), entry);
    }

    fn load(&self, key: &str) -> Option<Entry> {
        self.data.lock().unwrap().get(key).cloned()
    }

    fn delete(&self, key: &str) -> bool {
        self.data.lock().unwrap().remove(key).is_some()
    }

    fn list(&self, prefix: &str) -> Vec<String> {
        self.data
            .lock()
            .unwrap()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    fn exists(&self, key: &str) -> bool {
        self.data.lock().unwrap().contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_load() {
        let store = InMemoryStorage::new();
        store.store("test/key", b"hello world", Some("text/plain"));
        let entry = store.load("test/key").unwrap();
        assert_eq!(entry.value, b"hello world");
        assert_eq!(entry.content_type, Some("text/plain".into()));
    }

    #[test]
    fn load_missing() {
        let store = InMemoryStorage::new();
        assert!(store.load("nonexistent").is_none());
    }

    #[test]
    fn delete_entry() {
        let store = InMemoryStorage::new();
        store.store("a", b"1", None);
        assert!(store.delete("a"));
        assert!(!store.delete("a"));
    }

    #[test]
    fn list_with_prefix() {
        let store = InMemoryStorage::new();
        store.store("sessions/a", b"1", None);
        store.store("sessions/b", b"2", None);
        store.store("other/c", b"3", None);
        let keys = store.list("sessions/");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn exists_check() {
        let store = InMemoryStorage::new();
        store.store("x", b"1", None);
        assert!(store.exists("x"));
        assert!(!store.exists("y"));
    }
}
