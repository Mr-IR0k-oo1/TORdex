//! Threat Watcher — aggregates threat intelligence from monitoring changes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tordex_core::{Kernel, Result};

use crate::watcher::{ChangeEvent, Watcher};

struct ThreatState {
    known_threats: Vec<String>,
}

/// Aggregates threat intelligence by correlating changes across all watchers.
///
/// Uses: kernel.objects, kernel.event_store, kernel.event
pub struct ThreatWatcher {
    state: Arc<Mutex<ThreatState>>,
}

impl ThreatWatcher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ThreatState {
                known_threats: Vec::new(),
            })),
        }
    }
}

impl Watcher for ThreatWatcher {
    fn name(&self) -> &str {
        "threat"
    }

    fn kind(&self) -> &str {
        "intelligence"
    }

    fn description(&self) -> &str {
        "Aggregates threat intelligence by correlating changes from all monitoring domains"
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(600) // 10 minutes
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        let existing = kernel.objects.find_by_kind("threat_intel");
        let mut state = self.state.lock().unwrap();
        for obj in &existing {
            state.known_threats.push(obj.label.clone());
        }
        // Subscribe to all monitoring events for correlation
        kernel.event.subscribe("monitoring.repository");
        kernel.event.subscribe("monitoring.onion");
        kernel.event.subscribe("monitoring.api");
        kernel.event.subscribe("monitoring.cve");
        kernel.event.subscribe("monitoring.package");
        kernel.event.subscribe("system.alert");
        tracing::info!(known = state.known_threats.len(), "threat watcher initialized");
        Ok(())
    }

    fn poll(&self, kernel: &Kernel) -> Result<Vec<ChangeEvent>> {
        let mut changes = Vec::new();

        // Read recent monitoring changes from kernel objects
        let monitoring_changes = kernel.objects.find_by_kind("monitoring_change");
        let cve_records = kernel.objects.find_by_kind("cve_record");
        let alerts = kernel.objects.find_by_kind("health_snapshot");

        // Correlate: threats with high-severity CVEs + unreachable services
        for cve in &cve_records {
            if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&cve.data) {
                let severity = data.get("severity").and_then(|v| v.as_str()).unwrap_or("unknown");
                if severity == "critical" || severity == "high" {
                    let cve_id = &cve.label;
                    let is_known = {
                        let state = self.state.lock().unwrap();
                        state.known_threats.contains(cve_id)
                    };

                    if !is_known {
                        let change = ChangeEvent::new(
                            "threat",
                            cve_id,
                            "high_severity_cve",
                            json!({
                                "cve_id": cve_id,
                                "severity": severity,
                                "source": "cve_watcher",
                                "correlation": "direct_cve",
                            }),
                        );
                        changes.push(change);

                        // Record as threat intel object
                        kernel.objects.create(
                            "threat_intel",
                            cve_id,
                            &serde_json::to_vec(&json!({
                                "cve_id": cve_id,
                                "severity": severity,
                                "detected_at": time::OffsetDateTime::now_utc().to_string(),
                                "type": "cve_alert",
                            })).unwrap(),
                        );

                        let mut state = self.state.lock().unwrap();
                        state.known_threats.push(cve_id.clone());
                    }
                }
            }
        }

        // Check for unreachable services as potential threats
        let unreachable_count = monitoring_changes
            .iter()
            .filter(|obj| {
                if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&obj.data) {
                    data.get("change_type")
                        .and_then(|v| v.as_str())
                        == Some("poll_failed")
                } else {
                    false
                }
            })
            .count();

        if unreachable_count > 0 {
            let change = ChangeEvent::new(
                "threat",
                "service_availability",
                "multiple_unreachable",
                json!({
                    "unreachable_count": unreachable_count,
                    "severity": "medium",
                    "correlation": "service_outage",
                }),
            );
            changes.push(change);
        }

        // Check system alerts
        for alert in &alerts {
            if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&alert.data) {
                if let Some(alerts_count) = data.get("alerts_raised") {
                    let count = alerts_count.as_u64().unwrap_or(0);
                    if count > 0 {
                        let change = ChangeEvent::new(
                            "threat",
                            "system_health",
                            "system_alert",
                            json!({
                                "alert_count": count,
                                "severity": "medium",
                                "correlation": "system_monitoring",
                            }),
                        );
                        changes.push(change);
                    }
                }
            }
        }

        // Publish threat summary
        let known_count = self.state.lock().unwrap().known_threats.len();
        let summary = json!({
            "known_threats": known_count,
            "new_threats": changes.len(),
            "timestamp": time::OffsetDateTime::now_utc().to_string(),
        });
        kernel.event.publish(
            "monitoring.threat.summary",
            &serde_json::to_vec(&summary).unwrap(),
        );

        Ok(changes)
    }
}
