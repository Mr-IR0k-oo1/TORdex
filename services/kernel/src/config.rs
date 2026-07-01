use tordex_apps::profiles::{self, AppProfile};
use tordex_config::{AppConfig, MinioConfig};

pub struct KernelConfig {
    pub http_bind: String,
    pub database: tordex_config::DatabaseConfig,
    pub minio: Option<MinioConfig>,
    pub redis_url: Option<String>,
    pub app_profile: &'static AppProfile,
    pub tick_interval_ms: u64,
    pub storage_dir: String,
}

impl KernelConfig {
    pub fn from_env() -> Result<Self, anyhow::Error> {
        let app_config = AppConfig::from_env()?;

        let profile_name = std::env::var("TORDEX_PROFILE")
            .unwrap_or_else(|_| "TORdex OSINT".to_string());
        let app_profile = profiles::find(&profile_name).ok_or_else(|| {
            anyhow::anyhow!("unknown profile '{}'", profile_name)
        })?;

        let redis_url = std::env::var("REDIS_URL")
            .ok()
            .filter(|s| !s.is_empty());

        let tick_interval_ms: u64 = std::env::var("TORDEX_TICK_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(500);

        let storage_dir = std::env::var("TORDEX_STORAGE_DIR")
            .unwrap_or_else(|_| "/tmp/tordex-storage".to_string());

        let http_bind = app_config.http_bind.clone();

        Ok(Self {
            http_bind,
            database: app_config.database,
            minio: Some(app_config.minio),
            redis_url,
            app_profile,
            tick_interval_ms,
            storage_dir,
        })
    }
}
