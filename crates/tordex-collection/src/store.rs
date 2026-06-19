//! Persistence for collection attempts.

use async_trait::async_trait;
use bytes::Bytes;
use sqlx::PgPool;
use sqlx::Row;
use thiserror::Error;
use time::OffsetDateTime;

use tordex_core::error::CoreError;
use tordex_core::id::{CollectionId, SourceId};

use crate::collector::{CollectionResult, CollectionStatus, CollectorKind};

/// Errors produced by the collection store.
#[derive(Debug, Error)]
pub enum CollectionStoreError {
    #[error("collection {0} not found")]
    NotFound(CollectionId),
    #[error(transparent)]
    Core(#[from] CoreError),
}

impl From<sqlx::Error> for CollectionStoreError {
    fn from(err: sqlx::Error) -> Self {
        Self::Core(CoreError::infra(err.to_string()))
    }
}

/// A row view of a collection attempt.
#[derive(Debug, Clone)]
pub struct CollectionRecord {
    pub id: CollectionId,
    pub source_id: SourceId,
    pub started_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub status: String,
    pub collector_used: String,
    pub final_url: Option<String>,
    pub content_type: Option<String>,
    pub byte_count: i64,
    pub http_status: Option<i32>,
    pub error_message: Option<String>,
}

/// Store for collection attempts.
#[async_trait]
pub trait CollectionStore: Send + Sync {
    /// Persist a "running" row.
    async fn record_started(
        &self,
        source_id: &SourceId,
        collection_id: CollectionId,
        collector: CollectorKind,
    ) -> Result<(), CollectionStoreError>;

    /// Persist the final state.
    async fn record_finished(
        &self,
        result: &CollectionResult,
    ) -> Result<(), CollectionStoreError>;

    /// Fetch a record by id.
    async fn get(&self, id: CollectionId) -> Result<CollectionRecord, CollectionStoreError>;

    /// Page through records for a source.
    async fn list_for_source(
        &self,
        source_id: &SourceId,
        limit: u32,
        cursor: Option<CollectionId>,
    ) -> Result<Vec<CollectionRecord>, CollectionStoreError>;

    /// Look up an existing collection row matching `source_id` and
    /// `idempotency_key`. Used to dedupe `POST /collections` requests that
    /// carry an `Idempotency-Key` header.
    async fn find_idempotent(
        &self,
        source_id: &SourceId,
        idempotency_key: &str,
    ) -> Result<Option<CollectionRecord>, CollectionStoreError>;

