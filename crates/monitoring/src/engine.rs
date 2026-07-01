//! Monitoring Engine — drives watchers on their poll intervals.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::json;
use tordex_core::event_store::SystemEvent;
use tordex_core::{Agent, AgentId, AgentManifest, AgentStatus, Kernel, Result};
use tracing;

use crate::watcher::{ChangeEvent, Watcher};

/// Tracks when each watcher was last polled.
struct WatcherEntry {
    watcher: Box<dyn Watcher>,
    last_poll: Option<Instant>,
}

/// The monitoring engine owns all watchers and drives their poll cycles.
pub struct MonitoringEngine {
    watchers: Arc<Mutex<Vec<WatcherEntry>>>,
    active: Arc<AtomicBool>,
    handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl MonitoringEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(Mutex::new(Vec::new())),
            active: Arc::new(AtomicBool::new(false)),
            handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register a watcher.
    pub fn register(&self, watcher: Box<dyn Watcher>) {
        let mut inner = self.watchers.lock().unwrap();
        inner.push(WatcherEntry {
            watcher,
            last_poll: None,
        });
    }

    /// Register multiple watchers at once.
    pub fn register_all(&self, watchers: Vec<Box<dyn Watcher>>) {
        let mut inner = self.watchers.lock().unwrap();
        for w in watchers {
            inner.push(WatcherEntry {
                watcher: w,
                last_poll: None,
            });
        }
    }

    /// Initialize all watchers (called before start).
    pub fn init_all(&self, kernel: &Kernel) -> Result<()> {
        let inner = self.watchers.lock().unwrap();
        for entry in inner.iter() {
            entry.watcher.init(kernel)?;
        }
        Ok(())
    }

    /// Poll all watchers whose interval has elapsed. Returns total changes detected.
    pub fn poll_due(&self, kernel: &Kernel) -> Result<u64> {
        let mut total = 0u64;
        let mut due: Vec<usize> = Vec::new();

        // Determine which watchers are due
        {
            let inner = self.watchers.lock().unwrap();
            for (i, entry) in inner.iter().enumerate() {
                let should_poll = match entry.last_poll {
                    None => true,
                    Some(last) => last.elapsed() >= entry.watcher.poll_interval(),
                };
                if should_poll {
                    due.push(i);
                }
            }
        }

        // Poll due watchers (outside the lock to avoid deadlocks)
        for &idx in &due {
            let changes = {
                let mut inner = self.watchers.lock().unwrap();
                if idx >= inner.len() {
                    continue;
                }
                let entry = &mut inner[idx];
                entry.last_poll = Some(Instant::now());
                let name = entry.watcher.name().to_string();
                match entry.watcher.poll(kernel) {
                    Ok(changes) => changes,
                    Err(e) => {
                        tracing::warn!(watcher = %name, error = %e, "poll failed");
                        Vec::new()
                    }
                }
            };

            // Process detected changes through kernel APIs
            for change in &changes {
                emit_change(kernel, change)?;
                total += 1;
            }
        }

        Ok(total)
    }

    /// Start background threads that continuously poll watchers.
    pub fn start_background(&self, kernel: Arc<Kernel>) -> Result<()> {
        self.active.store(true, Ordering::SeqCst);
        let active = self.active.clone();
        let watchers = self.watchers.clone();

        let handle = std::thread::spawn(move || {
            while active.load(Ordering::SeqCst) {
                let mut total = 0u64;
                let mut due: Vec<usize> = Vec::new();
                {
                    let inner = watchers.lock().unwrap();
                    for (i, entry) in inner.iter().enumerate() {
                        let should_poll = match entry.last_poll {
                            None => true,
                            Some(last) => last.elapsed() >= entry.watcher.poll_interval(),
                        };
                        if should_poll {
                            due.push(i);
                        }
                    }
                }
                for &idx in &due {
                    let changes = {
                        let mut inner = watchers.lock().unwrap();
                        if idx >= inner.len() {
                            continue;
                        }
                        let entry = &mut inner[idx];
                        entry.last_poll = Some(Instant::now());
                        let name = entry.watcher.name().to_string();
                        match entry.watcher.poll(&kernel) {
                            Ok(changes) => changes,
                            Err(e) => {
                                tracing::warn!(watcher = %name, error = %e, "background poll failed");
                                Vec::new()
                            }
                        }
                    };
                    for change in &changes {
                        if let Err(e) = emit_change(&kernel, change) {
                            tracing::warn!(error = %e, "failed to emit change event");
                        }
                        total += 1;
                    }
                }
                if total > 0 {
                    tracing::info!(changes = total, "monitoring engine processed changes");
                }
                std::thread::sleep(Duration::from_secs(5));
            }
        });

        self.handles.lock().unwrap().push(handle);
        Ok(())
    }

