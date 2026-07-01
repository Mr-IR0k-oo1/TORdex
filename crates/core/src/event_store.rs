//! Event Store — append-only log for event-sourced kernel.
//!
//! Every state change is recorded as an event. State is derived by replaying
//! events. Supports snapshots for efficient recovery and rollback.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use ulid::Ulid;

// ─── EventEnvelope ───────────────────────────────────────────────────────────

/// An envelope wrapping every event in the system.
///
/// - `id`: globally unique, time-ordered (ULID)
/// - `aggregate_id`: the object this event belongs to
/// - `aggregate_type`: kind of aggregate ("Entity", "Observation", etc.)
/// - `event_type`: what happened ("Created", "Updated", "Deleted", etc.)
/// - `version`: sequence number on the aggregate
/// - `data`: the event payload as arbitrary JSON
/// - `metadata`: causation/correlation IDs, actor info, etc.
/// - `timestamp`: when the event was recorded
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: Ulid,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub event_type: String,
    pub version: u64,
    pub data: serde_json::Value,
    pub metadata: HashMap<String, String>,
    pub timestamp: OffsetDateTime,
}

impl EventEnvelope {
    #[must_use]
    pub fn new(
        aggregate_id: String,
        aggregate_type: &str,
        event_type: &str,
        version: u64,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: Ulid::new(),
            aggregate_id,
            aggregate_type: aggregate_type.to_string(),
            event_type: event_type.to_string(),
            version,
            data,
            metadata: HashMap::new(),
            timestamp: OffsetDateTime::now_utc(),
        }
    }

    /// Attach causation metadata.
    #[must_use]
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

// ─── SystemEvent ─────────────────────────────────────────────────────────────

/// Strongly-typed payload variants for all kernel events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum SystemEvent {
    // ── Entity ────────────────────────────────────────────────────────────
    EntityCreated {
        id: String,
        kind: String,
        name: String,
        attributes: HashMap<String, String>,
        first_seen: OffsetDateTime,
    },
    EntityUpdated {
        id: String,
        kind: Option<String>,
        name: Option<String>,
        attributes: Option<HashMap<String, String>>,
    },
    EntityDeleted { id: String },

    // ── Observation ───────────────────────────────────────────────────────
    ObservationRecorded {
        id: String,
        kind: String,
        data: Vec<u8>,
        content_type: Option<String>,
        source: String,
        observed_at: OffsetDateTime,
    },

    // ── Artifact ──────────────────────────────────────────────────────────
    ArtifactStored {
        id: String,
        session_id: String,
        kind: String,
        content_type: Option<String>,
        byte_count: u64,
        sha256: String,
        storage_path: String,
    },
    ArtifactDeleted { id: String },

    // ── Evidence ──────────────────────────────────────────────────────────
    EvidenceExtracted {
        id: String,
        artifact_id: String,
        kind: String,
        value: serde_json::Value,
        confidence: f64,
    },

    // ── Relationship ──────────────────────────────────────────────────────
    RelationshipEstablished {
        id: String,
        kind: String,
        source_type: String,
        source_id: String,
        target_type: String,
        target_id: String,
    },
    RelationshipDeleted { id: String },

    // ── Knowledge ─────────────────────────────────────────────────────────
    KnowledgeProduced {
        id: String,
        kind: String,
        content: serde_json::Value,
        confidence: f64,
        source_ids: Vec<String>,
    },

    // ── Finding ───────────────────────────────────────────────────────────
    FindingCreated {
        id: String,
        investigation_id: String,
        kind: String,
        title: String,
        description: String,
        severity: String,
        confidence: f64,
        source_ids: Vec<String>,
    },
    FindingUpdated {
        id: String,
        title: Option<String>,
        description: Option<String>,
        severity: Option<String>,
        confidence: Option<f64>,
    },

    // ── Decision ──────────────────────────────────────────────────────────
    DecisionMade {
        id: String,
        finding_ids: Vec<String>,
        kind: String,
        rationale: String,
        actor: String,
        status: String,
    },
    DecisionExecuted { id: String },

    // ── Service ───────────────────────────────────────────────────────────
    ServiceRegistered {
        id: String,
        name: String,
        kind: String,
        locator: String,
        status: String,
        version: Option<String>,
    },
    ServiceUpdated {
        id: String,
        name: Option<String>,
        status: Option<String>,
        version: Option<String>,
    },
    ServiceDeleted { id: String },

    // ── Investigation ─────────────────────────────────────────────────────
    InvestigationOpened {
        id: String,
        title: String,
        description: String,
        owner: String,
        tags: Vec<String>,
    },
    InvestigationClosed { id: String },
    InvestigationArchived { id: String },

    // ── Timeline ──────────────────────────────────────────────────────────
    TimelineEntryAdded {
        id: String,
        investigation_id: String,
        timestamp: OffsetDateTime,
        kind: String,
        source_id: String,
        summary: String,
    },

    // ── Agent ──────────────────────────────────────────────────────────────
    AgentStarted {
        id: String,
        name: String,
        kind: String,
        version: String,
    },
    AgentStopped {
        id: String,
        reason: String,
    },
    AgentHeartbeat {
        id: String,
        status: String,
        timestamp: OffsetDateTime,
    },
}

