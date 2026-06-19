//! Configuration loading via `figment`.
//!
//! Reads from environment variables first (prefix `TORDEX_` for tordex-specific
//! settings, none for `DATABASE_URL`/`REDIS_URL` for compatibility with the
//! conventions of each backend), with optional TOML overrides via `TORDEX_CONFIG`.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

use crate::error::CoreError;

const DEFAULT_HTTP_BIND: &str = "0.0.0.0:8080";
const DEFAULT_DATABASE_URL: &str = "postgres://tordex:tordex@localhost:5432/tordex";
const DEFAULT_REDIS_URL: &str = "redis://localhost:6379";
const DEFAULT_REDIS_STREAM_KEY: &str = "tordex:events";
const DEFAULT_MINIO_ENDPOINT: &str = "http://localhost:9000";
const DEFAULT_MINIO_REGION: &str = "us-east-1";
const DEFAULT_MINIO_BUCKET: &str = "tordex-evidence";
const DEFAULT_QDRANT_URL: &str = "http://localhost:6334";
const DEFAULT_HTTP_USER_AGENT: &str = "TORdex/0.1";
const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 30;
const DEFAULT_HTTP_MAX_REDIRECTS: u8 = 5;
const DEFAULT_RATE_LIMIT_PER_SEC: u32 = 5;
const DEFAULT_RATE_LIMIT_BURST: u32 = 10;
const DEFAULT_BROWSER_BACKEND: &str = "none";
const DEFAULT_LIGHTPANDA_CDP_URL: &str = "ws://localhost:9222";

/// Which browser backend to use when the routing policy escalates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserBackend {
    /// No browser available; `Browser` policy fails fast.
    None,
    /// Connect to an externally-running Lightpanda via CDP WebSocket.
    Lightpanda,
    /// Launch a local Chromium via `chromiumoxide`.
    Chromium,
}

impl BrowserBackend {
    fn from_env(s: &str) -> Result<Self, CoreError> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "lightpanda" => Ok(Self::Lightpanda),
            "chromium" => Ok(Self::Chromium),
            other => Err(CoreError::Config(format!(
                "invalid TORDEX_BROWSER_BACKEND: {other}"
            ))),
        }
    }
}

/// Database connection pool settings.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
}

/// Redis connection settings.
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub stream_key: String,
}

/// MinIO / S3-compatible object storage settings.
#[derive(Debug, Clone)]
pub struct MinioConfig {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
}

/// Qdrant vector database settings.
#[derive(Debug, Clone)]
pub struct QdrantConfig {
    pub url: String,
    pub api_key: Option<String>,
}

/// HTTP collector defaults.
#[derive(Debug, Clone)]
pub struct HttpCollectorConfig {
    pub user_agent: String,
    pub timeout: Duration,
    pub max_redirects: u8,
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    pub per_second: u32,
    pub burst: u32,
}

/// Collection routing policy for browser-backed collection.
#[derive(Debug, Clone)]
pub struct CollectionConfig {
    pub http: HttpCollectorConfig,
    pub rate_limit: RateLimitConfig,
    pub browser_backend: BrowserBackend,
    pub lightpanda_cdp_url: String,
}

/// Top-level application configuration.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub http_bind: String,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub minio: MinioConfig,
    pub qdrant: QdrantConfig,
    pub collection: CollectionConfig,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    MissingEnv(&'static str),
    #[error("invalid value for {name}: {reason}")]
    Invalid {
        name: &'static str,
        reason: String,
    },
}

impl AppConfig {
    /// Load configuration from environment variables, with optional TOML overrides.
    pub fn from_env() -> Result<Self, CoreError> {
        // Mandatory values with fallbacks that match docker-compose.yml so a
        // fresh checkout boots without extra config in development.
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
        if database_url.is_empty() {
            return Err(ConfigError::MissingEnv("DATABASE_URL").into());
        }

        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| DEFAULT_REDIS_URL.to_string());
        if redis_url.is_empty() {
            return Err(ConfigError::MissingEnv("REDIS_URL").into());
        }

