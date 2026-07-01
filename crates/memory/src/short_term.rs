use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use ulid::Ulid;

use redis::AsyncCommands;

use crate::consolidation::ConsolidationCandidate;

/// Maximum entries in the STM ring before oldest are evicted.
const DEFAULT_STM_CAPACITY: usize = 10_000;
/// Default TTL for STM entries.
const DEFAULT_STM_TTL_SECS: i64 = 3600;

/// An entry in short-term memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STMEntry {
    pub id: Ulid,
    pub kind: String,
    pub content: serde_json::Value,
    pub context: std::collections::HashMap<String, String>,
    pub created_at: OffsetDateTime,
    pub access_count: u64,
    pub importance: f64,
}

impl STMEntry {
    pub fn new(kind: &str, content: serde_json::Value) -> Self {
        Self {
            id: Ulid::new(),
            kind: kind.to_string(),
            content,
            context: std::collections::HashMap::new(),
            created_at: OffsetDateTime::now_utc(),
            access_count: 0,
            importance: 0.0,
        }
    }

    pub fn with_importance(mut self, importance: f64) -> Self {
        self.importance = importance;
        self
    }

    pub fn with_context(mut self, key: &str, value: &str) -> Self {
        self.context.insert(key.to_string(), value.to_string());
        self
    }
}

impl ConsolidationCandidate for STMEntry {
    fn importance(&self) -> f64 {
        self.importance
    }

    fn access_count(&self) -> u64 {
        self.access_count
    }

    fn age_seconds(&self) -> i64 {
        (OffsetDateTime::now_utc() - self.created_at).whole_seconds()
    }

    fn kind(&self) -> &str {
        &self.kind
    }
}

/// Query for short-term memory recall.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STMQuery {
    pub kinds: Option<Vec<String>>,
    pub since: Option<OffsetDateTime>,
    pub until: Option<OffsetDateTime>,
    pub limit: usize,
    pub min_importance: f64,
}

impl Default for STMQuery {
    fn default() -> Self {
        Self {
            kinds: None,
            since: None,
            until: None,
            limit: 100,
            min_importance: 0.0,
        }
    }
}

/// Short-Term Memory — recent events with bounded capacity and TTL.
///
/// - Redis-backed ring buffer with TTL
/// - Importance scoring for consolidation candidates
/// - Temporal query support
pub trait ShortTermMemory: Send + Sync {
    fn store(&mut self, entry: STMEntry) -> Result<Ulid, String>;
    fn store_batch(&mut self, entries: Vec<STMEntry>) -> Result<Vec<Ulid>, String>;
    fn recall(&self, query: &STMQuery) -> Result<Vec<STMEntry>, String>;
    fn get(&self, id: Ulid) -> Result<Option<STMEntry>, String>;
    fn count(&self) -> Result<usize, String>;
    fn clear(&mut self) -> Result<(), String>;
    fn consolidate_candidates(&self, min_importance: f64, max_age_secs: i64) -> Result<Vec<STMEntry>, String>;
}

/// Redis-backed short-term memory.
pub struct RedisShortTermMemory {
    redis: redis::aio::ConnectionManager,
    key_prefix: String,
    capacity: usize,
    ttl_secs: i64,
}

impl RedisShortTermMemory {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        Self {
            redis,
            key_prefix: "tordex:stm:".to_string(),
            capacity: DEFAULT_STM_CAPACITY,
            ttl_secs: DEFAULT_STM_TTL_SECS,
        }
    }

    fn entry_key(&self, id: Ulid) -> String {
        format!("{}entry:{}", self.key_prefix, id)
    }

    fn index_key(&self) -> String {
        format!("{}index", self.key_prefix)
    }
}

