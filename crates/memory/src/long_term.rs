use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use ulid::Ulid;

use tordex_core::event_store::{EventEnvelope, EventStore, SnapshotStore};
use tordex_core::object::{Object, ObjectManager};

/// An entry in long-term memory, backed by PostgreSQL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LTEntry {
    pub id: Ulid,
    pub kind: String,
    pub content: serde_json::Value,
    pub source_ids: Vec<String>,
    pub confidence: f64,
    pub created_at: OffsetDateTime,
    pub last_accessed_at: OffsetDateTime,
    pub access_count: u64,
    pub ttl: Option<time::Duration>,
}

/// Query for long-term memory retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LTQuery {
    pub kinds: Option<Vec<String>>,
    pub ids: Option<Vec<String>>,
    pub min_confidence: f64,
    pub created_after: Option<OffsetDateTime>,
    pub created_before: Option<OffsetDateTime>,
    pub limit: usize,
    pub offset: usize,
}

impl Default for LTQuery {
    fn default() -> Self {
        Self {
            kinds: None,
            ids: None,
            min_confidence: 0.0,
            created_after: None,
            created_before: None,
            limit: 100,
            offset: 0,
        }
    }
}

/// Long-Term Memory — persistent, PostgreSQL-backed canonical store.
///
/// Wraps the existing EventStore, SnapshotStore, and ObjectManager
/// behind a unified long-term memory interface with temporal query,
/// confidence scoring, and consolidation support.
pub trait LongTermMemory: Send + Sync {
    fn store(&mut self, entry: LTEntry) -> Result<Ulid, String>;
    fn store_batch(&mut self, entries: Vec<LTEntry>) -> Result<Vec<Ulid>, String>;
    fn retrieve(&self, query: &LTQuery) -> Result<Vec<LTEntry>, String>;
    fn get(&self, id: Ulid) -> Result<Option<LTEntry>, String>;
    fn update(&mut self, id: Ulid, content: serde_json::Value) -> Result<(), String>;
    fn forget(&mut self, id: Ulid) -> Result<(), String>;
    fn count(&self, kind: &str) -> Result<u64, String>;
    fn clear(&mut self) -> Result<(), String>;
}

/// Default LTM backed by existing PostgreSQL stores.
#[allow(dead_code)]
pub struct DefaultLongTermMemory {
    event_store: Box<dyn EventStore>,
    snapshot_store: Box<dyn SnapshotStore>,
    object_manager: Box<dyn ObjectManager>,
}

impl DefaultLongTermMemory {
    pub fn new(
        event_store: Box<dyn EventStore>,
        snapshot_store: Box<dyn SnapshotStore>,
        object_manager: Box<dyn ObjectManager>,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            object_manager,
        }
    }

    fn event_to_entry(event: &EventEnvelope) -> LTEntry {
        LTEntry {
            id: Ulid::new(),
            kind: format!("event:{}:{}", event.aggregate_type, event.event_type),
            content: event.data.clone(),
            source_ids: vec![event.id.to_string()],
            confidence: 1.0,
            created_at: event.timestamp,
            last_accessed_at: OffsetDateTime::now_utc(),
            access_count: 0,
            ttl: None,
        }
    }

    fn object_to_entry(obj: &Object) -> LTEntry {
        LTEntry {
            id: Ulid::new(),
            kind: format!("object:{}", obj.kind),
            content: serde_json::json!({
                "id": obj.id.to_string(),
                "label": obj.label,
                "data": serde_json::from_slice::<serde_json::Value>(&obj.data).ok(),
            }),
            source_ids: vec![obj.id.to_string()],
            confidence: 1.0,
            created_at: OffsetDateTime::from_unix_timestamp(obj.created_at / 1000).unwrap_or(OffsetDateTime::now_utc()),
            last_accessed_at: OffsetDateTime::now_utc(),
            access_count: 0,
            ttl: None,
        }
    }
}

impl LongTermMemory for DefaultLongTermMemory {
    fn store(&mut self, entry: LTEntry) -> Result<Ulid, String> {
        let event = EventEnvelope::new(
            entry.id.to_string(),
            "LongTermMemory",
            "EntryStored",
            1,
            serde_json::to_value(&entry).map_err(|e| e.to_string())?,
        );
        self.event_store
            .append(event)
            .map_err(|e| e.to_string())?;
        Ok(entry.id)
    }

    fn store_batch(&mut self, entries: Vec<LTEntry>) -> Result<Vec<Ulid>, String> {
        entries.into_iter().map(|e| self.store(e)).collect()
    }

    fn retrieve(&self, query: &LTQuery) -> Result<Vec<LTEntry>, String> {
        let raw = self
            .event_store
            .read_all("LongTermMemory")
            .map_err(|e| e.to_string())?;
        let mut entries: Vec<LTEntry> = raw
            .iter()
            .filter_map(|ev| {
                serde_json::from_value::<LTEntry>(ev.data.clone()).ok()
            })
            .filter(|e| e.confidence >= query.min_confidence)
            .filter(|e| {
                query
                    .kinds
                    .as_ref()
                    .map_or(true, |k| k.contains(&e.kind))
            })
            .collect();

        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let start = query.offset.min(entries.len());
        let end = (start + query.limit).min(entries.len());
        Ok(entries[start..end].to_vec())
    }

    fn get(&self, id: Ulid) -> Result<Option<LTEntry>, String> {
        let query = LTQuery {
            ids: Some(vec![id.to_string()]),
            limit: 1,
            ..Default::default()
        };
        let mut results = self.retrieve(&query)?;
        Ok(results.pop())
    }

    fn update(&mut self, id: Ulid, content: serde_json::Value) -> Result<(), String> {
        let entry = self.get(id)?.ok_or_else(|| format!("LTM entry {id} not found"))?;
        let updated = LTEntry {
            content,
            last_accessed_at: OffsetDateTime::now_utc(),
            ..entry
        };
        let event = EventEnvelope::new(
            id.to_string(),
            "LongTermMemory",
            "EntryUpdated",
            2,
            serde_json::to_value(&updated).map_err(|e| e.to_string())?,
        );
        self.event_store
            .append(event)
            .map_err(|e| e.to_string())
    }

    fn forget(&mut self, id: Ulid) -> Result<(), String> {
        let event = EventEnvelope::new(
            id.to_string(),
            "LongTermMemory",
            "EntryDeleted",
            3,
            serde_json::json!({"id": id.to_string()}),
        );
        self.event_store
            .append(event)
            .map_err(|e| e.to_string())
    }

    fn count(&self, kind: &str) -> Result<u64, String> {
        Ok(self.event_store.count(kind))
    }

    fn clear(&mut self) -> Result<(), String> {
        self.event_store
            .clear()
            .map_err(|e| e.to_string())
    }
}