        let minio_access_key = std::env::var("MINIO_ACCESS_KEY")
            .unwrap_or_else(|_| "tordex".to_string());
        let minio_secret_key = std::env::var("MINIO_SECRET_KEY")
            .unwrap_or_else(|_| "tordex-secret".to_string());
        if minio_access_key.is_empty() {
            return Err(ConfigError::MissingEnv("MINIO_ACCESS_KEY").into());
        }
        if minio_secret_key.is_empty() {
            return Err(ConfigError::MissingEnv("MINIO_SECRET_KEY").into());
        }

        let database = DatabaseConfig {
            url: database_url,
            max_connections: parse_env("DATABASE_MAX_CONNECTIONS", 20)?,
            min_connections: parse_env("DATABASE_MIN_CONNECTIONS", 2)?,
            acquire_timeout: Duration::from_secs(parse_env(
                "DATABASE_ACQUIRE_TIMEOUT_SECS",
                5,
            )?),
        };

        let redis = RedisConfig {
            url: redis_url,
            stream_key: std::env::var("REDIS_STREAM_KEY")
                .unwrap_or_else(|_| DEFAULT_REDIS_STREAM_KEY.to_string()),
        };

        let minio = MinioConfig {
            endpoint: std::env::var("MINIO_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_MINIO_ENDPOINT.to_string()),
            access_key: minio_access_key,
            secret_key: minio_secret_key,
            region: std::env::var("MINIO_REGION")
                .unwrap_or_else(|_| DEFAULT_MINIO_REGION.to_string()),
            bucket: std::env::var("MINIO_BUCKET")
                .unwrap_or_else(|_| DEFAULT_MINIO_BUCKET.to_string()),
        };

        let qdrant = QdrantConfig {
            url: std::env::var("QDRANT_URL").unwrap_or_else(|_| DEFAULT_QDRANT_URL.to_string()),
            api_key: std::env::var("QDRANT_API_KEY").ok().filter(|v| !v.is_empty()),
        };

        let http = HttpCollectorConfig {
            user_agent: std::env::var("TORDEX_HTTP_USER_AGENT")
                .unwrap_or_else(|_| DEFAULT_HTTP_USER_AGENT.to_string()),
            timeout: Duration::from_secs(parse_env(
                "TORDEX_HTTP_TIMEOUT_SECS",
                DEFAULT_HTTP_TIMEOUT_SECS,
            )?),
            max_redirects: u8::try_from(parse_env(
                "TORDEX_HTTP_MAX_REDIRECTS",
                u64::from(DEFAULT_HTTP_MAX_REDIRECTS),
            )?)
            .map_err(|_| ConfigError::Invalid {
                name: "TORDEX_HTTP_MAX_REDIRECTS",
                reason: "out of range".into(),
            })?,
        };

        let rate_limit = RateLimitConfig {
            per_second: parse_env("TORDEX_RATE_LIMIT_PER_SEC", DEFAULT_RATE_LIMIT_PER_SEC)?,
            burst: parse_env("TORDEX_RATE_LIMIT_BURST", DEFAULT_RATE_LIMIT_BURST)?,
        };

        let browser_backend = BrowserBackend::from_env(
            &std::env::var("TORDEX_BROWSER_BACKEND")
                .unwrap_or_else(|_| DEFAULT_BROWSER_BACKEND.to_string()),
        )?;

        let collection = CollectionConfig {
            http,
            rate_limit,
            browser_backend,
            lightpanda_cdp_url: std::env::var("TORDEX_LIGHTPANDA_CDP_URL")
                .unwrap_or_else(|_| DEFAULT_LIGHTPANDA_CDP_URL.to_string()),
        };

        Ok(Self {
            http_bind: std::env::var("TORDEX_HTTP_BIND")
                .unwrap_or_else(|_| DEFAULT_HTTP_BIND.to_string()),
            database,
            redis,
            minio,
            qdrant,
            collection,
        })
    }
}

fn parse_env<T>(name: &'static str, default: T) -> Result<T, CoreError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(v) => v
            .parse::<T>()
            .map_err(|e| CoreError::Config(format!("invalid {name}: {e}"))),
        Err(_) => Ok(default),
    }
}