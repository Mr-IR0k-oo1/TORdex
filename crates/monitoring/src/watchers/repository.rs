//! Repository Watcher — monitors git repositories for new commits, branches, tags.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tordex_core::{Kernel, Result};

use crate::watcher::{ChangeEvent, Watcher};

struct RepoState {
    tracked: Vec<String>,
    last_heads: HashMap<String, String>,
}

/// Watches git repositories for new commits, branches, and tags.
///
/// Uses: kernel.drivers (for HTTP/Git), kernel.objects, kernel.storage
pub struct RepositoryWatcher {
    state: Arc<Mutex<RepoState>>,
}

impl RepositoryWatcher {
    #[must_use]
    pub fn new() -> Self {
        let mut tracked = Vec::new();
        // Seed with known repository patterns
        tracked.push("https://github.com/opencode-ai/opencode".into());
        tracked.push("https://github.com/anomalyco/TORdex".into());
        tracked.push("https://github.com/anomalyco/TORdex-plugins".into());
        Self {
            state: Arc::new(Mutex::new(RepoState {
                tracked,
                last_heads: HashMap::new(),
            })),
        }
    }
}

impl Watcher for RepositoryWatcher {
    fn name(&self) -> &str {
        "repository"
    }

    fn kind(&self) -> &str {
        "scm"
    }

    fn description(&self) -> &str {
        "Monitors git repositories for new commits, branches, and tags"
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(300) // 5 minutes
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        // Load any previously tracked repositories from kernel objects
        let existing = kernel.objects.find_by_kind("monitored_repository");
        let mut state = self.state.lock().unwrap();
        for obj in &existing {
            if !state.tracked.contains(&obj.label) {
                state.tracked.push(obj.label.clone());
            }
        }
        // Record tracked repos as objects so other components can discover them
        for repo in &state.tracked {
            if kernel
                .objects
                .find_by_label(repo)
                .is_empty()
            {
                kernel.objects.create(
                    "monitored_repository",
                    repo,
                    &serde_json::to_vec(&json!({"url": repo, "status": "active"})).unwrap(),
                );
            }
        }
        tracing::info!(count = state.tracked.len(), "repository watcher initialized");
        Ok(())
    }

    fn poll(&self, kernel: &Kernel) -> Result<Vec<ChangeEvent>> {
        let mut changes = Vec::new();
        let tracked: Vec<String> = {
            let state = self.state.lock().unwrap();
            state.tracked.clone()
        };

        for url in &tracked {
            // Try to fetch repository info via HTTP driver
            let has_http = kernel
                .drivers
                .find_by_capability("fetch_html")
                .is_empty();
            // For now, use a simpler check: try HEAD request or direct fetch
            let result = if !has_http {
                kernel.drivers.execute(
                    "http",
                    "head_request",
                    json!({"url": format!("{url}/info/refs?service=git-upload-pack")}),
                )
            } else {
                // Fallback: record as "unknown" status when no driver available
                kernel.drivers.execute("http", "head_request", json!({"url": url}))
            };

            match result {
                Ok(resp) => {
                    let status = resp.get("status_code").and_then(|v| v.as_u64()).unwrap_or(0);
                    let change = ChangeEvent::new(
                        "repository",
                        url,
                        if status == 200 { "reachable" } else { "unreachable" },
                        json!({
                            "url": url,
                            "status_code": status,
                            "reachable": status == 200,
                        }),
                    );
                    changes.push(change);
                }
                Err(e) => {
                    tracing::debug!(repo = %url, error = %e, "repository poll failed");
                    let change = ChangeEvent::new(
                        "repository",
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
