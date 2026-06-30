//! Priority queue for collection tasks.
//!
//! Tasks are ordered by priority (Critical = 0 highest) then by submission
//! order (FIFO within the same priority level).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;


// ─── Priority ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
    Background = 4,
}

impl Priority {
    #[must_use]
    pub const fn label(&self) -> &str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Background => "background",
        }
    }

    #[must_use]
    pub fn from_label(s: &str) -> Option<Self> {
        Some(match s {
            "critical" => Self::Critical,
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            "background" => Self::Background,
            _ => return None,
        })
    }
}

// ─── CollectionTarget ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CollectionTarget {
    Url(String),
    Domain(String),
    Service { id: String, locator: String },
    Custom { kind: String, locator: String },
}

impl CollectionTarget {
    #[must_use]
    pub fn locator(&self) -> &str {
        match self {
            Self::Url(u) => u.as_str(),
            Self::Domain(d) => d.as_str(),
            Self::Service { locator, .. } => locator.as_str(),
            Self::Custom { locator, .. } => locator.as_str(),
        }
    }
}

// ─── CollectionTask ──────────────────────────────────────────────────────────

/// A collection job to be executed by a driver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionTask {
    pub id: String,
    pub target: CollectionTarget,
    pub priority: Priority,
    /// Name of the driver to use (e.g. "http", "browser").
    pub driver: String,
    /// Capability to invoke (e.g. "fetch", "`fetch_html`").
    pub capability: String,
    /// Parameters passed to the driver.
    pub params: Value,
    pub max_retries: u32,
    pub created_at: OffsetDateTime,
    pub metadata: HashMap<String, String>,
}

impl CollectionTask {
    #[must_use]
    pub fn new(
        id: String,
        target: CollectionTarget,
        driver: &str,
        capability: &str,
        params: Value,
    ) -> Self {
        Self {
            id,
            target,
            priority: Priority::Medium,
            driver: driver.to_string(),
            capability: capability.to_string(),
            params,
            max_retries: 3,
            created_at: OffsetDateTime::now_utc(),
            metadata: HashMap::new(),
        }
    }

    #[must_use]
    pub const fn with_priority(mut self, p: Priority) -> Self {
        self.priority = p;
        self
    }

    #[must_use]
    pub const fn with_max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, key: &str, val: &str) -> Self {
        self.metadata.insert(key.to_string(), val.to_string());
        self
    }
}

// ─── Internal queue entry ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct QueueEntry {
    task: CollectionTask,
    submitted_at: OffsetDateTime,
    seq: u64,
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Natural min-heap order: lower priority discriminant = higher priority.
        // Within same priority, earlier seq first (FIFO).
        self.task
            .priority
            .cmp(&other.task.priority)
            .then_with(|| self.seq.cmp(&other.seq))
    }
}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.task.id == other.task.id
    }
}

impl Eq for QueueEntry {}

// ─── PriorityQueue ───────────────────────────────────────────────────────────

/// A thread-safe priority queue for collection tasks.
///
/// Tasks are popped highest-priority-first, FIFO within the same priority.
pub struct PriorityQueue {
    inner: Arc<Mutex<QueueInner>>,
}

struct QueueInner {
    heap: Vec<QueueEntry>,
    by_id: HashMap<String, usize>,
    seen: HashSet<String>,
    seq: u64,
}