    /// Attach an idempotency key to a previously-started row.
    async fn attach_idempotency_key(
        &self,
        id: CollectionId,
        idempotency_key: &str,
    ) -> Result<(), CollectionStoreError>;
}

/// Postgres-backed implementation of [`CollectionStore`].
#[derive(Debug, Clone)]
pub struct PgCollectionStore {
    pool: PgPool,
}

impl PgCollectionStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CollectionStore for PgCollectionStore {
    async fn record_started(
        &self,
        source_id: &SourceId,
        collection_id: CollectionId,
        collector: CollectorKind,
    ) -> Result<(), CollectionStoreError> {
        let started_at = tordex_core::now();
        sqlx::query(
            r#"
            INSERT INTO collections (
                id, source_id, started_at, status, collector_used
            ) VALUES ($1, $2, $3, 'running', $4)
            "#,
        )
        .bind(collection_id.to_string())
        .bind(source_id.to_string())
        .bind(started_at)
        .bind(collector.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn record_finished(
        &self,
        result: &CollectionResult,
    ) -> Result<(), CollectionStoreError> {
        let body: Option<Bytes> = result.body.clone();
        sqlx::query(
            r#"
            UPDATE collections SET
                started_at = $2,
                completed_at = $3,
                status = $4,
                final_url = $5,
                content_type = $6,
                byte_count = $7,
                http_status = $8,
                error_message = $9,
                body = $10
            WHERE id = $1
            "#,
        )
        .bind(result.id.to_string())
        .bind(result.started_at)
        .bind(result.completed_at)
        .bind(status_to_str(result.status))
        .bind(result.final_url.as_deref())
        .bind(result.content_type.as_deref())
        .bind(result.byte_count as i64)
        .bind(result.http_status.map(|s| s.as_u16() as i32))
        .bind(result.error.as_deref())
        .bind(body.map(|b| b.to_vec()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(&self, id: CollectionId) -> Result<CollectionRecord, CollectionStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id, source_id, started_at, completed_at, status, collector_used,
                   final_url, content_type, byte_count, http_status, error_message
            FROM collections
            WHERE id = $1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => row_to_record(&row),
            None => Err(CollectionStoreError::NotFound(id)),
        }
    }

    async fn list_for_source(
        &self,
        source_id: &SourceId,
        limit: u32,
        cursor: Option<CollectionId>,
    ) -> Result<Vec<CollectionRecord>, CollectionStoreError> {
        let limit_i64 = i64::from(limit.clamp(1, 500));
        let cursor_str = cursor.map(|c| c.to_string());
        let rows = sqlx::query(
            r#"
            SELECT id, source_id, started_at, completed_at, status, collector_used,
                   final_url, content_type, byte_count, http_status, error_message
            FROM collections
            WHERE source_id = $1
              AND ($2::text IS NULL OR id < $2)
            ORDER BY id DESC
            LIMIT $3
            "#,
        )
        .bind(source_id.to_string())
        .bind(cursor_str)
        .bind(limit_i64)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_record).collect()
    }

    async fn find_idempotent(
        &self,
        source_id: &SourceId,
        idempotency_key: &str,
    ) -> Result<Option<CollectionRecord>, CollectionStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id, source_id, started_at, completed_at, status, collector_used,
                   final_url, content_type, byte_count, http_status, error_message
            FROM collections
            WHERE source_id = $1 AND idempotency_key = $2
            LIMIT 1
            "#,
        )
        .bind(source_id.to_string())
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_record).transpose()
    }

    async fn attach_idempotency_key(
        &self,
        id: CollectionId,
        idempotency_key: &str,
    ) -> Result<(), CollectionStoreError> {
        sqlx::query("UPDATE collections SET idempotency_key = $2 WHERE id = $1")
            .bind(id.to_string())
            .bind(idempotency_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub fn row_to_record_pub(row: &sqlx::postgres::PgRow) -> Result<CollectionRecord, CollectionStoreError> {
    row_to_record(row)
}

fn row_to_record(row: &sqlx::postgres::PgRow) -> Result<CollectionRecord, CollectionStoreError> {
    let id_str: String = row.try_get("id")?;
    let id = CollectionId::from_str(&id_str).ok_or_else(|| {
        CollectionStoreError::Core(CoreError::infra(format!(
            "invalid collection id in row: {id_str}"
        )))
    })?;
    let source_id_str: String = row.try_get("source_id")?;
    let source_id = SourceId::from_str(&source_id_str).ok_or_else(|| {
        CollectionStoreError::Core(CoreError::infra(format!(
            "invalid source id in row: {source_id_str}"
        )))
    })?;
    let http_status: Option<i32> = row.try_get("http_status")?;
    let byte_count: i64 = row.try_get("byte_count")?;
    Ok(CollectionRecord {
        id,
        source_id,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        status: row.try_get("status")?,
        collector_used: row.try_get("collector_used")?,
        final_url: row.try_get("final_url")?,
        content_type: row.try_get("content_type")?,
        byte_count,
        http_status,
        error_message: row.try_get("error_message")?,
    })
}

fn status_to_str(status: CollectionStatus) -> &'static str {
    match status {
        CollectionStatus::Succeeded => "succeeded",
        CollectionStatus::Failed => "failed",
        CollectionStatus::Cancelled => "cancelled",
        CollectionStatus::RateLimited => "rate_limited",
    }
}