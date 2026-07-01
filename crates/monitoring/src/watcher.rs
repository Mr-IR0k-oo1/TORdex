//! Watcher trait — each monitored domain implements this.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use tordex_core::{Kernel, Result};

/// A change detected by a watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub id: String,
    pub watcher_kind: String,
    pub subject: String,
    pub change_type: String,
    pub previous_state: Option<Value>,
    pub current_state: Value,
    pub detected_at: OffsetDateTime,
    pub metadata: std::collections::HashMap<String, String>,
}

impl ChangeEvent {
    #[must_use]
    pub fn new(
        watcher_kind: &str,
        subject: &str,
        change_type: &str,
        current_state: Value,
    ) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            watcher_kind: watcher_kind.to_string(),
            subject: subject.to_string(),
            change_type: change_type.to_string(),
            previous_state: None,
            current_state,
            detected_at: OffsetDateTime::now_utc(),
            metadata: std::collections::HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_previous(mut self, previous: Value) -> Self {
        self.previous_state = Some(previous);
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// A watcher continuously observes a domain for changes.
pub trait Watcher: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> &str;
    fn description(&self) -> &str;

    /// How often to poll for changes.
    fn poll_interval(&self) -> Duration;

    /// Initialize the watcher (subscribe to topics, etc.)
    fn init(&self, _kernel: &Kernel) -> Result<()> {
        Ok(())
    }

    /// Poll for changes. Returns any detected changes since the last poll.
    fn poll(&self, kernel: &Kernel) -> Result<Vec<ChangeEvent>>;
}
