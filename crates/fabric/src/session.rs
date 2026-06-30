//! Collection session state machine.
//!
//! Each collection task produces a session that tracks the lifecycle:
//! ```text
//! Queued → Running → Succeeded
//!                   → Failed → Queued (retry)
//!                   → Cancelled
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::queue::FabricError;

// ─── SessionState ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

// ─── CollectionSession ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSession {
    pub task_id: String,
    pub target: String,
    pub driver: String,
    pub capability: String,
    pub state: SessionState,
    pub attempt: u32,
    pub max_retries: u32,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub queued_at: OffsetDateTime,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub metadata: HashMap<String, String>,
}

/// All valid state transitions.
const fn allowed_transition(from: SessionState, to: SessionState) -> bool {
    matches!(
        (from, to),
        (SessionState::Queued, SessionState::Running | SessionState::Cancelled) |
(SessionState::Running,
SessionState::Succeeded | SessionState::Failed | SessionState::Cancelled) |
(SessionState::Failed, SessionState::Queued | SessionState::Cancelled)
    )
}

// ─── SessionManager ──────────────────────────────────────────────────────────

/// Manages collection session state transitions.
pub struct SessionManager {
    inner: Arc<Mutex<HashMap<String, CollectionSession>>>,
}

impl SessionManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new session in the Queued state.
    pub fn create(
        &self,
        task_id: &str,
        target: &str,
        driver: &str,
        capability: &str,
        max_retries: u32,
        metadata: HashMap<String, String>,
    ) -> CollectionSession {
        let mut inner = self.inner.lock().unwrap();
        let session = CollectionSession {
            task_id: task_id.to_string(),
            target: target.to_string(),
            driver: driver.to_string(),
            capability: capability.to_string(),
            state: SessionState::Queued,
            attempt: 0,
            max_retries,
            result: None,
            error: None,
            queued_at: OffsetDateTime::now_utc(),
            started_at: None,
            completed_at: None,
            metadata,
        };
        inner.insert(task_id.to_string(), session.clone());
        session
    }

    /// Transition a session to a new state.
    pub fn transition(
        &self,
        task_id: &str,
        to: SessionState,
        result: Option<serde_json::Value>,
        error: Option<String>,
    ) -> Result<CollectionSession, FabricError> {
        let mut inner = self.inner.lock().unwrap();
        let session = inner
            .get_mut(task_id)
            .ok_or_else(|| FabricError::TaskNotFound(task_id.to_string()))?;

        if !allowed_transition(session.state, to) {
            return Err(FabricError::Session(format!(
                "invalid transition: {:?} → {:?}",
                session.state, to
            )));
        }

        session.state = to;
        if result.is_some() {
            session.result = result;
        }
        if error.is_some() {
            session.error = error;
        }

        match session.state {
            SessionState::Running => {
                session.attempt += 1;
                session.started_at = Some(OffsetDateTime::now_utc());
            }
            SessionState::Succeeded | SessionState::Failed | SessionState::Cancelled => {
                session.completed_at = Some(OffsetDateTime::now_utc());
            }
            _ => {}
        }

        Ok(session.clone())
    }

    /// Get a session by task ID.
    #[must_use]
    pub fn get(&self, task_id: &str) -> Option<CollectionSession> {
        self.inner.lock().unwrap().get(task_id).cloned()
    }

    /// List all sessions with an optional state filter.
    #[must_use]
    pub fn list(&self, state_filter: Option<SessionState>) -> Vec<CollectionSession> {
        let inner = self.inner.lock().unwrap();
        inner
            .values()
            .filter(|s| {
                if let Some(ref filter) = state_filter {
                    s.state == *filter
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    /// Count sessions in each state.
    #[must_use]
    pub fn counts(&self) -> HashMap<SessionState, usize> {
        let inner = self.inner.lock().unwrap();
        let mut counts = HashMap::new();
        for session in inner.values() {
            *counts.entry(session.state).or_insert(0) += 1;
        }
        counts
    }

    /// Total session count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether a session should be retried (failed + attempts < `max_retries`).
    #[must_use]
    pub fn should_retry(&self, task_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .get(task_id)
            .is_some_and(|s| {
                s.state == SessionState::Failed && s.attempt < s.max_retries
            })
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mgr() -> SessionManager {
        SessionManager::new()
    }

    fn make_session(mgr: &SessionManager, id: &str) {
        mgr.create(id, "https://example.com", "http", "fetch", 3, HashMap::new());
    }

    #[test]
    fn create_and_get() {
        let mgr = make_mgr();
        mgr.create("t1", "https://example.com", "http", "fetch", 3, HashMap::new());
        let s = mgr.get("t1").unwrap();
        assert_eq!(s.state, SessionState::Queued);
        assert_eq!(s.target, "https://example.com");
    }

    #[test]
    fn transition_queued_to_running() {
        let mgr = make_mgr();
        make_session(&mgr, "t1");
        let s = mgr.transition("t1", SessionState::Running, None, None).unwrap();
        assert_eq!(s.state, SessionState::Running);
        assert_eq!(s.attempt, 1);
        assert!(s.started_at.is_some());
    }

    #[test]
    fn transition_running_to_succeeded() {
        let mgr = make_mgr();
        make_session(&mgr, "t1");
        mgr.transition("t1", SessionState::Running, None, None).unwrap();
        let result = serde_json::json!({"status": 200});
        let s = mgr
            .transition("t1", SessionState::Succeeded, Some(result.clone()), None)
            .unwrap();
        assert_eq!(s.state, SessionState::Succeeded);
        assert_eq!(s.result, Some(result));
        assert!(s.completed_at.is_some());
    }

    #[test]
    fn transition_running_to_failed() {
        let mgr = make_mgr();
        make_session(&mgr, "t1");
        mgr.transition("t1", SessionState::Running, None, None).unwrap();
        let s = mgr
            .transition("t1", SessionState::Failed, None, Some("timeout".into()))
            .unwrap();
        assert_eq!(s.state, SessionState::Failed);
        assert_eq!(s.error, Some("timeout".into()));
    }

    #[test]
    fn retry_rejects_when_not_failed() {
        let mgr = make_mgr();
        make_session(&mgr, "t1");
        assert!(!mgr.should_retry("t1"));
    }

    #[test]
    fn retry_when_failed_and_attempts_remain() {
        let mgr = make_mgr();
        let session = mgr.create("t1", "url", "http", "fetch", 3, HashMap::new());
        mgr.transition("t1", SessionState::Running, None, None).unwrap();
        mgr.transition("t1", SessionState::Failed, None, Some("err".into())).unwrap();
        assert!(mgr.should_retry("t1"));
    }

    #[test]
    fn no_retry_when_max_retries_exhausted() {
        let mgr = make_mgr();
        mgr.create("t1", "url", "http", "fetch", 0, HashMap::new());
        mgr.transition("t1", SessionState::Running, None, None).unwrap();
        mgr.transition("t1", SessionState::Failed, None, Some("err".into())).unwrap();
        assert!(!mgr.should_retry("t1"));
    }

    #[test]
    fn invalid_transition_errors() {
        let mgr = make_mgr();
        make_session(&mgr, "t1");
        let err = mgr
            .transition("t1", SessionState::Succeeded, None, None)
            .unwrap_err();
        assert!(matches!(err, FabricError::Session(_)));
    }

    #[test]
    fn get_missing_returns_none() {
        let mgr = make_mgr();
        assert!(mgr.get("nonexistent").is_none());
    }

    #[test]
    fn list_with_state_filter() {
        let mgr = make_mgr();
        make_session(&mgr, "t1");
        make_session(&mgr, "t2");
        mgr.transition("t1", SessionState::Running, None, None).unwrap();

        let queued = mgr.list(Some(SessionState::Queued));
        assert_eq!(queued.len(), 1);

        let running = mgr.list(Some(SessionState::Running));
        assert_eq!(running.len(), 1);
    }

    #[test]
    fn counts_by_state() {
        let mgr = make_mgr();
        make_session(&mgr, "t1");
        make_session(&mgr, "t2");
        mgr.transition("t1", SessionState::Running, None, None).unwrap();

        let counts = mgr.counts();
        assert_eq!(counts.get(&SessionState::Queued), Some(&1));
        assert_eq!(counts.get(&SessionState::Running), Some(&1));
    }
}
