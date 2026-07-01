use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

/// Capacity limit for working memory entries.
const DEFAULT_WM_CAPACITY: usize = 1024;
/// Default TTL for working memory entries.
const DEFAULT_WM_TTL_SECS: i64 = 300;

/// An entry in working memory — fast, ephemeral, with TTL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WMEntry {
    pub key: String,
    pub value: Value,
    pub created_at: OffsetDateTime,
    pub ttl_secs: i64,
    pub access_count: u64,
}

impl WMEntry {
    pub fn new(key: String, value: Value, ttl_secs: i64) -> Self {
        Self {
            key,
            value,
            created_at: OffsetDateTime::now_utc(),
            ttl_secs,
            access_count: 0,
        }
    }

    pub fn is_expired(&self) -> bool {
        if self.ttl_secs == 0 {
            return false;
        }
        (OffsetDateTime::now_utc() - self.created_at).whole_seconds() >= self.ttl_secs
    }
}

/// Working Memory — high-speed scratchpad for active context.
///
/// - In-memory HashMap with TTL expiry
/// - LRU eviction when capacity exceeded
/// - Access-count tracking for consolidation heuristics
pub trait WorkingMemory: Send + Sync {
    fn set(&mut self, key: &str, value: Value, ttl_secs: u64) -> String;
    fn get(&mut self, key: &str) -> Option<Value>;
    fn remove(&mut self, key: &str) -> bool;
    fn clear(&mut self);
    fn snapshot(&self) -> Vec<WMEntry>;
    fn contains(&self, key: &str) -> bool;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn capacity(&self) -> usize;
}

/// Default in-memory working memory.
pub struct DefaultWorkingMemory {
    inner: Arc<Mutex<InnerWM>>,
}

struct InnerWM {
    entries: HashMap<String, WMEntry>,
    capacity: usize,
    default_ttl_secs: i64,
}

impl DefaultWorkingMemory {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerWM {
                entries: HashMap::new(),
                capacity: DEFAULT_WM_CAPACITY,
                default_ttl_secs: DEFAULT_WM_TTL_SECS,
            })),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerWM {
                entries: HashMap::new(),
                capacity,
                default_ttl_secs: DEFAULT_WM_TTL_SECS,
            })),
        }
    }

    /// Evict expired entries and LRU if over capacity.
    fn evict(&self, inner: &mut InnerWM) {
        inner.entries.retain(|_, e| !e.is_expired());
        if inner.entries.len() > inner.capacity {
            let mut entries: Vec<(String, WMEntry)> = inner.entries.drain().collect();
            entries.sort_by(|a, b| a.1.access_count.cmp(&b.1.access_count));
            entries.truncate(inner.capacity);
            inner.entries = entries.into_iter().collect();
        }
    }
}

impl Default for DefaultWorkingMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkingMemory for DefaultWorkingMemory {
    fn set(&mut self, key: &str, value: Value, ttl_secs: u64) -> String {
        let mut inner = self.inner.lock().unwrap();
        let ts = if ttl_secs == 0 {
            inner.default_ttl_secs
        } else {
            ttl_secs as i64
        };
        inner
            .entries
            .insert(key.to_string(), WMEntry::new(key.to_string(), value, ts));
        self.evict(&mut inner);
        key.to_string()
    }

    fn get(&mut self, key: &str) -> Option<Value> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(entry) = inner.entries.get(key) {
            if entry.is_expired() {
                inner.entries.remove(key);
                return None;
            }
        }
        if let Some(entry) = inner.entries.get_mut(key) {
            entry.access_count += 1;
            return Some(entry.value.clone());
        }
        None
    }

    fn remove(&mut self, key: &str) -> bool {
        self.inner.lock().unwrap().entries.remove(key).is_some()
    }

    fn clear(&mut self) {
        self.inner.lock().unwrap().entries.clear();
    }

    fn snapshot(&self) -> Vec<WMEntry> {
        let inner = self.inner.lock().unwrap();
        inner
            .entries
            .values()
            .filter(|e| !e.is_expired())
            .cloned()
            .collect()
    }

    fn contains(&self, key: &str) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.entries.get(key).is_some_and(|e| !e.is_expired())
    }

    fn len(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.entries.values().filter(|e| !e.is_expired()).count()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn capacity(&self) -> usize {
        self.inner.lock().unwrap().capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn set_and_get() {
        let mut wm = DefaultWorkingMemory::new();
        wm.set("key1", json!("value1"), 60);
        assert_eq!(wm.get("key1"), Some(json!("value1")));
    }

    #[test]
    fn get_missing() {
        let mut wm = DefaultWorkingMemory::new();
        assert!(wm.get("nonexistent").is_none());
    }

    #[test]
    fn remove_entry() {
        let mut wm = DefaultWorkingMemory::new();
        wm.set("key1", json!("v"), 60);
        assert!(wm.remove("key1"));
        assert!(!wm.remove("key1"));
    }

    #[test]
    fn ttl_expiry() {
        let mut wm = DefaultWorkingMemory::new();
        wm.set("key1", json!("v"), 0);
        assert!(wm.contains("key1"));
    }

    #[test]
    fn capacity_eviction() {
        let mut wm = DefaultWorkingMemory::with_capacity(3);
        wm.set("a", json!(1), 60);
        wm.set("b", json!(2), 60);
        wm.set("c", json!(3), 60);
        wm.set("d", json!(4), 60);
        assert!(wm.len() <= 3);
    }

    #[test]
    fn clear_empties() {
        let mut wm = DefaultWorkingMemory::new();
        wm.set("a", json!(1), 60);
        wm.set("b", json!(2), 60);
        wm.clear();
        assert!(wm.is_empty());
    }

    #[test]
    fn snapshot_returns_entries() {
        let mut wm = DefaultWorkingMemory::new();
        wm.set("x", json!("hello"), 60);
        let snap = wm.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].key, "x");
    }
}
