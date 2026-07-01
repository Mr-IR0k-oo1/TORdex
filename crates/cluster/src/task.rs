use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use ulid::Ulid;

use crate::node::NodeRole;

pub type TaskId = Ulid;

/// Status of a distributed task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// The payload of a distributed cluster task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskPayload {
    /// Collector worker: fetch and process a URL.
    Collect {
        url: String,
        depth: u32,
        collect_images: bool,
        collect_links: bool,
    },
    /// AI worker: run a model operation.
    Ai {
        model: String,
        operation: AiOperation,
        input: serde_json::Value,
    },
    /// Graph worker: temporal graph operations.
    Graph {
        operation: GraphOperation,
        params: serde_json::Value,
    },
    /// Search worker: index or query.
    Search {
        operation: SearchOperation,
        params: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiOperation {
    Classify,
    Summarize,
    ExtractEntities,
    GenerateEmbedding,
    Translate { target_lang: String },
    Reason { rules: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphOperation {
    Snapshot { aggregate_type: String },
    Evolution { aggregate_type: String, window_hours: f64 },
    Predict { aggregate_type: String, horizon_hours: f64 },
    Query { pattern: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchOperation {
    Index {
        documents: Vec<IndexDocument>,
    },
    Query {
        expression: serde_json::Value,
        max_results: usize,
    },
    Delete {
        document_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDocument {
    pub id: String,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub metadata: std::collections::HashMap<String, String>,
}

/// A distributed task in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterTask {
    pub id: TaskId,
    pub target_role: NodeRole,
    pub payload: TaskPayload,
    pub status: TaskStatus,
    pub result: Option<serde_json::Value>,
    pub worker_id: Option<String>,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub max_retries: u32,
    pub retry_count: u32,
}

impl ClusterTask {
    pub fn new(target_role: NodeRole, payload: TaskPayload) -> Self {
        Self {
            id: Ulid::new(),
            target_role,
            payload,
            status: TaskStatus::Queued,
            result: None,
            worker_id: None,
            created_at: OffsetDateTime::now_utc(),
            completed_at: None,
            max_retries: 3,
            retry_count: 0,
        }
    }

    pub fn stream_key(&self) -> &'static str {
        self.target_role.task_stream()
    }
}

/// Result of a completed or failed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub worker_id: String,
    pub completed_at: OffsetDateTime,
}

impl TaskResult {
    pub fn ok(task_id: TaskId, data: serde_json::Value, worker_id: String) -> Self {
        Self {
            task_id,
            success: true,
            data: Some(data),
            error: None,
            worker_id,
            completed_at: OffsetDateTime::now_utc(),
        }
    }

    pub fn fail(task_id: TaskId, error: String, worker_id: String) -> Self {
        Self {
            task_id,
            success: false,
            data: None,
            error: Some(error),
            worker_id,
            completed_at: OffsetDateTime::now_utc(),
        }
    }
}

/// Redis-backed task queue using streams.
pub struct TaskQueue {
    redis: redis::aio::ConnectionManager,
}

impl TaskQueue {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        Self { redis }
    }

    /// Push a task to the appropriate worker stream.
    pub async fn enqueue(&self, task: &ClusterTask) -> Result<(), String> {
        let mut conn = self.redis.clone();
        let payload = serde_json::to_string(task).map_err(|e| e.to_string())?;
        let stream = task.stream_key();
        redis::cmd("XADD")
            .arg(stream)
            .arg("*")
            .arg("task")
            .arg(&payload)
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Blocking read of next task from a stream.
    pub async fn dequeue(
        &self,
        stream: &str,
        consumer_group: &str,
        consumer_name: &str,
        block_ms: usize,
    ) -> Result<Option<ClusterTask>, String> {
        let mut conn = self.redis.clone();

        // Ensure consumer group exists
        let _ = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream)
            .arg(consumer_group)
            .arg("0")
            .arg("MKSTREAM")
            .query_async::<()>(&mut conn)
            .await;

        let opts = redis::streams::StreamReadOptions::default()
            .group(consumer_group, consumer_name)
            .block(block_ms)
            .count(1);

        let result: redis::streams::StreamReadReply = conn
            .xread_options(&[stream], &[">"], &opts)
            .await
            .map_err(|e| e.to_string())?;

        for key in result.keys {
            for entry in key.ids {
                for (field, value) in &entry.map {
                    if field == "task" {
                        if let redis::Value::BulkString(bytes) = value {
                            let raw = String::from_utf8_lossy(&bytes);
                            let task: ClusterTask =
                                serde_json::from_str(&raw).map_err(|e| e.to_string())?;

                            // Acknowledge
                            let _: () = conn
                                .xack(stream, consumer_group, &[entry.id.as_str()])
                                .await
                                .unwrap_or(());

                            return Ok(Some(task));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Publish a task result to the scheduler's result stream.
    pub async fn publish_result(&self, result: &TaskResult) -> Result<(), String> {
        let mut conn = self.redis.clone();
        let payload = serde_json::to_string(result).map_err(|e| e.to_string())?;
        redis::cmd("XADD")
            .arg("tordex:tasks:results")
            .arg("*")
            .arg("result")
            .arg(&payload)
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Collect pending results (non-blocking).
    pub async fn collect_results(
        &self,
        count: usize,
    ) -> Result<Vec<TaskResult>, String> {
        let mut conn = self.redis.clone();
        let opts = redis::streams::StreamReadOptions::default().count(count);
        let result: redis::streams::StreamReadReply = conn
            .xread_options(&["tordex:tasks:results"], &["0"], &opts)
            .await
            .map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        for key in result.keys {
            for entry in key.ids {
                for (field, value) in &entry.map {
                    if field == "result" {
                        if let redis::Value::BulkString(bytes) = value {
                            let raw = String::from_utf8_lossy(&bytes);
                            if let Ok(task_result) = serde_json::from_str::<TaskResult>(&raw) {
                                results.push(task_result);
                                // Trim processed result
                                let _: () = conn
                                    .xdel("tordex:tasks:results", &[entry.id.as_str()])
                                    .await
                                    .unwrap_or(());
                            }
                        }
                    }
                }
            }
        }
        Ok(results)
    }
}