impl SystemEvent {
    /// The aggregate type string for this event.
    #[must_use]
    pub fn aggregate_type(&self) -> &str {
        match self {
            Self::EntityCreated { .. }
            | Self::EntityUpdated { .. }
            | Self::EntityDeleted { .. } => "Entity",
            Self::ObservationRecorded { .. } => "Observation",
            Self::ArtifactStored { .. } | Self::ArtifactDeleted { .. } => "Artifact",
            Self::EvidenceExtracted { .. } => "Evidence",
            Self::RelationshipEstablished { .. } | Self::RelationshipDeleted { .. } => {
                "Relationship"
            }
            Self::KnowledgeProduced { .. } => "Knowledge",
            Self::FindingCreated { .. } | Self::FindingUpdated { .. } => "Finding",
            Self::DecisionMade { .. } | Self::DecisionExecuted { .. } => "Decision",
            Self::ServiceRegistered { .. }
            | Self::ServiceUpdated { .. }
            | Self::ServiceDeleted { .. } => "Service",
            Self::InvestigationOpened { .. }
            | Self::InvestigationClosed { .. }
            | Self::InvestigationArchived { .. } => "Investigation",
            Self::TimelineEntryAdded { .. } => "Timeline",
            Self::AgentStarted { .. }
            | Self::AgentStopped { .. }
            | Self::AgentHeartbeat { .. } => "Agent",
        }
    }

    /// The event type string for this variant.
    #[must_use]
    pub fn event_type(&self) -> &str {
        match self {
            Self::EntityCreated { .. } => "Created",
            Self::EntityUpdated { .. } => "Updated",
            Self::EntityDeleted { .. } => "Deleted",
            Self::ObservationRecorded { .. } => "Recorded",
            Self::ArtifactStored { .. } => "Stored",
            Self::ArtifactDeleted { .. } => "Deleted",
            Self::EvidenceExtracted { .. } => "Extracted",
            Self::RelationshipEstablished { .. } => "Established",
            Self::RelationshipDeleted { .. } => "Deleted",
            Self::KnowledgeProduced { .. } => "Produced",
            Self::FindingCreated { .. } => "Created",
            Self::FindingUpdated { .. } => "Updated",
            Self::DecisionMade { .. } => "Made",
            Self::DecisionExecuted { .. } => "Executed",
            Self::ServiceRegistered { .. } => "Registered",
            Self::ServiceUpdated { .. } => "Updated",
            Self::ServiceDeleted { .. } => "Deleted",
            Self::InvestigationOpened { .. } => "Opened",
            Self::InvestigationClosed { .. } => "Closed",
            Self::InvestigationArchived { .. } => "Archived",
            Self::TimelineEntryAdded { .. } => "EntryAdded",
            Self::AgentStarted { .. } => "Started",
            Self::AgentStopped { .. } => "Stopped",
            Self::AgentHeartbeat { .. } => "Heartbeat",
        }
    }

