use crate::node::{ClusterMembership, NodeRole};
use crate::task::{ClusterTask, TaskPayload, TaskQueue, TaskResult};
use ulid::Ulid;

/// The cluster scheduler distributes work to worker nodes.
pub struct ClusterScheduler {
    task_queue: TaskQueue,
    membership: ClusterMembership,
}

impl ClusterScheduler {
    pub fn new(task_queue: TaskQueue, membership: ClusterMembership) -> Self {
        Self {
            task_queue,
            membership,
        }
    }

    /// Access the cluster membership handle.
    pub fn cluster_membership(&self) -> &ClusterMembership {
        &self.membership
    }

    // ── Task dispatch ────────────────────────────────────────────────────

    pub async fn dispatch_collect(
        &self,
        url: &str,
        depth: u32,
        collect_images: bool,
        collect_links: bool,
    ) -> Result<Ulid, String> {
        let task = ClusterTask::new(
            NodeRole::Collector,
            TaskPayload::Collect {
                url: url.to_string(),
                depth,
                collect_images,
                collect_links,
            },
        );
        let id = task.id;
        self.task_queue.enqueue(&task).await?;
        Ok(id)
    }

    pub async fn dispatch_ai(
        &self,
        model: &str,
        operation: crate::task::AiOperation,
        input: serde_json::Value,
    ) -> Result<Ulid, String> {
        let task = ClusterTask::new(
            NodeRole::AiWorker,
            TaskPayload::Ai {
                model: model.to_string(),
                operation,
                input,
            },
        );
        let id = task.id;
        self.task_queue.enqueue(&task).await?;
        Ok(id)
    }

    pub async fn dispatch_graph(
        &self,
        operation: crate::task::GraphOperation,
        params: serde_json::Value,
    ) -> Result<Ulid, String> {
        let task = ClusterTask::new(
            NodeRole::GraphWorker,
            TaskPayload::Graph { operation, params },
        );
        let id = task.id;
        self.task_queue.enqueue(&task).await?;
        Ok(id)
    }

    pub async fn dispatch_search(
        &self,
        operation: crate::task::SearchOperation,
        params: serde_json::Value,
    ) -> Result<Ulid, String> {
        let task = ClusterTask::new(
            NodeRole::SearchWorker,
            TaskPayload::Search { operation, params },
        );
        let id = task.id;
        self.task_queue.enqueue(&task).await?;
        Ok(id)
    }

    // ── Result collection ────────────────────────────────────────────────

    pub async fn collect_pending_results(&self, count: usize) -> Result<Vec<TaskResult>, String> {
        self.task_queue.collect_results(count).await
    }
}
