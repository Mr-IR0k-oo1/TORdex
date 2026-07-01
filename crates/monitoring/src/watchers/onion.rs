//! Onion Watcher — monitors Tor hidden services for availability and changes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tordex_core::{Kernel, Result};

use crate::watcher::{ChangeEvent, Watcher};

struct OnionState {
    tracked: HashMap<String, String>,
}

/// Watches .onion addresses for availability and content changes.
///
/// Uses: kernel.drivers (Tor/HTTP), kernel.objects, kernel.event
pub struct OnionWatcher {
    state: Arc<Mutex<OnionState>>,
}

impl OnionWatcher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(OnionState {
                tracked: HashMap::new(),
            })),
        }
    }
}

impl Watcher for OnionWatcher {
    fn name(&self) -> &str {
        "onion"
    }

    fn kind(&self) -> &str {
        "hidden-service"
    }

    fn description(&self) -> &str {
        "Monitors Tor hidden services (.onion) for availability and content changes"
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(600) // 10 minutes
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        let existing = kernel.objects.find_by_kind("monitored_onion");
        let mut state = self.state.lock().unwrap();
        for obj in &existing {
            if !state.tracked.contains_key(&obj.label) {
                state
                    .tracked
                    .insert(obj.label.clone(), "active".to_string());
            }
        }
        // Subscribe to onion-related events
        kernel.event.subscribe("monitoring.onion");
        tracing::info!(count = state.tracked.len(), "onion watcher initialized");
        Ok(())
    }

    fn poll(&self, kernel: &Kernel) -> Result<Vec<ChangeEvent>> {
        let mut changes = Vec::new();
        let tracked: Vec<String> = {
            let state = self.state.lock().unwrap();
            state.tracked.keys().cloned().collect()
        };

        for address in &tracked {
            let has_http = !kernel.drivers.find_by_capability("fetch_html").is_empty();

            let result = if has_http {
                kernel.drivers.execute(
                    "http",
                    "head_request",
                    json!({"url": format!("http://{address}")}),
                )
            } else {
                Err(tordex_core::driver::DriverError::DriverNotFound(
                    "http".into(),
                )
                .into())
            };

            match result {
                Ok(resp) => {
                    let status = resp.get("status_code").and_then(|v| v.as_u64()).unwrap_or(0);
                    let reachable = status > 0 && status < 600;
                    let prev_status = {
                        let s = self.state.lock().unwrap();
                        s.tracked.get(address).cloned()
                    };
                    let new_status = if reachable { "online" } else { "offline" };
                    let change_type = if prev_status.as_deref() != Some(new_status) {
                        "status_changed"
                    } else {
                        "no_change"
                    };

                    {
                        let mut s = self.state.lock().unwrap();
                        s.tracked
                            .insert(address.clone(), new_status.to_string());
                    }

                    let change = ChangeEvent::new(
                        "onion",
                        address,
                        change_type,
                        json!({
                            "address": address,
                            "reachable": reachable,
                            "status_code": status,
                            "status": new_status,
                        }),
                    );
                    changes.push(change);
                }
                Err(e) => {
                    tracing::debug!(onion = %address, error = %e, "onion poll failed");
                    let change = ChangeEvent::new(
                        "onion",
                        address,
                        "poll_failed",
                        json!({"address": address, "error": e.to_string()}),
                    );
                    changes.push(change);
                }
            }
        }

        Ok(changes)
    }
}