    /// The aggregate ID for this event.
    #[must_use]
    pub fn aggregate_id(&self) -> &str {
        match self {
            Self::EntityCreated { id, .. }
            | Self::EntityUpdated { id, .. }
            | Self::EntityDeleted { id, .. } => id,
            Self::ObservationRecorded { id, .. } => id,
            Self::ArtifactStored { id, .. } | Self::ArtifactDeleted { id, .. } => id,
            Self::EvidenceExtracted { id, .. } => id,
            Self::RelationshipEstablished { id, .. }
            | Self::RelationshipDeleted { id, .. } => id,
            Self::KnowledgeProduced { id, .. } => id,
            Self::FindingCreated { id, .. } | Self::FindingUpdated { id, .. } => id,
            Self::DecisionMade { id, .. } | Self::DecisionExecuted { id, .. } => id,
            Self::ServiceRegistered { id, .. }
            | Self::ServiceUpdated { id, .. }
            | Self::ServiceDeleted { id, .. } => id,
            Self::InvestigationOpened { id, .. }
            | Self::InvestigationClosed { id, .. }
            | Self::InvestigationArchived { id, .. } => id,
            Self::TimelineEntryAdded { id, .. } => id,
            Self::AgentStarted { id, .. }
            | Self::AgentStopped { id, .. }
            | Self::AgentHeartbeat { id, .. } => id,
        }
    }

    /// Pack into an `EventEnvelope`.
    #[must_use]
    pub fn into_envelope(self, version: u64) -> EventEnvelope {
        let id = self.aggregate_id().to_string();
        let agg_type = self.aggregate_type().to_string();
        let evt_type = self.event_type().to_string();
        let data = serde_json::to_value(&self).expect("system event is always serializable");
        EventEnvelope {
            id: Ulid::new(),
            aggregate_id: id,
            aggregate_type: agg_type,
            event_type: evt_type,
            version,
            data,
            metadata: HashMap::new(),
            timestamp: OffsetDateTime::now_utc(),
        }
    }
}

// ─── Event Store ─────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum EventStoreError {
    #[error("event store error: {0}")]
    Storage(String),
    #[error("version conflict for aggregate {aggregate_id}: expected {expected}, got {got}")]
    VersionConflict {
        aggregate_id: String,
        expected: u64,
        got: u64,
    },
}

/// Append-only event store.
///
/// Events are immutable once written. Reading by aggregate returns all events
/// for that aggregate in order. Reading since a given ID enables catch-up
/// subscriptions and incremental replay.
pub trait EventStore: Send + Sync {
    /// Append an event. Returns an error if the version does not follow the
    /// latest version for this aggregate (optimistic concurrency).
    fn append(&self, event: EventEnvelope) -> Result<(), EventStoreError>;

    /// Read all events for an aggregate, ordered by version.
    fn read_events(&self, aggregate_id: &str) -> Result<Vec<EventEnvelope>, EventStoreError>;

    /// Read all events of a given aggregate type.
    fn read_all(&self, aggregate_type: &str) -> Result<Vec<EventEnvelope>, EventStoreError>;

    /// Read all events after a given ULID (for incremental replay).
    fn read_since(&self, since_id: Ulid) -> Result<Vec<EventEnvelope>, EventStoreError>;

    /// Read all events after a given aggregate version (for catch-up).
    fn read_since_version(
        &self,
        aggregate_id: &str,
        since_version: u64,
    ) -> Result<Vec<EventEnvelope>, EventStoreError>;

    /// Latest version for an aggregate (0 if none).
    fn latest_version(&self, aggregate_id: &str) -> u64;

    /// Total event count for an aggregate type.
    fn count(&self, aggregate_type: &str) -> u64;

    /// Total events across all aggregates.
    fn total_count(&self) -> u64;

    /// Clear all events (for testing).
    fn clear(&self) -> Result<(), EventStoreError>;
}

// ─── InMemoryEventStore ──────────────────────────────────────────────────────

