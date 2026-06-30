//! Collection Fabric — the orchestrator.
//!
//! Ties together discovery, priority queues, drivers, sessions, and event
//! emission into a unified collection pipeline:
//!
//! ```text
//! submit(task) → PriorityQueue → process_next() → Driver.execute() → Session.transition()
//!                                                      │
//!                                                      ▼
//!                                               EventStore.append()
//! ```

use std::sync::Arc;

use time::OffsetDateTime;
use tracing::info;

use serde_json::Value;
use tordex_core::driver::DriverRegistry;
use tordex_core::event_store::{EventStore, SystemEvent};

use crate::queue::{CollectionTask, CollectionTarget, FabricError, Priority, PriorityQueue};
use crate::session::{CollectionSession, SessionManager, SessionState};

/// The collection fabric orchestrator.
///
/// Coordinates task submission → queue → driver dispatch → session tracking
/// → event emission.
pub struct CollectionFabric {
    queue: PriorityQueue,
    sessions: SessionManager,
    drivers: Arc<dyn DriverRegistry>,
    events: Arc<dyn EventStore>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl CollectionFabric {
    pub fn new(
        drivers: Arc<dyn DriverRegistry>,
        events: Arc<dyn EventStore>,
    ) -> Self {
        Self {
            queue: PriorityQueue::new(),
            sessions: SessionManager::new(),
            drivers,
            events,
            running: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    /// Submit a collection task to the fabric.
    ///
    /// Returns the task ID. Creates a session in Queued state.
    pub fn submit(&self, task: CollectionTask) -> Result<String, FabricError> {
        let id = task.id.clone();
        let target_desc = match &task.target {
            CollectionTarget::Url(u) => u.clone(),
            CollectionTarget::Domain(d) => d.clone(),
            CollectionTarget::Service { locator, .. } => locator.clone(),
            CollectionTarget::Custom { locator, .. } => locator.clone(),
        };

        self.sessions.create(
            &id,
            &target_desc,
            &task.driver,
            &task.capability,
            task.max_retries,
            task.metadata.clone(),
        );

        self.queue.enqueue(task)?;

        // Emit queued event
        let event = SystemEvent::ServiceRegistered {
            id: id.clone(),
            name: format!("collection:{}", &target_desc),
            kind: "collection".into(),
            locator: target_desc,
            status: "active".into(),
            version: None,
        };
        let version = self.events.latest_version(&id) + 1;
        self.events.append(event.into_envelope(version)).ok();

        info!(task_id = %id, "task submitted");
        Ok(id)
    }

    /// Cancel a task by ID. Removes from queue or marks running sessions.
    pub fn cancel(&self, task_id: &str) -> Result<(), FabricError> {
        // Try to remove from queue first
        if self.queue.cancel(task_id).is_ok() {
            self.sessions
                .transition(task_id, SessionState::Cancelled, None, None)?;
            return Ok(());
        }

        // Otherwise transition running session to cancelled
        if let Some(_session) = self.sessions.get(task_id) {
            self.sessions
                .transition(task_id, SessionState::Cancelled, None, None)?;
            return Ok(());
        }

        Err(FabricError::TaskNotFound(task_id.to_string()))
    }

    /// Process the next task from the queue.
    ///
    /// 1. Dequeues highest-priority task
    /// 2. Transitions session to Running
    /// 3. Dispatches to the appropriate driver
    /// 4. Transitions session to Succeeded or Failed
    /// 5. Emits events for each step
    ///
    /// Returns the task ID that was processed, or `None` if the queue is empty.
    pub fn process_next(&self) -> Result<Option<String>, FabricError> {
        if !self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(FabricError::Stopped);
        }

        let task = match self.queue.dequeue() {
            Some(t) => t,
            None => return Ok(None),
        };

        let task_id = task.id.clone();
        let _target_str = task.target.locator().to_string();

        // Mark running
        self.sessions
            .transition(&task_id, SessionState::Running, None, None)?;

        // Emit running event
        let event = SystemEvent::ServiceUpdated {
            id: task_id.clone(),
            name: None,
            status: Some("active".into()),
            version: None,
        };
        let version = self.events.latest_version(&task_id) + 1;
        self.events.append(event.into_envelope(version)).ok();

        // Dispatch to driver
        let result = self.drivers.execute(&task.driver, &task.capability, task.params.clone());

        match result {
            Ok(output) => {
                self.sessions.transition(
                    &task_id,
                    SessionState::Succeeded,
                    Some(output.clone()),
                    None,
                )?;

                // Emit success event
                let obs_event = SystemEvent::ObservationRecorded {
                    id: format!("obs_{task_id}"),
                    kind: format!("collection.{}", &task.capability),
                    data: serde_json::to_vec(&output).unwrap_or_default(),
                    content_type: Some("application/json".into()),
                    source: format!("driver:{}", &task.driver),
                    observed_at: OffsetDateTime::now_utc(),
                };
                let version = self.events.latest_version(&task_id) + 1;
                self.events.append(obs_event.into_envelope(version)).ok();

                info!(task_id = %task_id, driver = %task.driver, capability = %task.capability, "collection succeeded");
                Ok(Some(task_id))
            }
            Err(e) => {
                let err_msg = e.to_string();
                self.sessions.transition(
                    &task_id,
                    SessionState::Failed,
                    None,
                    Some(err_msg.clone()),
                )?;

                // Check for retry
                if self.sessions.should_retry(&task_id) {
                    // Re-enqueue at next priority level down
                    let demoted = match task.priority {
                        Priority::Critical => Priority::High,
                        Priority::High => Priority::Medium,
                        Priority::Medium => Priority::Low,
                        Priority::Low => Priority::Background,
                        Priority::Background => Priority::Background,
                    };
                    let mut retry_task = task;
                    retry_task.priority = demoted;
                    self.queue.enqueue(retry_task).ok();
                    info!(task_id = %task_id, "scheduled retry");
                }

                // Emit failure event
                let fail_event = SystemEvent::ServiceUpdated {
                    id: task_id.clone(),
                    name: None,
                    status: Some("error".into()),
                    version: None,
                };
                let version = self.events.latest_version(&task_id) + 1;
                self.events.append(fail_event.into_envelope(version)).ok();

                info!(task_id = %task_id, error = %err_msg, "collection failed");
                Ok(Some(task_id))
            }
        }
    }

    /// Get session status for a task.
    #[must_use]
    pub fn status(&self, task_id: &str) -> Option<CollectionSession> {
        self.sessions.get(task_id)
    }

    /// List all sessions with optional state filter.
    #[must_use]
    pub fn list_sessions(&self, filter: Option<SessionState>) -> Vec<CollectionSession> {
        self.sessions.list(filter)
    }

    /// Queue depth.
    #[must_use]
    pub fn queue_depth(&self) -> usize {
        self.queue.len()
    }

    /// List queued tasks.
    #[must_use]
    pub fn queued_tasks(&self) -> Vec<CollectionTask> {
        self.queue.list()
    }

    /// Session counts by state.
    #[must_use]
    pub fn session_counts(&self) -> std::collections::HashMap<SessionState, usize> {
        self.sessions.counts()
    }

    /// Stop processing.
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Start (restart) processing.
    pub fn start(&self) {
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Process all tasks currently in the queue.
    ///
    /// Returns the number of tasks processed.
    pub fn drain(&self) -> usize {
        let mut count = 0;
        while let Ok(Some(_)) = self.process_next() {
            count += 1;
        }
        count
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use serde_json::json;
    use tordex_core::driver::InMemoryDriverRegistry;
    use tordex_core::event_store::InMemoryEventStore;
    use tordex_core::driver::{Driver, Capability, DriverError};
    use crate::queue::CollectionTarget;

    /// A test driver that always succeeds.
    struct OkDriver;

    impl Driver for OkDriver {
        fn name(&self) -> &str { "ok-driver" }
        fn description(&self) -> &str { "always succeeds" }
        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::new("fetch", "fetch", json!({}), json!({}))]
        }
        fn execute(&self, _cap: &str, _params: Value) -> Result<Value, DriverError> {
            Ok(json!({"status": 200, "body": "ok"}))
        }
    }

    /// A test driver that always fails.
    struct FailDriver;

    impl Driver for FailDriver {
        fn name(&self) -> &str { "fail-driver" }
        fn description(&self) -> &str { "always fails" }
        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::new("fetch", "fetch", json!({}), json!({}))]
        }
        fn execute(&self, _cap: &str, _params: Value) -> Result<Value, DriverError> {
            Err(DriverError::Execution("simulated failure".into()))
        }
    }

