//! Package Watcher — monitors package ecosystems (npm, cargo, pypi) for new versions.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tordex_core::{Kernel, Result};

use crate::watcher::{ChangeEvent, Watcher};

struct PackageState {
    tracked: Vec<(String, String)>,
    last_versions: HashMap<String, String>,
}

/// Watches package ecosystems for new releases and version changes.
///
/// Uses: kernel.drivers.execute("http", "fetch_json", ...), kernel.objects
pub struct PackageWatcher {
    state: Arc<Mutex<PackageState>>,
}

impl PackageWatcher {
    #[must_use]
    pub fn new() -> Self {
        let mut tracked = Vec::new();
        // TORdex own dependencies worth watching
        tracked.push(("cargo".into(), "serde".into()));
        tracked.push(("cargo".into(), "tokio".into()));
        tracked.push(("npm".into(), "react".into()));
        tracked.push(("pypi".into(), "requests".into()));
        Self {
            state: Arc::new(Mutex::new(PackageState {
                tracked,
                last_versions: HashMap::new(),
            })),
        }
    }
}

impl Watcher for PackageWatcher {
    fn name(&self) -> &str {
        "package"
    }

    fn kind(&self) -> &str {
        "ecosystem"
    }

    fn description(&self) -> &str {
        "Monitors package ecosystems (npm, cargo, pypi) for new versions and releases"
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(3600) // 1 hour
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        let existing = kernel.objects.find_by_kind("monitored_package");
        let mut state = self.state.lock().unwrap();
        for obj in &existing {
            if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&obj.data) {
                let eco = data["ecosystem"].as_str().unwrap_or("unknown").to_string();
                let pkg = obj.label.clone();
                let _key = format!("{eco}:{pkg}");
                if !state.tracked.iter().any(|(e, p)| e == &eco && p == &pkg) {
                    state.tracked.push((eco, pkg));
                }
            }
        }
        // Record monitored packages as kernel objects
        for (eco, pkg) in &state.tracked {
            let label = format!("{eco}:{pkg}");
            if kernel.objects.find_by_label(&label).is_empty() {
                kernel.objects.create(
                    "monitored_package",
                    &pkg,
                    &serde_json::to_vec(&json!({"ecosystem": eco, "package": pkg})).unwrap(),
                );
            }
        }
        kernel.event.subscribe("monitoring.package");
        tracing::info!(count = state.tracked.len(), "package watcher initialized");
        Ok(())
    }

    fn poll(&self, kernel: &Kernel) -> Result<Vec<ChangeEvent>> {
        let mut changes = Vec::new();
        let tracked: Vec<(String, String)> = {
            let state = self.state.lock().unwrap();
            state.tracked.clone()
        };
        let has_http = !kernel.drivers.find_by_capability("fetch_json").is_empty();

        for (ecosystem, package) in &tracked {
            let registry_url = match ecosystem.as_str() {
                "npm" => format!("https://registry.npmjs.org/{package}/latest"),
                "cargo" => format!("https://crates.io/api/v1/crates/{package}"),
                "pypi" => format!("https://pypi.org/pypi/{package}/json"),
                _ => continue,
            };

            let result = if has_http {
                kernel.drivers.execute("http", "fetch_json", json!({"url": registry_url}))
            } else {
                Err(tordex_core::driver::DriverError::DriverNotFound("http".into()).into())
            };

            match result {
                Ok(resp) => {
                    let version = resp
                        .get("version")
                        .or_else(|| resp.get("crate").and_then(|c| c.get("max_version")))
                        .or_else(|| resp.get("info").and_then(|i| i.get("version")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let key = format!("{ecosystem}:{package}");
                    let prev_version = {
                        let s = self.state.lock().unwrap();
                        s.last_versions.get(&key).cloned()
                    };

                    {
                        let mut s = self.state.lock().unwrap();
                        s.last_versions.insert(key.clone(), version.clone());
                    }

                    let change_type = match prev_version {
                        Some(ref pv) if pv != &version => "version_changed",
                        Some(_) => "no_change",
                        None => "initial",
                    };

                    let change = ChangeEvent::new(
                        "package",
                        &key,
                        change_type,
                        json!({
                            "ecosystem": ecosystem,
                            "package": package,
                            "version": version,
                            "previous_version": prev_version,
                        }),
                    );
                    changes.push(change);
                }
                Err(e) => {
                    let key = format!("{ecosystem}:{package}");
                    let change = ChangeEvent::new(
                        "package",
                        &key,
                        "poll_failed",
                        json!({"ecosystem": ecosystem, "package": package, "error": e.to_string()}),
                    );
                    changes.push(change);
                }
            }
        }

        Ok(changes)
    }
}