struct Inner {
    events: Vec<EventEnvelope>,
    by_aggregate: HashMap<String, Vec<usize>>,
    by_type: HashMap<String, Vec<usize>>,
}

/// In-memory implementation of EventStore. Thread-safe via Mutex.
pub struct InMemoryEventStore {
    inner: Arc<Mutex<Inner>>,
}

impl InMemoryEventStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                events: Vec::new(),
                by_aggregate: HashMap::new(),
                by_type: HashMap::new(),
            })),
        }
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore for InMemoryEventStore {
    fn append(&self, event: EventEnvelope) -> Result<(), EventStoreError> {
        let mut inner = self.inner.lock().unwrap();

        // Version check
        let agg_id = event.aggregate_id.clone();
        let current = inner
            .by_aggregate
            .get(&agg_id)
            .map(|indices| indices.len() as u64)
            .unwrap_or(0);

        if event.version != current + 1 {
            return Err(EventStoreError::VersionConflict {
                aggregate_id: agg_id,
                expected: current + 1,
                got: event.version,
            });
        }

        let agg_type = event.aggregate_type.clone();
        let idx = inner.events.len();
        inner.events.push(event);
        inner
            .by_aggregate
            .entry(agg_id)
            .or_default()
            .push(idx);
        inner.by_type.entry(agg_type).or_default().push(idx);
        Ok(())
    }

    fn read_events(&self, aggregate_id: &str) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let inner = self.inner.lock().unwrap();
        let indices = match inner.by_aggregate.get(aggregate_id) {
            Some(v) => v,
            None => return Ok(Vec::new()),
        };
        let events: Vec<EventEnvelope> = indices
            .iter()
            .map(|&i| inner.events[i].clone())
            .collect();
        Ok(events)
    }

    fn read_all(&self, aggregate_type: &str) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let inner = self.inner.lock().unwrap();
        let indices = match inner.by_type.get(aggregate_type) {
            Some(v) => v,
            None => return Ok(Vec::new()),
        };
        let events: Vec<EventEnvelope> = indices
            .iter()
            .map(|&i| inner.events[i].clone())
            .collect();
        Ok(events)
    }

    fn read_since(&self, since_id: Ulid) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let inner = self.inner.lock().unwrap();
        let events: Vec<EventEnvelope> = inner
            .events
            .iter()
            .filter(|e| e.id > since_id)
            .cloned()
            .collect();
        Ok(events)
    }

    fn read_since_version(
        &self,
        aggregate_id: &str,
        since_version: u64,
    ) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let inner = self.inner.lock().unwrap();
        let indices = match inner.by_aggregate.get(aggregate_id) {
            Some(v) => v,
            None => return Ok(Vec::new()),
        };
        let events: Vec<EventEnvelope> = indices
            .iter()
            .filter_map(|&i| {
                let e = &inner.events[i];
                if e.version > since_version {
                    Some(e.clone())
                } else {
                    None
                }
            })
            .collect();
        Ok(events)
    }

    fn latest_version(&self, aggregate_id: &str) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner
            .by_aggregate
            .get(aggregate_id)
            .map(|v| v.len() as u64)
            .unwrap_or(0)
    }

    fn count(&self, aggregate_type: &str) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner
            .by_type
            .get(aggregate_type)
            .map(|v| v.len() as u64)
            .unwrap_or(0)
    }

    fn total_count(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner.events.len() as u64
    }

    fn clear(&self) -> Result<(), EventStoreError> {
        let mut inner = self.inner.lock().unwrap();
        inner.events.clear();
        inner.by_aggregate.clear();
        inner.by_type.clear();
        Ok(())
    }
}

// ─── Snapshot Store ──────────────────────────────────────────────────────────

/// A materialized snapshot of an aggregate at a given version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub version: u64,
    pub state: Vec<u8>,
    pub timestamp: OffsetDateTime,
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("snapshot not found: {0}")]
    NotFound(String),
    #[error("snapshot serialization error: {0}")]
    Serialization(String),
}