    fn test_fabric(ok: bool) -> CollectionFabric {
        let drivers = Arc::new(InMemoryDriverRegistry::new());
        if ok {
            drivers.register(Box::new(OkDriver)).unwrap();
        } else {
            drivers.register(Box::new(FailDriver)).unwrap();
        }
        let events = Arc::new(InMemoryEventStore::new());
        CollectionFabric::new(drivers, events)
    }

    #[test]
    fn submit_and_process() {
        let fabric = test_fabric(true);
        let task = CollectionTask::new(
            "t1".into(),
            CollectionTarget::Url("https://example.com".into()),
            "ok-driver",
            "fetch",
            json!({}),
        );
        let id = fabric.submit(task).unwrap();
        assert_eq!(id, "t1");

        let processed = fabric.process_next().unwrap();
        assert_eq!(processed, Some("t1".into()));

        let session = fabric.status("t1").unwrap();
        assert_eq!(session.state, SessionState::Succeeded);
    }

    #[test]
    fn submit_and_fail_with_retry() {
        let fabric = test_fabric(false);
        let task = CollectionTask::new(
            "t2".into(),
            CollectionTarget::Url("https://fail.example".into()),
            "fail-driver",
            "fetch",
            json!({}),
        )
        .with_max_retries(2);
        fabric.submit(task).unwrap();

        let processed = fabric.process_next().unwrap();
        assert_eq!(processed, Some("t2".into()));

        // Should be failed but retry re-enqueued
        let session = fabric.status("t2").unwrap();
        assert_eq!(session.state, SessionState::Failed);
        assert!(session.error.is_some());

        // Should have been re-enqueued at lower priority
        assert_eq!(fabric.queue_depth(), 1);
    }

