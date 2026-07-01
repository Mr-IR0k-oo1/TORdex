use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use time::OffsetDateTime;
use ulid::Ulid;

use tordex_core::event_store::{
    EventEnvelope, EventStore, EventStoreError, Snapshot, SnapshotError, SnapshotStore,
};
use tordex_core::object::{Link, LinkId, Object, ObjectId, ObjectManager};
use tordex_core::storage::{Entry, StorageManager};
use tordex_types::ArtifactStore;

// ─── PgEventStore ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PgEventStore {
    pool: PgPool,
}

impl PgEventStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl EventStore for PgEventStore {
    fn append(&self, event: EventEnvelope) -> Result<(), EventStoreError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let latest: Option<(i64,)> = sqlx::query_as(
                    "SELECT MAX(version) FROM kernel_events WHERE aggregate_id = $1",
                )
                .bind(&event.aggregate_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| EventStoreError::Storage(e.to_string()))?;

                let current = latest.unwrap_or((0,)).0 as u64;
                if event.version != current + 1 {
                    return Err(EventStoreError::VersionConflict {
                        aggregate_id: event.aggregate_id.clone(),
                        expected: current + 1,
                        got: event.version,
                    });
                }

                sqlx::query(
                    r#"INSERT INTO kernel_events
                       (id, aggregate_id, aggregate_type, event_type, version, data, metadata, timestamp)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
                )
                .bind(event.id.to_string())
                .bind(&event.aggregate_id)
                .bind(&event.aggregate_type)
                .bind(&event.event_type)
                .bind(event.version as i64)
                .bind(&event.data)
                .bind(serde_json::to_value(&event.metadata).unwrap_or_default())
                .bind(event.timestamp)
                .execute(&pool)
                .await
                .map_err(|e| EventStoreError::Storage(e.to_string()))?;

                Ok(())
            })
        })
    }

    fn read_events(&self, aggregate_id: &str) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let pool = self.pool.clone();
        let agg_id = aggregate_id.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let rows: Vec<PgEventRow> = sqlx::query_as(
                    "SELECT id, aggregate_id, aggregate_type, event_type, version, data, metadata, timestamp
                     FROM kernel_events WHERE aggregate_id = $1 ORDER BY version",
                )
                .bind(&agg_id)
                .fetch_all(&pool)
                .await
                .map_err(|e| EventStoreError::Storage(e.to_string()))?;

                Ok(rows.into_iter().map(row_to_envelope).collect())
            })
        })
    }

    fn read_all(&self, aggregate_type: &str) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let pool = self.pool.clone();
        let agg_type = aggregate_type.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let rows: Vec<PgEventRow> = sqlx::query_as(
                    "SELECT id, aggregate_id, aggregate_type, event_type, version, data, metadata, timestamp
                     FROM kernel_events WHERE aggregate_type = $1 ORDER BY timestamp",
                )
                .bind(&agg_type)
                .fetch_all(&pool)
                .await
                .map_err(|e| EventStoreError::Storage(e.to_string()))?;

                Ok(rows.into_iter().map(row_to_envelope).collect())
            })
        })
    }

    fn read_since(&self, since_id: Ulid) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let pool = self.pool.clone();
        let since = since_id.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let rows: Vec<PgEventRow> = sqlx::query_as(
                    "SELECT id, aggregate_id, aggregate_type, event_type, version, data, metadata, timestamp
                     FROM kernel_events WHERE id > $1 ORDER BY timestamp",
                )
                .bind(&since)
                .fetch_all(&pool)
                .await
                .map_err(|e| EventStoreError::Storage(e.to_string()))?;

                Ok(rows.into_iter().map(row_to_envelope).collect())
            })
        })
    }

    fn read_since_version(
        &self,
        aggregate_id: &str,
        since_version: u64,
    ) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let pool = self.pool.clone();
        let agg_id = aggregate_id.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let rows: Vec<PgEventRow> = sqlx::query_as(
                    "SELECT id, aggregate_id, aggregate_type, event_type, version, data, metadata, timestamp
                     FROM kernel_events WHERE aggregate_id = $1 AND version > $2 ORDER BY version",
                )
                .bind(&agg_id)
                .bind(since_version as i64)
                .fetch_all(&pool)
                .await
                .map_err(|e| EventStoreError::Storage(e.to_string()))?;

                Ok(rows.into_iter().map(row_to_envelope).collect())
            })
        })
    }

    fn latest_version(&self, aggregate_id: &str) -> u64 {
        let pool = self.pool.clone();
        let agg_id = aggregate_id.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let result: Result<Option<(i64,)>, _> = sqlx::query_as(
                    "SELECT MAX(version) FROM kernel_events WHERE aggregate_id = $1",
                )
                .bind(&agg_id)
                .fetch_optional(&pool)
                .await;
                result.ok().flatten().map_or(0, |r| r.0.max(0) as u64)
            })
        })
    }

    fn count(&self, aggregate_type: &str) -> u64 {
        let pool = self.pool.clone();
        let agg_type = aggregate_type.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let result: Result<Option<(i64,)>, _> = sqlx::query_as(
                    "SELECT COUNT(*) FROM kernel_events WHERE aggregate_type = $1",
                )
                .bind(&agg_type)
                .fetch_optional(&pool)
                .await;
                result.ok().flatten().map_or(0, |r| r.0.max(0) as u64)
            })
        })
    }

    fn total_count(&self) -> u64 {
        let pool = self.pool.clone();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let result: Result<Option<(i64,)>, _> =
                    sqlx::query_as("SELECT COUNT(*) FROM kernel_events")
                        .fetch_optional(&pool)
                        .await;
                result.ok().flatten().map_or(0, |r| r.0.max(0) as u64)
            })
        })
    }

    fn clear(&self) -> Result<(), EventStoreError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                sqlx::query("DELETE FROM kernel_events")
                    .execute(&pool)
                    .await
                    .map_err(|e| EventStoreError::Storage(e.to_string()))?;
                Ok(())
            })
        })
    }
}