/// Stores materialized snapshots for efficient replay recovery.
pub trait SnapshotStore: Send + Sync {
    fn save(&self, snapshot: &Snapshot) -> Result<(), SnapshotError>;
    fn load(&self, aggregate_id: &str) -> Result<Option<Snapshot>, SnapshotError>;
    fn load_latest(&self, aggregate_type: &str) -> Result<Option<Snapshot>, SnapshotError>;
    fn delete(&self, aggregate_id: &str) -> Result<(), SnapshotError>;
}

/// In-memory snapshot store backed by a HashMap.
pub struct InMemorySnapshotStore {
    inner: Arc<Mutex<HashMap<String, Snapshot>>>,
}

impl InMemorySnapshotStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for InMemorySnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotStore for InMemorySnapshotStore {
    fn save(&self, snapshot: &Snapshot) -> Result<(), SnapshotError> {
        self.inner
            .lock()
            .unwrap()
            .insert(snapshot.aggregate_id.clone(), snapshot.clone());
        Ok(())
    }

    fn load(&self, aggregate_id: &str) -> Result<Option<Snapshot>, SnapshotError> {
        Ok(self.inner.lock().unwrap().get(aggregate_id).cloned())
    }

    fn load_latest(&self, aggregate_type: &str) -> Result<Option<Snapshot>, SnapshotError> {
        let inner = self.inner.lock().unwrap();
        Ok(inner
            .values()
            .filter(|s| s.aggregate_type == aggregate_type)
            .max_by_key(|s| s.version)
            .cloned())
    }

    fn delete(&self, aggregate_id: &str) -> Result<(), SnapshotError> {
        self.inner
            .lock()
            .unwrap()
            .remove(aggregate_id)
            .ok_or_else(|| SnapshotError::NotFound(aggregate_id.to_string()))?;
        Ok(())
    }
}

// ─── Aggregate Trait ─────────────────────────────────────────────────────────

/// An aggregate is a domain object whose state is derived from an event stream.
///
/// Implement this for any type that should be recoverable from the event store.
pub trait Aggregate: Sized {
    /// The event type this aggregate consumes.
    type Event;

    /// Apply an event to mutate this aggregate's state.
    fn apply(&mut self, event: &Self::Event);

    /// Return the initial (empty) state.
    fn initial() -> Self;

    /// Replay a sequence of events to reconstruct this aggregate.
    fn replay(events: &[Self::Event]) -> Self {
        let mut agg = Self::initial();
        for event in events {
            agg.apply(event);
        }
        agg
    }
}

// ─── Projector ───────────────────────────────────────────────────────────────

/// A projector consumes events and updates a read model.
///
/// Unlike aggregates which reconstruct a single object, projectors maintain
/// arbitrary read models (e.g., VIFS, search indices, materialized views).
pub trait Projector: Send + Sync {
    /// Process an event and update the read model.
    fn project(&self, event: &EventEnvelope) -> Result<(), EventStoreError>;

    /// Identifier for this projector (for dedup).
    fn name(&self) -> &str;
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_event(agg_id: &str, agg_type: &str, evt_type: &str, version: u64) -> EventEnvelope {
        EventEnvelope::new(
            agg_id.to_string(),
            agg_type,
            evt_type,
            version,
            json!({"test": true}),
        )
    }

    // ── EventEnvelope ─────────────────────────────────────────────────────

    #[test]
    fn envelope_auto_generates_ulid() {
        let e1 = EventEnvelope::new("a".into(), "Test", "Created", 1, json!({}));
        let e2 = EventEnvelope::new("a".into(), "Test", "Created", 1, json!({}));
        assert_ne!(e1.id, e2.id);
    }

    #[test]
    fn envelope_metadata_chain() {
        let e = EventEnvelope::new("a".into(), "Test", "Created", 1, json!({}))
            .with_metadata("cause", "cmd_1")
            .with_metadata("actor", "system");
        assert_eq!(e.metadata.get("cause").unwrap(), "cmd_1");
        assert_eq!(e.metadata.get("actor").unwrap(), "system");
    }

    // ── SystemEvent ───────────────────────────────────────────────────────