fn block_on_redis<F, T>(f: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, redis::RedisError>>,
{
    futures::executor::block_on(f).map_err(|e| e.to_string())
}

impl ShortTermMemory for RedisShortTermMemory {
    fn store(&mut self, entry: STMEntry) -> Result<Ulid, String> {
        let id = entry.id;
        let key = self.entry_key(id);
        let json = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
        let redis = self.redis.clone();
        let index_key = self.index_key();
        let capacity = self.capacity;
        let ttl_secs = self.ttl_secs;

        block_on_redis(async move {
            let mut conn = redis;
            let _: () = conn.set_ex(&key, &json, ttl_secs as u64).await?;
            let _: isize = conn
                .zadd(&index_key, id.to_string(), id.timestamp_ms() as f64)
                .await?;
            let _: isize = conn
                .zremrangebyrank(&index_key, 0, -(capacity as isize + 1))
                .await?;
            Ok(id)
        })
    }

    fn store_batch(&mut self, entries: Vec<STMEntry>) -> Result<Vec<Ulid>, String> {
        entries.into_iter().map(|e| self.store(e)).collect()
    }

    fn recall(&self, query: &STMQuery) -> Result<Vec<STMEntry>, String> {
        let redis = self.redis.clone();
        let index_key = self.index_key();
        let limit = query.limit;
        let kinds = query.kinds.clone();
        let min_importance = query.min_importance;

        let min_score = query
            .since
            .map(|t| t.unix_timestamp() as f64 * 1000.0)
            .unwrap_or(-1e18);
        let max_score = query
            .until
            .map(|t| t.unix_timestamp() as f64 * 1000.0)
            .unwrap_or(1e18);

        let ids: Vec<String> = block_on_redis(async move {
            let mut conn = redis;
            let ids: Vec<String> = conn
                .zrangebyscore_limit(&index_key, min_score, max_score, 0, limit as isize)
                .await?;
            Ok(ids)
        })?;

        let mut results = Vec::new();
        let redis2 = self.redis.clone();
        for id_str in ids {
            if let Ok(id) = Ulid::from_string(&id_str) {
                let key = self.entry_key(id);
                let raw: Option<String> = block_on_redis(async {
                    let mut conn = redis2.clone();
                    conn.get(&key).await
                })?;
                if let Some(json) = raw {
                    if let Ok(entry) = serde_json::from_str::<STMEntry>(&json) {
                        if entry.importance >= min_importance {
                            if let Some(ref kinds) = kinds {
                                if kinds.contains(&entry.kind) || kinds.is_empty() {
                                    results.push(entry);
                                }
                            } else {
                                results.push(entry);
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    fn get(&self, id: Ulid) -> Result<Option<STMEntry>, String> {
        let redis = self.redis.clone();
        let key = self.entry_key(id);
        let raw: Option<String> = block_on_redis(async move {
            let mut conn = redis;
            conn.get(&key).await
        })?;
        match raw {
            Some(json) => serde_json::from_str(&json).map(Some).map_err(|e| e.to_string()),
            None => Ok(None),
        }
    }

    fn count(&self) -> Result<usize, String> {
        let redis = self.redis.clone();
        let index_key = self.index_key();
        let count: isize = block_on_redis(async move {
            let mut conn = redis;
            conn.zcard(&index_key).await
        })?;
        Ok(count.max(0) as usize)
    }

    fn clear(&mut self) -> Result<(), String> {
        let redis = self.redis.clone();
        let index_key = self.index_key();
        block_on_redis(async move {
            let mut conn = redis;
            let _: () = conn.del(&index_key).await?;
            Ok(())
        })
    }

    fn consolidate_candidates(&self, min_importance: f64, max_age_secs: i64) -> Result<Vec<STMEntry>, String> {
        let now = OffsetDateTime::now_utc();
        let min_time = (now - time::Duration::seconds(max_age_secs)).unix_timestamp() as f64 * 1000.0;
        let query = STMQuery {
            since: Some(OffsetDateTime::from_unix_timestamp((min_time / 1000.0) as i64).unwrap_or(now)),
            limit: 1000,
            min_importance,
            ..Default::default()
        };
        let entries = self.recall(&query)?;
        Ok(entries)
    }
}
