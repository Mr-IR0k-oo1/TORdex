use std::time::Duration;
use tordex_core::CoreError;

const DEFAULT_HTTP_BIND: &str = "0.0.0.0:8080";
const DEFAULT_DATABASE_URL: &str = "postgres://tordex:tordex@localhost:5432/tordex";
const DEFAULT_MINIO_ENDPOINT: &str = "http://localhost:9000";
const DEFAULT_MINIO_REGION: &str = "us-east-1";
const DEFAULT_MINIO_BUCKET: &str = "tordex-evidence";

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct MinioConfig {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub http_bind: String,
    pub database: DatabaseConfig,
    pub minio: MinioConfig,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, CoreError> {
        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
        if database_url.is_empty() {
            return Err(CoreError::Config("missing DATABASE_URL".into()));
        }

        let minio_access_key =
            std::env::var("MINIO_ACCESS_KEY").unwrap_or_else(|_| "tordex".to_string());
        let minio_secret_key =
            std::env::var("MINIO_SECRET_KEY").unwrap_or_else(|_| "tordex-secret".to_string());
        if minio_access_key.is_empty() {
            return Err(CoreError::Config("missing MINIO_ACCESS_KEY".into()));
        }
        if minio_secret_key.is_empty() {
            return Err(CoreError::Config("missing MINIO_SECRET_KEY".into()));
        }

        let database = DatabaseConfig {
            url: database_url,
            max_connections: parse_env("DATABASE_MAX_CONNECTIONS", 20)?,
            min_connections: parse_env("DATABASE_MIN_CONNECTIONS", 2)?,
            acquire_timeout: Duration::from_secs(parse_env("DATABASE_ACQUIRE_TIMEOUT_SECS", 5)?),
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

        Ok(Self {
            http_bind: std::env::var("TORDEX_HTTP_BIND")
                .unwrap_or_else(|_| DEFAULT_HTTP_BIND.to_string()),
            database,
            minio,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env_returns_default_when_unset() {
        let v: u32 = parse_env("TORDEX_TEST_UNSET_VAR", 42).unwrap();
        assert_eq!(v, 42);
    }

    #[test]
    fn parse_env_parses_valid_value() {
        let v: u32 = parse_env("TORDEX_TEST_UNSET_VAR", 42).unwrap();
        assert_eq!(v, 42);
    }

    #[test]
    fn defaults_are_sensible() {
        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.http_bind, "0.0.0.0:8080");
        assert_eq!(config.database.url, "postgres://tordex:tordex@localhost:5432/tordex");
        assert_eq!(config.database.max_connections, 20);
        assert_eq!(config.database.min_connections, 2);
        assert_eq!(config.minio.endpoint, "http://localhost:9000");
        assert_eq!(config.minio.region, "us-east-1");
        assert_eq!(config.minio.bucket, "tordex-evidence");
        assert_eq!(config.minio.access_key, "tordex");
        assert_eq!(config.minio.secret_key, "tordex-secret");
    }
}