    #[test]
    fn system_event_aggregate_type() {
        let ev = SystemEvent::EntityCreated {
            id: "e1".into(),
            kind: "ip".into(),
            name: "8.8.8.8".into(),
            attributes: HashMap::new(),
            first_seen: OffsetDateTime::now_utc(),
        };
        assert_eq!(ev.aggregate_type(), "Entity");
        assert_eq!(ev.event_type(), "Created");
        assert_eq!(ev.aggregate_id(), "e1");
    }

    #[test]
    fn system_event_into_envelope() {
        let ev = SystemEvent::EntityCreated {
            id: "e1".into(),
            kind: "ip".into(),
            name: "8.8.8.8".into(),
            attributes: HashMap::new(),
            first_seen: OffsetDateTime::now_utc(),
        };
        let env = ev.into_envelope(1);
        assert_eq!(env.aggregate_id, "e1");
        assert_eq!(env.aggregate_type, "Entity");
        assert_eq!(env.event_type, "Created");
        assert_eq!(env.version, 1);
    }

    // ── InMemoryEventStore ────────────────────────────────────────────────

    #[test]
    fn append_and_read_events() {
        let store = InMemoryEventStore::new();
        store.append(make_event("agg1", "Test", "Created", 1)).unwrap();
        store.append(make_event("agg1", "Test", "Updated", 2)).unwrap();

        let events = store.read_events("agg1").unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].version, 1);
        assert_eq!(events[1].version, 2);
    }

    #[test]
    fn append_version_conflict() {
        let store = InMemoryEventStore::new();
        store.append(make_event("agg1", "Test", "Created", 1)).unwrap();
        let err = store
            .append(make_event("agg1", "Test", "Updated", 3))
            .unwrap_err();
        assert!(matches!(
            err,
            EventStoreError::VersionConflict {
                expected: 2,
                got: 3,
                ..
            }
        ));
    }

    #[test]
    fn read_all_by_type() {
        let store = InMemoryEventStore::new();
        store
            .append(make_event("a1", "Entity", "Created", 1))
            .unwrap();
        store
            .append(make_event("a2", "Entity", "Created", 1))
            .unwrap();
        store
            .append(make_event("b1", "Observation", "Recorded", 1))
            .unwrap();

        assert_eq!(store.read_all("Entity").unwrap().len(), 2);
        assert_eq!(store.read_all("Observation").unwrap().len(), 1);
        assert_eq!(store.read_all("Unknown").unwrap().len(), 0);
    }

    #[test]
    fn read_since_filters_by_ulid() {
        let store = InMemoryEventStore::new();
        store
            .append(make_event("a1", "Test", "Created", 1))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let e2 = make_event("a1", "Test", "Updated", 2);
        let id2 = e2.id;
        store.append(e2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        store
            .append(make_event("b1", "Test", "Created", 1))
            .unwrap();

        let since = store.read_since(id2).unwrap();
        assert_eq!(since.len(), 1);
    }

    #[test]
    fn read_since_version() {
        let store = InMemoryEventStore::new();
        store
            .append(make_event("agg1", "Test", "Created", 1))
            .unwrap();
        store
            .append(make_event("agg1", "Test", "Updated", 2))
            .unwrap();
        store
            .append(make_event("agg1", "Test", "Updated", 3))
            .unwrap();

        let since = store
            .read_since_version("agg1", 1)
            .unwrap();
        assert_eq!(since.len(), 2);
        assert_eq!(since[0].version, 2);
        assert_eq!(since[1].version, 3);
    }

    #[test]
    fn latest_version_tracking() {
        let store = InMemoryEventStore::new();
        assert_eq!(store.latest_version("agg1"), 0);
        store
            .append(make_event("agg1", "Test", "Created", 1))
            .unwrap();
        assert_eq!(store.latest_version("agg1"), 1);
        store
            .append(make_event("agg1", "Test", "Updated", 2))
            .unwrap();
        assert_eq!(store.latest_version("agg1"), 2);
    }

    #[test]
    fn count_and_total() {
        let store = InMemoryEventStore::new();
        store
            .append(make_event("a1", "Entity", "Created", 1))
            .unwrap();
        store
            .append(make_event("a2", "Entity", "Created", 1))
            .unwrap();
        store
            .append(make_event("b1", "Test", "Created", 1))
            .unwrap();
        assert_eq!(store.count("Entity"), 2);
        assert_eq!(store.total_count(), 3);
    }

    #[test]
    fn clear_resets_everything() {
        let store = InMemoryEventStore::new();
        store
            .append(make_event("a1", "Test", "Created", 1))
            .unwrap();
        store.clear().unwrap();
        assert_eq!(store.total_count(), 0);
        assert_eq!(store.latest_version("a1"), 0);
    }

    #[test]
    fn read_events_nonexistent_aggregate() {
        let store = InMemoryEventStore::new();
        let events = store.read_events("nonexistent").unwrap();
        assert!(events.is_empty());
    }

    // ── Snapshot Store ────────────────────────────────────────────────────

    #[test]
    fn snapshot_save_and_load() {
        let store = InMemorySnapshotStore::new();
        let snap = Snapshot {
            aggregate_id: "agg1".into(),
            aggregate_type: "Entity".into(),
            version: 5,
            state: b"entity state".to_vec(),
            timestamp: OffsetDateTime::now_utc(),
        };
        store.save(&snap).unwrap();
        let loaded = store.load("agg1").unwrap().unwrap();
        assert_eq!(loaded.version, 5);
        assert_eq!(loaded.state, b"entity state");
    }

    #[test]
    fn snapshot_load_missing() {
        let store = InMemorySnapshotStore::new();
        assert!(store.load("nonexistent").unwrap().is_none());
    }

    #[test]
    fn snapshot_load_latest_by_type() {
        let store = InMemorySnapshotStore::new();
        store
            .save(&Snapshot {
                aggregate_id: "a1".into(),
                aggregate_type: "Entity".into(),
                version: 3,
                state: vec![],
                timestamp: OffsetDateTime::now_utc(),
            })
            .unwrap();
        store
            .save(&Snapshot {
                aggregate_id: "a2".into(),
                aggregate_type: "Entity".into(),
                version: 7,
                state: vec![],
                timestamp: OffsetDateTime::now_utc(),
            })
            .unwrap();
        let latest = store.load_latest("Entity").unwrap().unwrap();
        assert_eq!(latest.version, 7);
    }

    #[test]
    fn snapshot_delete() {
        let store = InMemorySnapshotStore::new();
        store
            .save(&Snapshot {
                aggregate_id: "agg1".into(),
                aggregate_type: "Test".into(),
                version: 1,
                state: vec![],
                timestamp: OffsetDateTime::now_utc(),
            })
            .unwrap();
        store.delete("agg1").unwrap();
        assert!(store.load("agg1").unwrap().is_none());
    }

    // ── Aggregate Replay ──────────────────────────────────────────────────

    /// Simple test aggregate: a counter that increments on "Incremented" events.
    #[derive(Debug, Clone, PartialEq)]
    struct Counter {
        value: i64,
    }

    impl Aggregate for Counter {
        type Event = String;

        fn initial() -> Self {
            Self { value: 0 }
        }

        fn apply(&mut self, event: &Self::Event) {
            if event == "Incremented" {
                self.value += 1;
            }
        }
    }

    #[test]
    fn aggregate_initial_state() {
        let counter = Counter::initial();
        assert_eq!(counter.value, 0);
    }

    #[test]
    fn aggregate_apply_single_event() {
        let mut counter = Counter::initial();
        counter.apply(&"Incremented".to_string());
        assert_eq!(counter.value, 1);
    }

    #[test]
    fn aggregate_replay_events() {
        let events = vec![
            "Incremented".to_string(),
            "Incremented".to_string(),
            "Incremented".to_string(),
        ];
        let counter = Counter::replay(&events);
        assert_eq!(counter.value, 3);
    }

    #[test]
    fn aggregate_replay_empty() {
        let counter = Counter::replay(&[]);
        assert_eq!(counter.value, 0);
    }
}