#[derive(sqlx::FromRow)]
struct PgEventRow {
    id: String,
    aggregate_id: String,
    aggregate_type: String,
    event_type: String,
    version: i64,
    data: serde_json::Value,
    metadata: serde_json::Value,
    timestamp: OffsetDateTime,
}

fn row_to_envelope(row: PgEventRow) -> EventEnvelope {
    let metadata: HashMap<String, String> =
        serde_json::from_value(row.metadata).unwrap_or_default();
    EventEnvelope {
        id: Ulid::from_string(&row.id).unwrap_or_else(|_| Ulid::new()),
        aggregate_id: row.aggregate_id,
        aggregate_type: row.aggregate_type,
        event_type: row.event_type,
        version: row.version as u64,
        data: row.data,
        metadata,
        timestamp: row.timestamp,
    }
}

// ─── PgSnapshotStore ──────────────────────────────────────────────────────

pub struct PgSnapshotStore {
    pool: PgPool,
}

impl PgSnapshotStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl SnapshotStore for PgSnapshotStore {
    fn save(&self, snapshot: &Snapshot) -> Result<(), SnapshotError> {
        let pool = self.pool.clone();
        let snap = snapshot.clone();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                sqlx::query(
                    r#"INSERT INTO kernel_snapshots (aggregate_id, aggregate_type, version, state, timestamp)
                       VALUES ($1, $2, $3, $4, $5)
                       ON CONFLICT (aggregate_id) DO UPDATE SET
                           version = EXCLUDED.version,
                           state = EXCLUDED.state,
                           timestamp = EXCLUDED.timestamp"#,
                )
                .bind(&snap.aggregate_id)
                .bind(&snap.aggregate_type)
                .bind(snap.version as i64)
                .bind(&snap.state)
                .bind(snap.timestamp)
                .execute(&pool)
                .await
                .map_err(|e| SnapshotError::Serialization(e.to_string()))?;
                Ok(())
            })
        })
    }

    fn load(&self, aggregate_id: &str) -> Result<Option<Snapshot>, SnapshotError> {
        let pool = self.pool.clone();
        let agg_id = aggregate_id.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let row: Option<PgSnapshotRow> = sqlx::query_as(
                    "SELECT aggregate_id, aggregate_type, version, state, timestamp
                     FROM kernel_snapshots WHERE aggregate_id = $1",
                )
                .bind(&agg_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| SnapshotError::Serialization(e.to_string()))?;

                Ok(row.map(row_to_snapshot))
            })
        })
    }

    fn load_latest(&self, aggregate_type: &str) -> Result<Option<Snapshot>, SnapshotError> {
        let pool = self.pool.clone();
        let agg_type = aggregate_type.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let row: Option<PgSnapshotRow> = sqlx::query_as(
                    "SELECT aggregate_id, aggregate_type, version, state, timestamp
                     FROM kernel_snapshots WHERE aggregate_type = $1
                     ORDER BY version DESC LIMIT 1",
                )
                .bind(&agg_type)
                .fetch_optional(&pool)
                .await
                .map_err(|e| SnapshotError::Serialization(e.to_string()))?;

                Ok(row.map(row_to_snapshot))
            })
        })
    }

    fn delete(&self, aggregate_id: &str) -> Result<(), SnapshotError> {
        let pool = self.pool.clone();
        let agg_id = aggregate_id.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let result = sqlx::query("DELETE FROM kernel_snapshots WHERE aggregate_id = $1")
                    .bind(&agg_id)
                    .execute(&pool)
                    .await
                    .map_err(|e| SnapshotError::Serialization(e.to_string()))?;

                if result.rows_affected() == 0 {
                    return Err(SnapshotError::NotFound(agg_id));
                }
                Ok(())
            })
        })
    }
}