impl PriorityQueue {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(QueueInner {
                heap: Vec::new(),
                by_id: HashMap::new(),
                seen: HashSet::new(),
                seq: 0,
            })),
        }
    }

    /// Enqueue a task. Returns an error if a task with the same ID is already
    /// in the queue or has already been processed.
    pub fn enqueue(&self, task: CollectionTask) -> Result<(), FabricError> {
        let mut inner = self.inner.lock().unwrap();
        if inner.by_id.contains_key(&task.id) {
            return Err(FabricError::DuplicateTask(task.id));
        }
        let seq = inner.seq;
        inner.seq += 1;
        let idx = inner.heap.len();
        let entry = QueueEntry {
            task: task.clone(),
            submitted_at: OffsetDateTime::now_utc(),
            seq,
        };
        inner.heap.push(entry);
        inner.by_id.insert(task.id.clone(), idx);
        inner.seen.insert(task.id);
        // Bubble up
        bubble_up(&mut inner.heap, idx);
        Ok(())
    }

    /// Pop the highest-priority task.
    pub fn dequeue(&self) -> Option<CollectionTask> {
        let mut inner = self.inner.lock().unwrap();
        if inner.heap.is_empty() {
            return None;
        }
        let last = inner.heap.len() - 1;
        inner.heap.swap(0, last);
        let entry = inner.heap.pop();
        inner.by_id.remove(&entry.as_ref()?.task.id);
        if !inner.heap.is_empty() {
            bubble_down(&mut inner.heap, 0);
        }
        entry.map(|e| e.task)
    }

    /// Peek at the highest-priority task without removing it.
    #[must_use]
    pub fn peek(&self) -> Option<CollectionTask> {
        self.inner
            .lock()
            .unwrap()
            .heap
            .first()
            .map(|e| e.task.clone())
    }

    /// Remove a task by ID (if still in queue).
    pub fn cancel(&self, task_id: &str) -> Result<(), FabricError> {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner
            .by_id
            .remove(task_id)
            .ok_or_else(|| FabricError::TaskNotFound(task_id.to_string()))?;
        // Mark as removed (swap with last, pop)
        let last = inner.heap.len() - 1;
        if idx != last {
            inner.heap.swap(idx, last);
            let tid = inner.heap[idx].task.id.clone();
            inner.by_id.insert(tid, idx);
        }
        inner.heap.pop();
        inner.by_id.remove(task_id);
        // Restore heap property if the swapped element is still there
        if idx < inner.heap.len() {
            let saved = idx;
            bubble_up(&mut inner.heap, saved);
            bubble_down(&mut inner.heap, saved);
        }
        Ok(())
    }

    /// Number of tasks in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().heap.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether a task ID has been seen (queued or processed).
    #[must_use]
    pub fn contains(&self, task_id: &str) -> bool {
        self.inner.lock().unwrap().seen.contains(task_id)
    }

    /// List all tasks currently in the queue (sorted by priority).
    #[must_use]
    pub fn list(&self) -> Vec<CollectionTask> {
        let inner = self.inner.lock().unwrap();
        let mut tasks: Vec<CollectionTask> = inner.heap.iter().map(|e| e.task.clone()).collect();
        tasks.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| b.created_at.cmp(&a.created_at))
        });
        tasks
    }
}

impl Default for PriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Heap helpers ────────────────────────────────────────────────────────────

fn bubble_up(heap: &mut Vec<QueueEntry>, mut idx: usize) {
    while idx > 0 {
        let parent = (idx - 1) / 2;
        if heap[idx].cmp(&heap[parent]) != Ordering::Less {
            break;
        }
        heap.swap(idx, parent);
        idx = parent;
    }
}

fn bubble_down(heap: &mut Vec<QueueEntry>, mut idx: usize) {
    let len = heap.len();
    loop {
        let mut smallest = idx;
        let left = 2 * idx + 1;
        let right = 2 * idx + 2;
        if left < len && heap[left].cmp(&heap[smallest]) == Ordering::Less {
            smallest = left;
        }
        if right < len && heap[right].cmp(&heap[smallest]) == Ordering::Less {
            smallest = right;
        }
        if smallest == idx {
            break;
        }
        heap.swap(idx, smallest);
        idx = smallest;
    }
}

