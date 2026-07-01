//! CVE Watcher — monitors CVE feeds and databases for new advisories.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tordex_core::{Kernel, Result};

use crate::watcher::{ChangeEvent, Watcher};

struct CveState {
    tracked_feeds: Vec<String>,
    known_cves: HashMap<String, String>,
}

/// Watches CVE feeds and databases for new advisories and changes.
///
/// Uses: kernel.drivers.execute("http", "fetch_json", ...), kernel.objects
pub struct CveWatcher {
    state: Arc<Mutex<CveState>>,
}

impl CveWatcher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CveState {
                tracked_feeds: vec![
                    "https://cve.circl.lu/api/last".into(),
                    "https://services.nvd.nist.gov/rest/json/cves/2.0".into(),
                ],
                known_cves: HashMap::new(),
            })),
        }
    }
}

impl Watcher for CveWatcher {
    fn name(&self) -> &str {
        "cve"
    }

    fn kind(&self) -> &str {
        "vulnerability"
    }

    fn description(&self) -> &str {
        "Monitors CVE feeds and databases for new advisories and changes"
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(1800) // 30 minutes
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        let existing = kernel.objects.find_by_kind("monitored_cve_feed");
        let mut state = self.state.lock().unwrap();
        for obj in &existing {
            let feed_url = obj.label.clone();
            if !state.tracked_feeds.contains(&feed_url) {
                state.tracked_feeds.push(feed_url);
            }
        }
        // Load known CVEs from kernel objects
        let known = kernel.objects.find_by_kind("cve_record");
        for obj in &known {
            state.known_cves.insert(obj.label.clone(), obj.label.clone());
        }
        kernel.event.subscribe("monitoring.cve");
        tracing::info!(
            feeds = state.tracked_feeds.len(),
            known = state.known_cves.len(),
            "cve watcher initialized"
        );
        Ok(())
    }

    fn poll(&self, kernel: &Kernel) -> Result<Vec<ChangeEvent>> {
        let mut changes = Vec::new();
        let feeds: Vec<String> = {
            let state = self.state.lock().unwrap();
            state.tracked_feeds.clone()
        };
        let has_http = !kernel.drivers.find_by_capability("fetch_json").is_empty();

        for feed_url in &feeds {
            let result = if has_http {
                kernel.drivers.execute("http", "fetch_json", json!({"url": feed_url}))
            } else {
                Err(tordex_core::driver::DriverError::DriverNotFound("http".into()).into())
            };

            match result {
                Ok(resp) => {
                    let cves = resp.get("cves").or_else(|| resp.as_array().map(|_| &resp));
                    if let Some(items) = cves {
                        if let Some(arr) = items.as_array() {
                            for item in arr {
                                let cve_id = item
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let severity = item
                                    .get("severity")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let description = item
                                    .get("description")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                let is_new = {
                                    let state = self.state.lock().unwrap();
                                    !state.known_cves.contains_key(&cve_id)
                                };

                                if is_new {
                                    {
                                        let mut state = self.state.lock().unwrap();
                                        state.known_cves.insert(cve_id.clone(), cve_id.clone());
                                    }
                                    // Store as kernel object
                                    kernel.objects.create(
                                        "cve_record",
                                        &cve_id,
                                        &serde_json::to_vec(&json!({
                                            "id": cve_id,
                                            "severity": severity,
                                            "description": description,
                                            "source": feed_url,
                                        })).unwrap(),
                                    );

                                    let change = ChangeEvent::new(
                                        "cve",
                                        &cve_id,
                                        "new_advisory",
                                        json!({
                                            "id": cve_id,
                                            "severity": severity,
                                            "description": description,
                                            "source": feed_url,
                                        }),
                                    );
                                    changes.push(change);
                                }
                            }
                        }
                    }

                    // Record feed status
                    let change = ChangeEvent::new(
                        "cve",
                        feed_url,
                        "feed_polled",
                        json!({"url": feed_url, "status": "ok"}),
                    );
                    changes.push(change);
                }
                Err(e) => {
                    tracing::debug!(feed = %feed_url, error = %e, "CVE feed poll failed");
                    let change = ChangeEvent::new(
                        "cve",
                        feed_url,
                        "feed_failed",
                        json!({"url": feed_url, "error": e.to_string()}),
                    );
                    changes.push(change);
                }
            }
        }

        Ok(changes)
    }
}