#[derive(sqlx::FromRow)]
struct PgSnapshotRow {
    aggregate_id: String,
    aggregate_type: String,
    version: i64,
    state: Vec<u8>,
    timestamp: OffsetDateTime,
}

fn row_to_snapshot(row: PgSnapshotRow) -> Snapshot {
    Snapshot {
        aggregate_id: row.aggregate_id,
        aggregate_type: row.aggregate_type,
        version: row.version as u64,
        state: row.state,
        timestamp: row.timestamp,
    }
}

// ─── PgObjectManager ──────────────────────────────────────────────────────

pub struct PgObjectManager {
    pool: PgPool,
}

impl PgObjectManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }
}

impl ObjectManager for PgObjectManager {
    fn create(&self, kind: &str, label: &str, data: &[u8]) -> ObjectId {
        let pool = self.pool.clone();
        let kind = kind.to_string();
        let label = label.to_string();
        let data = data.to_vec();
        let now = Self::now();
        let id = ObjectId::new();
        let id_str = id.to_string();

        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                sqlx::query(
                    "INSERT INTO kernel_objects (id, kind, label, data, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(&id_str)
                .bind(&kind)
                .bind(&label)
                .bind(&data)
                .bind(now)
                .bind(now)
                .execute(&pool)
                .await
                .ok();
            });
        });
        id
    }

    fn read(&self, id: ObjectId) -> Option<Object> {
        let pool = self.pool.clone();
        let id_str = id.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let row: Option<PgObjectRow> = sqlx::query_as(
                    "SELECT id, kind, label, data, created_at, updated_at
                     FROM kernel_objects WHERE id = $1",
                )
                .bind(&id_str)
                .fetch_optional(&pool)
                .await
                .ok()?;
                row.map(row_to_object)
            })
        })
    }

    fn update(&self, id: ObjectId, data: &[u8]) -> bool {
        let pool = self.pool.clone();
        let id_str = id.to_string();
        let data = data.to_vec();
        let now = Self::now();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let result = sqlx::query(
                    "UPDATE kernel_objects SET data = $1, updated_at = $2 WHERE id = $3",
                )
                .bind(&data)
                .bind(now)
                .bind(&id_str)
                .execute(&pool)
                .await;
                matches!(result, Ok(r) if r.rows_affected() > 0)
            })
        })
    }

    fn delete(&self, id: ObjectId) -> bool {
        let pool = self.pool.clone();
        let id_str = id.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let result = sqlx::query("DELETE FROM kernel_objects WHERE id = $1")
                    .bind(&id_str)
                    .execute(&pool)
                    .await;
                matches!(result, Ok(r) if r.rows_affected() > 0)
            })
        })
    }

    fn link(&self, source: ObjectId, target: ObjectId, kind: &str) -> LinkId {
        let pool = self.pool.clone();
        let source_str = source.to_string();
        let target_str = target.to_string();
        let kind = kind.to_string();
        let id = LinkId::new();
        let id_str = id.to_string();
        let now = Self::now();

        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                sqlx::query(
                    "INSERT INTO kernel_object_links (id, source_id, target_id, kind, created_at)
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(&id_str)
                .bind(&source_str)
                .bind(&target_str)
                .bind(&kind)
                .bind(now)
                .execute(&pool)
                .await
                .ok();
            });
        });
        id
    }

    fn unlink(&self, id: LinkId) -> bool {
        let pool = self.pool.clone();
        let id_str = id.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let result = sqlx::query("DELETE FROM kernel_object_links WHERE id = $1")
                    .bind(&id_str)
                    .execute(&pool)
                    .await;
                matches!(result, Ok(r) if r.rows_affected() > 0)
            })
        })
    }

    fn links(&self, object: ObjectId) -> Vec<Link> {
        let pool = self.pool.clone();
        let id_str = object.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let rows: Vec<PgLinkRow> = sqlx::query_as(
                    "SELECT id, source_id, target_id, kind, created_at
                     FROM kernel_object_links
                     WHERE source_id = $1 OR target_id = $1
                     ORDER BY created_at",
                )
                .bind(&id_str)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

                rows.into_iter().map(row_to_link).collect()
            })
        })
    }

    fn find_by_kind(&self, kind: &str) -> Vec<Object> {
        let pool = self.pool.clone();
        let kind_str = kind.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let rows: Vec<PgObjectRow> = sqlx::query_as(
                    "SELECT id, kind, label, data, created_at, updated_at
                     FROM kernel_objects WHERE kind = $1 ORDER BY created_at",
                )
                .bind(&kind_str)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

                rows.into_iter().map(row_to_object).collect()
            })
        })
    }

    fn find_by_label(&self, label: &str) -> Vec<Object> {
        let pool = self.pool.clone();
        let label_str = label.to_string();
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let rows: Vec<PgObjectRow> = sqlx::query_as(
                    "SELECT id, kind, label, data, created_at, updated_at
                     FROM kernel_objects WHERE label = $1 ORDER BY created_at",
                )
                .bind(&label_str)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

                rows.into_iter().map(row_to_object).collect()
            })
        })
    }
}