    /// Stop background threads.
    pub fn stop_background(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    /// Number of registered watchers.
    pub fn watcher_count(&self) -> usize {
        self.watchers.lock().unwrap().len()
    }

    /// List registered watchers.
    pub fn list_watchers(&self) -> Vec<(String, String, String)> {
        self.watchers
            .lock()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e.watcher.name().to_string(),
                    e.watcher.kind().to_string(),
                    e.watcher.description().to_string(),
                )
            })
            .collect()
    }
}

impl Default for MonitoringEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Emit a change event through kernel APIs: event_store, objects, event bus.
fn emit_change(kernel: &Kernel, change: &ChangeEvent) -> Result<()> {
    // 1. Record as a SystemEvent in the event store
    let system_event = SystemEvent::MonitoringChangeDetected {
        id: change.id.clone(),
        watcher_kind: change.watcher_kind.clone(),
        subject: change.subject.clone(),
        change_type: change.change_type.clone(),
        previous_state: change.previous_state.clone(),
        current_state: change.current_state.clone(),
        detected_at: change.detected_at,
    };
    let version = kernel.event_store.latest_version(&change.id) + 1;
    let envelope = system_event.into_envelope(version);
    kernel.event_store.append(envelope)?;

    // 2. Store as a kernel object for queryability
    let obj_data = json!({
        "id": change.id,
        "watcher_kind": change.watcher_kind,
        "subject": change.subject,
        "change_type": change.change_type,
        "previous_state": change.previous_state,
        "current_state": change.current_state,
        "detected_at": change.detected_at.to_string(),
        "metadata": change.metadata,
    });
    kernel.objects.create(
        "monitoring_change",
        &format!("change-{}", change.id),
        &serde_json::to_vec(&obj_data).unwrap(),
    );

    // 3. Publish to event bus for real-time subscribers
    kernel.event.publish(
        &format!("monitoring.{}", change.watcher_kind),
        &serde_json::to_vec(&obj_data).unwrap(),
    );

    Ok(())
}

/// Agent wrapper that drives the monitoring engine through the agent tick cycle.
pub struct MonitoringAgent {
    id: AgentId,
    engine: Arc<MonitoringEngine>,
}

impl MonitoringAgent {
    #[must_use]
    pub fn new(engine: Arc<MonitoringEngine>) -> Self {
        Self {
            id: AgentId::new(),
            engine,
        }
    }
}

impl Agent for MonitoringAgent {
    fn manifest(&self) -> AgentManifest {
        AgentManifest {
            id: self.id,
            name: "monitoring-engine".into(),
            kind: "monitoring".into(),
            version: "0.1.0".into(),
            description: "Drives the monitoring engine poll cycle, watching repositories, onions, APIs, organizations, packages, CVEs, and threats".into(),
            status: AgentStatus::Idle,
        }
    }

    fn tick(&self, kernel: &Kernel) -> Result<()> {
        let count = self.engine.poll_due(kernel)?;
        if count > 0 {
            tracing::info!(changes = count, "monitoring engine detected changes");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::ChangeEvent;
    use std::sync::Arc;
    use tordex_core::Kernel;

    struct TestWatcher;

    impl Watcher for TestWatcher {
        fn name(&self) -> &str {
            "test-watcher"
        }
        fn kind(&self) -> &str {
            "test"
        }
        fn description(&self) -> &str {
            "test watcher"
        }
        fn poll_interval(&self) -> Duration {
            Duration::from_millis(10)
        }
        fn poll(&self, _kernel: &Kernel) -> Result<Vec<ChangeEvent>> {
            Ok(vec![ChangeEvent::new("test", "test-subject", "updated", json!({"key": "value"}))])
        }
    }

    #[test]
    fn engine_register_and_poll() {
        let kernel = Kernel::new();
        let engine = MonitoringEngine::new();
        engine.register(Box::new(TestWatcher));
        assert_eq!(engine.watcher_count(), 1);

        let count = engine.poll_due(&kernel).unwrap();
        assert_eq!(count, 1);

        // Verify the change was emitted via kernel APIs
        let changes = kernel.objects.find_by_kind("monitoring_change");
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn engine_init_all() {
        let kernel = Kernel::new();
        let engine = MonitoringEngine::new();
        engine.register(Box::new(TestWatcher));
        engine.init_all(&kernel).unwrap();
        // Just verify no panic
    }

    #[test]
    fn engine_list_watchers() {
        let engine = MonitoringEngine::new();
        engine.register(Box::new(TestWatcher));
        let list = engine.list_watchers();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "test-watcher");
    }

    #[test]
    fn monitoring_agent_tick() {
        let kernel = Kernel::new();
        let engine = Arc::new(MonitoringEngine::new());
        engine.register(Box::new(TestWatcher));
        let agent = MonitoringAgent::new(engine);
        agent.tick(&kernel).unwrap();

        let changes = kernel.objects.find_by_kind("monitoring_change");
        assert_eq!(changes.len(), 1);
    }
}
