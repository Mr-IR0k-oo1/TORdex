//! API Watcher — monitors external API endpoints for availability and response changes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tordex_core::{Kernel, Result};

use crate::watcher::{ChangeEvent, Watcher};

struct ApiState {
    endpoints: HashMap<String, serde_json::Value>,
}

/// Watches external API endpoints for availability and response changes.
///
/// Uses: kernel.drivers.execute("http", "fetch_json", ...), kernel.objects
pub struct ApiWatcher {
    state: Arc<Mutex<ApiState>>,
}

impl ApiWatcher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ApiState {
                endpoints: HashMap::new(),
            })),
        }
    }
}

impl Watcher for ApiWatcher {
    fn name(&self) -> &str {
        "api"
    }

    fn kind(&self) -> &str {
        "endpoint"
    }

    fn description(&self) -> &str {
        "Monitors external API endpoints for availability and response changes"
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(120) // 2 minutes
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        let existing = kernel.objects.find_by_kind("monitored_api");
        let mut state = self.state.lock().unwrap();
        for obj in &existing {
            let label = obj.label.clone();
            if !state.endpoints.contains_key(&label) {
                state.endpoints.insert(label, json!({"status": "unknown"}));
            }
        }
        // Seed default endpoints if none exist
        if state.endpoints.is_empty() {
            let defaults = vec![
                "https://api.github.com",
                "https://httpbin.org/get",
            ];
            for url in defaults {
                state.endpoints.insert(url.to_string(), json!({"status": "pending"}));
                if kernel.objects.find_by_label(url).is_empty() {
                    kernel.objects.create(
                        "monitored_api",
                        url,
                        &serde_json::to_vec(&json!({"url": url, "status": "pending"})).unwrap(),
                    );
                }
            }
        }
        kernel.event.subscribe("monitoring.api");
        tracing::info!(count = state.endpoints.len(), "api watcher initialized");
        Ok(())
    }

    fn poll(&self, kernel: &Kernel) -> Result<Vec<ChangeEvent>> {
        let mut changes = Vec::new();
        let endpoints: Vec<String> = {
            let state = self.state.lock().unwrap();
            state.endpoints.keys().cloned().collect()
        };

        let has_http = !kernel.drivers.find_by_capability("fetch_json").is_empty();
        // Try fetch_html as fallback
        let has_fetch = has_http || !kernel.drivers.find_by_capability("fetch_html").is_empty();

        for url in &endpoints {
            // Try fetch_json first, fall back to fetch_html
            let result = if has_http {
                kernel.drivers.execute("http", "fetch_json", json!({"url": url}))
            } else if has_fetch {
                kernel.drivers.execute("http", "fetch_html", json!({"url": url}))
            } else {
                Err(tordex_core::driver::DriverError::DriverNotFound("http".into()).into())
            };

            match result {
                Ok(resp) => {
                    let status = resp.get("status_code").and_then(|v| v.as_u64()).unwrap_or(0);
                    let prev = {
                        let s = self.state.lock().unwrap();
                        s.endpoints.get(url).cloned()
                    };
                    let current = json!({"status_code": status, "reachable": status == 200});

                    let change_type = match prev {
                        Some(ref p) if p == &current => "no_change",
                        _ => "changed",
                    };

                    {
                        let mut s = self.state.lock().unwrap();
                        s.endpoints.insert(url.clone(), current.clone());
                    }

                    let change = ChangeEvent::new(
                        "api",
                        url,
                        change_type,
                        json!({"url": url, "status_code": status, "reachable": status == 200}),
                    );
                    changes.push(change);
                }
                Err(e) => {
                    let change = ChangeEvent::new(
                        "api",
                        url,
                        "poll_failed",
                        json!({"url": url, "error": e.to_string()}),
                    );
                    changes.push(change);
                }
            }
        }

        Ok(changes)
    }
}
