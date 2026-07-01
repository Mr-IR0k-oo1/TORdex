use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use ulid::Ulid;

pub type NodeId = Ulid;

/// Role a cluster node can take.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeRole {
    Scheduler,
    Collector,
    AiWorker,
    GraphWorker,
    SearchWorker,
}

impl NodeRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeRole::Scheduler => "scheduler",
            NodeRole::Collector => "collector",
            NodeRole::AiWorker => "ai",
            NodeRole::GraphWorker => "graph",
            NodeRole::SearchWorker => "search",
        }
    }

    pub fn task_stream(&self) -> &'static str {
        match self {
            NodeRole::Scheduler => "tordex:tasks:scheduler",
            NodeRole::Collector => "tordex:tasks:collector",
            NodeRole::AiWorker => "tordex:tasks:ai",
            NodeRole::GraphWorker => "tordex:tasks:graph",
            NodeRole::SearchWorker => "tordex:tasks:search",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "scheduler" => Some(NodeRole::Scheduler),
            "collector" => Some(NodeRole::Collector),
            "ai" => Some(NodeRole::AiWorker),
            "graph" => Some(NodeRole::GraphWorker),
            "search" => Some(NodeRole::SearchWorker),
            _ => None,
        }
    }
}

/// Node registration info published to Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub role: NodeRole,
    pub host: String,
    pub port: u16,
    pub started_at: OffsetDateTime,
    pub version: String,
    pub tags: Vec<String>,
}

impl NodeInfo {
    pub fn new(role: NodeRole, host: String, port: u16) -> Self {
        Self {
            id: Ulid::new(),
            role,
            host,
            port,
            started_at: OffsetDateTime::now_utc(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            tags: Vec::new(),
        }
    }

    pub fn redis_key(&self) -> String {
        format!("tordex:cluster:node:{}", self.id)
    }

    pub fn heartbeat_key(&self) -> String {
        format!("tordex:cluster:heartbeat:{}", self.id)
    }
}

/// Summary of a cluster node for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSummary {
    pub id: NodeId,
    pub role: NodeRole,
    pub host: String,
    pub port: u16,
    pub status: NodeStatus,
    pub started_at: OffsetDateTime,
    pub tasks_processed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Alive,
    Dead,
    Unknown,
}

/// Cluster membership store backed by Redis.
pub struct ClusterMembership {
    redis: redis::aio::ConnectionManager,
}

impl ClusterMembership {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        Self { redis }
    }

    /// Register this node in the cluster with a heartbeat TTL.
    pub async fn register(
        &self,
        info: &NodeInfo,
        heartbeat_ttl_secs: u64,
    ) -> Result<(), String> {
        let mut conn = self.redis.clone();
        let json = serde_json::to_string(info).map_err(|e| e.to_string())?;
        redis::cmd("SET")
            .arg(info.redis_key())
            .arg(&json)
            .arg("EX")
            .arg(heartbeat_ttl_secs)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        redis::cmd("SADD")
            .arg("tordex:cluster:nodes")
            .arg(info.id.to_string())
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Refresh heartbeat to keep node alive.
    pub async fn heartbeat(
        &self,
        info: &NodeInfo,
        heartbeat_ttl_secs: u64,
    ) -> Result<(), String> {
        let mut conn = self.redis.clone();
        let now = OffsetDateTime::now_utc().to_string();
        redis::cmd("SET")
            .arg(info.heartbeat_key())
            .arg(&now)
            .arg("EX")
            .arg(heartbeat_ttl_secs)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| e.to_string())
    }

    /// List all registered node IDs.
    pub async fn list_nodes(&self) -> Result<Vec<NodeId>, String> {
        let mut conn = self.redis.clone();
        let ids: Vec<String> = redis::cmd("SMEMBERS")
            .arg("tordex:cluster:nodes")
            .query_async(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(ids
            .into_iter()
            .filter_map(|id| Ulid::from_string(&id).ok())
            .collect())
    }

    /// Get node info by ID.
    pub async fn get_node(&self, id: NodeId) -> Result<Option<NodeInfo>, String> {
        let mut conn = self.redis.clone();
        let key = format!("tordex:cluster:node:{id}");
        let raw: Option<String> = conn.get(&key).await.map_err(|e| e.to_string())?;
        match raw {
            Some(json) => serde_json::from_str(&json)
                .map(Some)
                .map_err(|e| e.to_string()),
            None => Ok(None),
        }
    }

    /// Check if a node is alive (has recent heartbeat).
    pub async fn is_alive(&self, id: NodeId) -> Result<bool, String> {
        let mut conn = self.redis.clone();
        let key = format!("tordex:cluster:heartbeat:{id}");
        let exists: bool = conn.exists(&key).await.map_err(|e| e.to_string())?;
        Ok(exists)
    }

    /// Deregister a node from the cluster.
    pub async fn deregister(&self, id: NodeId) -> Result<(), String> {
        let mut conn = self.redis.clone();
        redis::cmd("SREM")
            .arg("tordex:cluster:nodes")
            .arg(id.to_string())
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        let node_key = format!("tordex:cluster:node:{id}");
        let hb_key = format!("tordex:cluster:heartbeat:{id}");
        let _: () = conn.del(&node_key).await.map_err(|e| e.to_string())?;
        let _: () = conn.del(&hb_key).await.map_err(|e| e.to_string())?;
        Ok(())
    }
}
