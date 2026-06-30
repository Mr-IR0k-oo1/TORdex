use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ulid::Ulid;

pub type TaskId = Ulid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

pub trait Scheduler: Send + Sync {
    fn spawn(&self, name: &str, task: Box<dyn FnOnce() + Send>) -> TaskId;
    fn schedule_after(
        &self,
        name: &str,
        task: Box<dyn FnOnce() + Send>,
        delay_ms: u64,
    ) -> TaskId;
    fn cancel(&self, id: TaskId) -> bool;
    fn status(&self, id: TaskId) -> Option<TaskStatus>;
    fn running_count(&self) -> u64;
}

pub struct SimpleScheduler {
    inner: Arc<Mutex<HashMap<TaskId, TaskStatus>>>,
}

impl Default for SimpleScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn set_status(&self, id: TaskId, status: TaskStatus) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert(id, status);
        }
    }
}

impl Scheduler for SimpleScheduler {
    fn spawn(&self, name: &str, task: Box<dyn FnOnce() + Send>) -> TaskId {
        let id = Ulid::new();
        self.set_status(id, TaskStatus::Running);
        let inner = self.inner.clone();
        let _ = name;
        std::thread::spawn(move || {
            task();
            if let Ok(mut guard) = inner.lock() {
                if guard.get(&id) != Some(&TaskStatus::Cancelled) {
                    guard.insert(id, TaskStatus::Completed);
                }
            }
        });
        id
    }

    fn schedule_after(
        &self,
        name: &str,
        task: Box<dyn FnOnce() + Send>,
        delay_ms: u64,
    ) -> TaskId {
        let id = Ulid::new();
        self.set_status(id, TaskStatus::Pending);
        let inner = self.inner.clone();
        let _ = name;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            if let Ok(mut guard) = inner.lock() {
                if guard.get(&id) == Some(&TaskStatus::Cancelled) {
                    return;
                }
                guard.insert(id, TaskStatus::Running);
            }
            task();
            if let Ok(mut guard) = inner.lock() {
                if guard.get(&id) != Some(&TaskStatus::Cancelled) {
                    guard.insert(id, TaskStatus::Completed);
                }
            }
        });
        id
    }

    fn cancel(&self, id: TaskId) -> bool {
        if let Ok(mut guard) = self.inner.lock() {
            match guard.get(&id) {
                Some(TaskStatus::Pending | TaskStatus::Running) => {
                    guard.insert(id, TaskStatus::Cancelled);
                    return true;
                }
                _ => return false,
            }
        }
        false
    }

    fn status(&self, id: TaskId) -> Option<TaskStatus> {
        self.inner.lock().ok().and_then(|g| g.get(&id).cloned())
    }

    fn running_count(&self) -> u64 {
        self.inner
            .lock()
            .map_or(0, |g| {
                g.values()
                    .filter(|s| **s == TaskStatus::Running)
                    .count() as u64
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_track_status() {
        let scheduler = SimpleScheduler::new();
        let id = scheduler.spawn("test", Box::new(|| {}));
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(scheduler.status(id), Some(TaskStatus::Completed));
    }

    #[test]
    fn cancel_task() {
        let scheduler = SimpleScheduler::new();
        let id = scheduler.spawn(
            "cancel-test",
            Box::new(|| {
                std::thread::sleep(std::time::Duration::from_secs(10));
            }),
        );
        assert!(scheduler.cancel(id));
    }

    #[test]
    fn running_count() {
        let scheduler = SimpleScheduler::new();
        assert_eq!(scheduler.running_count(), 0);
    }

    #[test]
    fn schedule_after() {
        let scheduler = SimpleScheduler::new();
        let flag = Arc::new(Mutex::new(false));
        let flag_clone = flag.clone();
        let id = scheduler.schedule_after(
            "delayed",
            Box::new(move || {
                *flag_clone.lock().unwrap() = true;
            }),
            10,
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(scheduler.status(id), Some(TaskStatus::Completed));
        assert!(*flag.lock().unwrap());
    }
}