// ─── Fabric Error (re-exported) ──────────────────────────────────────────────

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FabricError {
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("duplicate task: {0}")]
    DuplicateTask(String),
    #[error("session error: {0}")]
    Session(String),
    #[error("driver error: {0}")]
    Driver(String),
    #[error("queue full")]
    QueueFull,
    #[error("fabric is stopped")]
    Stopped,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_task(id: &str) -> CollectionTask {
        CollectionTask::new(
            id.to_string(),
            CollectionTarget::Url("https://example.com".into()),
            "http",
            "fetch",
            json!({}),
        )
    }

    #[test]
    fn queue_enqueue_dequeue() {
        let q = PriorityQueue::new();
        q.enqueue(test_task("t1")).unwrap();
        q.enqueue(test_task("t2")).unwrap();
        assert_eq!(q.len(), 2);

        let t1 = q.dequeue().unwrap();
        assert_eq!(t1.id, "t1");
        let t2 = q.dequeue().unwrap();
        assert_eq!(t2.id, "t2");
        assert!(q.is_empty());
    }

    #[test]
    fn priority_ordering() {
        let q = PriorityQueue::new();
        q.enqueue(test_task("low").with_priority(Priority::Low))
            .unwrap();
        q.enqueue(test_task("critical").with_priority(Priority::Critical))
            .unwrap();
        q.enqueue(test_task("high").with_priority(Priority::High))
            .unwrap();
        q.enqueue(test_task("medium").with_priority(Priority::Medium))
            .unwrap();

        assert_eq!(q.dequeue().unwrap().id, "critical");
        assert_eq!(q.dequeue().unwrap().id, "high");
        assert_eq!(q.dequeue().unwrap().id, "medium");
        assert_eq!(q.dequeue().unwrap().id, "low");
    }

    #[test]
    fn fifo_within_same_priority() {
        let q = PriorityQueue::new();
        q.enqueue(test_task("a").with_priority(Priority::Medium))
            .unwrap();
        q.enqueue(test_task("b").with_priority(Priority::Medium))
            .unwrap();
        q.enqueue(test_task("c").with_priority(Priority::Medium))
            .unwrap();

        assert_eq!(q.dequeue().unwrap().id, "a");
        assert_eq!(q.dequeue().unwrap().id, "b");
        assert_eq!(q.dequeue().unwrap().id, "c");
    }

    #[test]
    fn duplicate_task_errors() {
        let q = PriorityQueue::new();
        q.enqueue(test_task("dup")).unwrap();
        let err = q.enqueue(test_task("dup")).unwrap_err();
        assert!(matches!(err, FabricError::DuplicateTask(_)));
    }

    #[test]
    fn cancel_removes_task() {
        let q = PriorityQueue::new();
        q.enqueue(test_task("t1")).unwrap();
        q.enqueue(test_task("t2")).unwrap();
        q.cancel("t1").unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q.dequeue().unwrap().id, "t2");
    }

    #[test]
    fn cancel_missing_errors() {
        let q = PriorityQueue::new();
        let err = q.cancel("ghost").unwrap_err();
        assert!(matches!(err, FabricError::TaskNotFound(_)));
    }

    #[test]
    fn peek_does_not_remove() {
        let q = PriorityQueue::new();
        q.enqueue(test_task("peek-me")).unwrap();
        let task = q.peek().unwrap();
        assert_eq!(task.id, "peek-me");
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn contains_tracks_seen_ids() {
        let q = PriorityQueue::new();
        q.enqueue(test_task("seen")).unwrap();
        assert!(q.contains("seen"));
        q.dequeue();
        // Even after dequeue, the ID is tracked
        assert!(q.contains("seen"));
    }

    #[test]
    fn list_returns_sorted_tasks() {
        let q = PriorityQueue::new();
        q.enqueue(test_task("b").with_priority(Priority::Low))
            .unwrap();
        q.enqueue(test_task("a").with_priority(Priority::Critical))
            .unwrap();
        let tasks = q.list();
        assert_eq!(tasks[0].id, "a");
        assert_eq!(tasks[1].id, "b");
    }

    #[test]
    fn priority_label_roundtrip() {
        for p in &[
            Priority::Critical,
            Priority::High,
            Priority::Medium,
            Priority::Low,
            Priority::Background,
        ] {
            let label = p.label();
            let back = Priority::from_label(label).unwrap();
            assert_eq!(*p, back);
        }
    }
}
