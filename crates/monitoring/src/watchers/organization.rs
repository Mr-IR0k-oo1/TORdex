//! Organization Watcher — monitors organizations and entities for changes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tordex_core::{Kernel, Result};

use crate::watcher::{ChangeEvent, Watcher};

struct OrgState {
    tracked: Vec<String>,
}

/// Watches organizations and entities for changes via kernel entity objects.
///
/// Uses: kernel.objects, kernel.drivers (DNS), kernel.event_store
pub struct OrganizationWatcher {
    state: Arc<Mutex<OrgState>>,
}

impl OrganizationWatcher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(OrgState {
                tracked: Vec::new(),
            })),
        }
    }
}

impl Watcher for OrganizationWatcher {
    fn name(&self) -> &str {
        "organization"
    }

    fn kind(&self) -> &str {
        "entity"
    }

    fn description(&self) -> &str {
        "Monitors organizations and entities for attribute and relationship changes"
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(3600) // 1 hour
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        // Discover organizations from kernel entity objects
        let entities = kernel.objects.find_by_kind("entity");
        let mut state = self.state.lock().unwrap();
        for obj in &entities {
            state.tracked.push(obj.label.clone());
        }
        // Also look for direct organization objects
        let orgs = kernel.objects.find_by_kind("organization");
        for obj in &orgs {
            if !state.tracked.contains(&obj.label) {
                state.tracked.push(obj.label.clone());
            }
        }
        kernel.event.subscribe("monitoring.organization");
        tracing::info!(count = state.tracked.len(), "organization watcher initialized");
        Ok(())
    }

    fn poll(&self, kernel: &Kernel) -> Result<Vec<ChangeEvent>> {
        let mut changes = Vec::new();
        let tracked: Vec<String> = {
            let state = self.state.lock().unwrap();
            state.tracked.clone()
        };

        // Read all entities from the event store for change detection
        let entity_events = kernel.event_store.read_all("Entity").unwrap_or_default();
        let entity_count = entity_events.len();

        // Read all relationship events
        let rel_events = kernel.event_store.read_all("Relationship").unwrap_or_default();
        let rel_count = rel_events.len();

        // Summarize current entity landscape
        let current = json!({
            "entity_events": entity_count,
            "relationship_events": rel_count,
            "tracked_organizations": tracked.len(),
        });

        // Check against previous state in kernel objects
        let prev_objects = kernel.objects.find_by_kind("organization_snapshot");
        let prev = prev_objects.first().map(|o| {
            serde_json::from_slice::<serde_json::Value>(&o.data).unwrap_or_default()
        });

        let change_type = match prev {
            Some(ref p) if p == &current => "no_change",
            Some(_) => "updated",
            None => "initial",
        };

        // Store current snapshot
        kernel.objects.create(
            "organization_snapshot",
            &format!("org-snapshot-{}", ulid::Ulid::new()),
            &serde_json::to_vec(&current).unwrap(),
        );

        let change = ChangeEvent::new(
            "organization",
            "entity_landscape",
            change_type,
            current,
        );
        changes.push(change);

        // Also check each tracked org via DNS if available
        for org in &tracked {
            let has_dns = !kernel.drivers.find_by_capability("resolve_a").is_empty();
            if has_dns {
                match kernel.drivers.execute("dns", "resolve_a", json!({"name": org})) {
                    Ok(records) => {
                        let change = ChangeEvent::new(
                            "organization",
                            org,
                            "dns_resolved",
                            json!({"name": org, "records": records}),
                        );
                        changes.push(change);
                    }
                    Err(e) => {
                        let change = ChangeEvent::new(
                            "organization",
                            org,
                            "dns_failed",
                            json!({"name": org, "error": e.to_string()}),
                        );
                        changes.push(change);
                    }
                }
            }
        }

        Ok(changes)
    }
}
