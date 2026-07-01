//! TORdex Monitoring Engine — continuous observation of changes.
//!
//! Watches repositories, onions, APIs, organizations, packages, CVEs, and threats.
//! Everything produces events through kernel APIs. No direct database access.

pub mod engine;
pub mod watcher;
pub mod watchers;

pub use engine::{MonitoringAgent, MonitoringEngine};
pub use watcher::{ChangeEvent, Watcher};

/// Register all domain watchers and the monitoring engine agent into the kernel.
pub fn register(kernel: &tordex_core::Kernel) -> tordex_core::Result<()> {
    let engine = std::sync::Arc::new(MonitoringEngine::new());
    watchers::register_all(&engine, kernel);
    engine.init_all(kernel)?;
    let agent = MonitoringAgent::new(engine);
    kernel.agents.register(Box::new(agent))?;
    tracing::info!("monitoring engine registered with 7 domain watchers");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tordex_core::Kernel;

    #[test]
    fn register_monitoring_engine() {
        let kernel = Kernel::new();
        register(&kernel).unwrap();

        let agents = kernel.agents.list();
        let has_monitoring = agents.iter().any(|a| a.name == "monitoring-engine");
        assert!(has_monitoring, "monitoring engine agent should be registered");
    }

    #[test]
    fn monitoring_engine_poll_due() {
        let kernel = Kernel::new();
        let engine = MonitoringEngine::new();

        // Register a mock test watcher that produces changes
        use std::time::Duration;
        struct QuickWatcher;
        impl Watcher for QuickWatcher {
            fn name(&self) -> &str { "quick" }
            fn kind(&self) -> &str { "test" }
            fn description(&self) -> &str { "quick test watcher" }
            fn poll_interval(&self) -> Duration { Duration::from_millis(1) }
            fn poll(&self, _kernel: &Kernel) -> tordex_core::Result<Vec<ChangeEvent>> {
                Ok(vec![ChangeEvent::new("test", "quick-subject", "detected", serde_json::json!({"val": 1}))])
            }
        }

        engine.register(Box::new(QuickWatcher));
        engine.init_all(&kernel).unwrap();

        let count = engine.poll_due(&kernel).unwrap();
        assert_eq!(count, 1, "should detect 1 change");

        // Verify the change was stored via kernel APIs
        let changes = kernel.objects.find_by_kind("monitoring_change");
        assert_eq!(changes.len(), 1, "change should be persisted as kernel object");

        // Verify event was published
        let events = kernel.event_store.read_all("Monitoring").unwrap();
        assert_eq!(events.len(), 1, "change should be in event store");
    }
}
