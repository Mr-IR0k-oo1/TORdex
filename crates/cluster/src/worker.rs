use async_trait::async_trait;

use crate::task::{ClusterTask, TaskResult};

/// A worker processes tasks from the cluster task queue.
#[async_trait]
pub trait Worker: Send + Sync {
    /// Unique name for this worker type.
    fn name(&self) -> &str;

    /// Process a task and return a result.
    async fn process(&self, task: &ClusterTask) -> TaskResult;
}