#[derive(sqlx::FromRow)]
struct PgObjectRow {
    id: String,
    kind: String,
    label: String,
    data: Vec<u8>,
    created_at: i64,
    updated_at: i64,
}

fn row_to_object(row: PgObjectRow) -> Object {
    Object {
        id: Ulid::from_string(&row.id).unwrap_or_else(|_| Ulid::new()),
        kind: row.kind,
        label: row.label,
        data: row.data,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[derive(sqlx::FromRow)]
struct PgLinkRow {
    id: String,
    source_id: String,
    target_id: String,
    kind: String,
    created_at: i64,
}

fn row_to_link(row: PgLinkRow) -> Link {
    Link {
        id: Ulid::from_string(&row.id).unwrap_or_else(|_| Ulid::new()),
        source_id: Ulid::from_string(&row.source_id).unwrap_or_else(|_| Ulid::new()),
        target_id: Ulid::from_string(&row.target_id).unwrap_or_else(|_| Ulid::new()),
        kind: row.kind,
        created_at: row.created_at,
    }
}

// ─── StorageManagerWrapper ─────────────────────────────────────────────────

pub struct StorageManagerWrapper {
    inner: Arc<dyn ArtifactStore>,
    file_dir: Option<String>,
}

impl StorageManagerWrapper {
    pub fn new(store: Arc<dyn ArtifactStore>) -> Self {
        Self {
            inner: store,
            file_dir: None,
        }
    }

    pub fn new_file_based(dir: &str) -> Self {
        std::fs::create_dir_all(dir).ok();
        Self {
            inner: Arc::new(NoopArtifactStore),
            file_dir: Some(dir.to_string()),
        }
    }
}

impl StorageManager for StorageManagerWrapper {
    fn store(&self, key: &str, value: &[u8], content_type: Option<&str>) {
        if let Some(dir) = &self.file_dir {
            let path = format!("{dir}/{key}");
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(&path, value).ok();
        } else {
            let key = key.to_string();
            let value = value.to_vec();
            let ct = content_type.map(String::from);
            let store = self.inner.clone();
            tokio::task::block_in_place(move || {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async move {
                    store
                        .put(&key, &value, ct.as_deref())
                        .await
                        .ok();
                });
            });
        }
    }

    fn load(&self, key: &str) -> Option<Entry> {
        if let Some(dir) = &self.file_dir {
            let path = format!("{dir}/{key}");
            let value = std::fs::read(&path).ok()?;
            Some(Entry {
                key: key.to_string(),
                value,
                content_type: None,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            })
        } else {
            let key = key.to_string();
            let store = self.inner.clone();
            tokio::task::block_in_place(move || {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async move {
                    let data = store.get(&key).await.ok()?;
                    Some(Entry {
                        key: key.clone(),
                        value: data.to_vec(),
                        content_type: None,
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64,
                    })
                })
            })
        }
    }

    fn delete(&self, key: &str) -> bool {
        if let Some(dir) = &self.file_dir {
            let path = format!("{dir}/{key}");
            std::fs::remove_file(&path).is_ok()
        } else {
            false
        }
    }

    fn list(&self, prefix: &str) -> Vec<String> {
        if let Some(dir) = &self.file_dir {
            let path = std::path::Path::new(dir).join(prefix);
            std::fs::read_dir(&path)
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.path().to_string_lossy().to_string())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    fn exists(&self, key: &str) -> bool {
        if let Some(dir) = &self.file_dir {
            let path = format!("{dir}/{key}");
            std::path::Path::new(&path).exists()
        } else {
            self.load(key).is_some()
        }
    }
}

struct NoopArtifactStore;

#[async_trait]
impl ArtifactStore for NoopArtifactStore {
    async fn put(
        &self,
        _key: &str,
        _data: &[u8],
        _content_type: Option<&str>,
    ) -> Result<String, tordex_types::StoreError> {
        Ok("noop".to_string())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, tordex_types::StoreError> {
        Err(tordex_types::StoreError::NotFound(key.to_string()))
    }
}