    #[test]
    fn drain_processes_all() {
        let fabric = test_fabric(true);
        for i in 0..5 {
            let task = CollectionTask::new(
                format!("t{i}"),
                CollectionTarget::Url("https://example.com".into()),
                "ok-driver",
                "fetch",
                json!({}),
            );
            fabric.submit(task).unwrap();
        }

        let count = fabric.drain();
        assert_eq!(count, 5);

        // All sessions should be succeeded
        let succeeded = fabric.list_sessions(Some(SessionState::Succeeded));
        assert_eq!(succeeded.len(), 5);
        assert_eq!(fabric.queue_depth(), 0);
    }

    #[test]
    fn cancel_queued_task() {
        let fabric = test_fabric(true);
        let task = CollectionTask::new(
            "cancel-me".into(),
            CollectionTarget::Url("https://example.com".into()),
            "ok-driver",
            "fetch",
            json!({}),
        );
        fabric.submit(task).unwrap();
        fabric.cancel("cancel-me").unwrap();

        let session = fabric.status("cancel-me").unwrap();
        assert_eq!(session.state, SessionState::Cancelled);
    }

    #[test]
    fn stop_prevents_processing() {
        let fabric = test_fabric(true);
        fabric.stop();
        let task = CollectionTask::new(
            "stop-test".into(),
            CollectionTarget::Url("https://example.com".into()),
            "ok-driver",
            "fetch",
            json!({}),
        );
        fabric.submit(task).unwrap();
        let err = fabric.process_next().unwrap_err();
        assert!(matches!(err, FabricError::Stopped));
    }

    #[test]
    fn empty_queue_returns_none() {
        let fabric = test_fabric(true);
        let result = fabric.process_next().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn session_counts_are_accurate() {
        let fabric = test_fabric(true);
        for i in 0..3 {
            let task = CollectionTask::new(
                format!("t{i}"),
                CollectionTarget::Url("https://example.com".into()),
                "ok-driver",
                "fetch",
                json!({}),
            );
            fabric.submit(task).unwrap();
        }
        fabric.drain();

        let counts = fabric.session_counts();
        assert_eq!(counts.get(&SessionState::Succeeded), Some(&3));
    }

    #[test]
    fn priority_queue_order_respected() {
        let fabric = test_fabric(true);

        // Submit low priority first, critical second
        let low = CollectionTask::new(
            "low-pri".into(),
            CollectionTarget::Url("https://low.example".into()),
            "ok-driver",
            "fetch",
            json!({}),
        )
        .with_priority(Priority::Low);
        fabric.submit(low).unwrap();

        let critical = CollectionTask::new(
            "critical-pri".into(),
            CollectionTarget::Url("https://critical.example".into()),
            "ok-driver",
            "fetch",
            json!({}),
        )
        .with_priority(Priority::Critical);
        fabric.submit(critical).unwrap();

        let first = fabric.process_next().unwrap();
        assert_eq!(first, Some("critical-pri".into()));
    }
}
