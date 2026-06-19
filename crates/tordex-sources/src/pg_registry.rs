//! Postgres-backed [`SourceRegistry`].

use async_trait::async_trait;
use sqlx::PgPool;
use sqlx::Row;

use tordex_core::id::SourceId;

use crate::descriptor::{SourceDescriptor, SourceInput, SourceKind, SourceStatus};
use crate::registry::{SourceFilter, SourcePage, SourceRegistry, SourceRegistryError};

/// Postgres implementation of the source registry.
#[derive(Debug, Clone)]
pub struct PgSourceRegistry {
    pool: PgPool,
}

impl PgSourceRegistry {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SourceRegistry for PgSourceRegistry {
    async fn insert(&self, input: &SourceInput) -> Result<SourceDescriptor, SourceRegistryError> {
        let id = SourceId::generate();
        let now = self.clock();

        let result = sqlx::query(
            r#"
            INSERT INTO sources (
                id, kind, display_name, locator, routing_policy, hints,
                status, tags, metadata, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            "#,
        )
        .bind(id.to_string())
        .bind(kind_to_str(input.kind))
        .bind(&input.display_name)
        .bind(&input.locator)
        .bind(routing_policy_to_str(input.routing_policy))
        .bind(sqlx::types::Json(&input.hints))
        .bind(status_to_str(input.status))
        .bind(&input.tags)
        .bind(sqlx::types::Json(&input.metadata))
        .bind(now)
        .execute(&self.pool)
        .await;

        if let Err(sqlx::Error::Database(db_err)) = &result {
            if db_err.code().as_deref() == Some("23505") {
                return Err(SourceRegistryError::Duplicate {
                    kind: input.kind,
                    locator: input.locator.clone(),
                });
            }
        }
        result?;

        Ok(SourceDescriptor {
            id,
            kind: input.kind,
            display_name: input.display_name.clone(),
            locator: input.locator.clone(),
            routing_policy: input.routing_policy,
            hints: input.hints.clone(),
            status: input.status,
            tags: input.tags.clone(),
            metadata: input.metadata.clone(),
            created_at: now,
            updated_at: now,
        })
    }

    async fn get(&self, id: SourceId) -> Result<SourceDescriptor, SourceRegistryError> {
        let row = sqlx::query(
            r#"
            SELECT id, kind, display_name, locator, routing_policy, hints,
                   status, tags, metadata, created_at, updated_at
            FROM sources
            WHERE id = $1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(row_to_descriptor(&row)?),
            None => Err(SourceRegistryError::NotFound(id)),
        }
    }

    async fn list(&self, filter: &SourceFilter) -> Result<SourcePage, SourceRegistryError> {
        let limit = filter.limit.unwrap_or(50).clamp(1, 500);
        // We over-fetch by one to determine whether a next page exists.
        let limit_plus_one = i64::from(limit) + 1;

        let kind_str = filter.kind.map(kind_to_str);
        let status_str = filter.status.map(status_to_str);
        let cursor_str = filter.cursor.map(|c| c.to_string());

        let rows = sqlx::query(
            r#"
            SELECT id, kind, display_name, locator, routing_policy, hints,
                   status, tags, metadata, created_at, updated_at
            FROM sources
            WHERE ($1::text IS NULL OR kind = $1)
              AND ($2::text IS NULL OR status = $2)
              AND ($3::text IS NULL OR id < $3)
            ORDER BY id DESC
            LIMIT $4
            "#,
        )
        .bind(kind_str)
        .bind(status_str)
        .bind(cursor_str)
        .bind(limit_plus_one)
        .fetch_all(&self.pool)
        .await?;

        let mut sources = Vec::with_capacity(rows.len());
        for row in &rows {
            sources.push(row_to_descriptor(row)?);
        }

        let next_cursor = if sources.len() as i64 > i64::from(limit) {
            sources.pop();
            sources.last().map(|s| s.id)
        } else {
            None
        };

        Ok(SourcePage { sources, next_cursor })
    }

    async fn update(
        &self,
        id: SourceId,
        input: &SourceInput,
    ) -> Result<SourceDescriptor, SourceRegistryError> {
        let now = self.clock();

        let result = sqlx::query(
            r#"
            UPDATE sources SET
                kind = $2,
                display_name = $3,
                locator = $4,
                routing_policy = $5,
                hints = $6,
                status = $7,
                tags = $8,
                metadata = $9,
                updated_at = $10
            WHERE id = $1
            "#,
        )
        .bind(id.to_string())
        .bind(kind_to_str(input.kind))
        .bind(&input.display_name)
        .bind(&input.locator)
        .bind(routing_policy_to_str(input.routing_policy))
        .bind(sqlx::types::Json(&input.hints))
        .bind(status_to_str(input.status))
        .bind(&input.tags)
        .bind(sqlx::types::Json(&input.metadata))
        .bind(now)
        .execute(&self.pool)
        .await;

        if let Err(sqlx::Error::Database(db_err)) = &result {
            if db_err.code().as_deref() == Some("23505") {
                return Err(SourceRegistryError::Duplicate {
                    kind: input.kind,
                    locator: input.locator.clone(),
                });
            }
        }
        let result = result?;

        if result.rows_affected() == 0 {
            return Err(SourceRegistryError::NotFound(id));
        }

        self.get(id).await
    }

    async fn delete(&self, id: SourceId) -> Result<(), SourceRegistryError> {
        let result = sqlx::query("DELETE FROM sources WHERE id = $1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(SourceRegistryError::NotFound(id));
        }
        Ok(())
    }
}

fn row_to_descriptor(row: &sqlx::postgres::PgRow) -> Result<SourceDescriptor, SourceRegistryError> {
    let id_str: String = row.try_get("id")?;
    let id = SourceId::from_str(&id_str)
        .ok_or_else(|| SourceRegistryError::Core(tordex_core::error::CoreError::infra(format!("invalid source id in row: {id_str}"))))?;
    let kind_str: String = row.try_get("kind")?;
    let kind = str_to_kind(&kind_str)
        .ok_or_else(|| SourceRegistryError::Core(tordex_core::error::CoreError::infra(format!("unknown kind {kind_str:?}"))))?;
    let routing_policy_str: String = row.try_get("routing_policy")?;
    let routing_policy = str_to_routing_policy(&routing_policy_str).ok_or_else(|| {
        SourceRegistryError::Core(tordex_core::error::CoreError::infra(format!(
            "unknown routing_policy {routing_policy_str:?}"
        )))
    })?;
    let hints: sqlx::types::Json<crate::descriptor::CollectionHints> = row.try_get("hints")?;
    let status_str: String = row.try_get("status")?;
    let status = str_to_status(&status_str).ok_or_else(|| {
        SourceRegistryError::Core(tordex_core::error::CoreError::infra(format!(
            "unknown status {status_str:?}"
        )))
    })?;
    let tags: Vec<String> = row.try_get("tags")?;
    let metadata: sqlx::types::Json<serde_json::Value> = row.try_get("metadata")?;

    Ok(SourceDescriptor {
        id,
        kind,
        display_name: row.try_get("display_name")?,
        locator: row.try_get("locator")?,
        routing_policy,
        hints: hints.0,
        status,
        tags,
        metadata: metadata.0,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn kind_to_str(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Website => "website",
        SourceKind::OnionService => "onion_service",
        SourceKind::Api => "api",
        SourceKind::Repository => "repository",
        SourceKind::Document => "document",
        SourceKind::RssFeed => "rss_feed",
        SourceKind::LocalFile => "local_file",
        SourceKind::Paper => "paper",
    }
}

fn str_to_kind(s: &str) -> Option<SourceKind> {
    Some(match s {
        "website" => SourceKind::Website,
        "onion_service" => SourceKind::OnionService,
        "api" => SourceKind::Api,
        "repository" => SourceKind::Repository,
        "document" => SourceKind::Document,
        "rss_feed" => SourceKind::RssFeed,
        "local_file" => SourceKind::LocalFile,
        "paper" => SourceKind::Paper,
        _ => return None,
    })
}

fn routing_policy_to_str(p: crate::descriptor::RoutingPolicy) -> &'static str {
    match p {
        crate::descriptor::RoutingPolicy::Auto => "auto",
        crate::descriptor::RoutingPolicy::Http => "http",
        crate::descriptor::RoutingPolicy::Browser => "browser",
    }
}

fn str_to_routing_policy(s: &str) -> Option<crate::descriptor::RoutingPolicy> {
    Some(match s {
        "auto" => crate::descriptor::RoutingPolicy::Auto,
        "http" => crate::descriptor::RoutingPolicy::Http,
        "browser" => crate::descriptor::RoutingPolicy::Browser,
        _ => return None,
    })
}

fn status_to_str(s: SourceStatus) -> &'static str {
    match s {
        SourceStatus::Active => "active",
        SourceStatus::Paused => "paused",
        SourceStatus::Errored => "errored",
    }
}

fn str_to_status(s: &str) -> Option<SourceStatus> {
    Some(match s {
        "active" => SourceStatus::Active,
        "paused" => SourceStatus::Paused,
        "errored" => SourceStatus::Errored,
        _ => return None,
    })
}