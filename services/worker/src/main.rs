use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use tracing::info;

use tordex_ai_runtime::translation::{Language, TranslationConfig};
use tordex_ai_runtime::{Fact, NEREngine};
use tordex_cluster::node::{ClusterMembership, NodeInfo, NodeRole};
use tordex_cluster::task::{ClusterTask, TaskQueue, TaskResult};
use tordex_cluster::Worker;
use tordex_core::driver::Driver;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let role_str = std::env::var("WORKER_ROLE")
        .unwrap_or_else(|_| "collector".to_string());
    let role = NodeRole::from_str(&role_str)
        .context("invalid WORKER_ROLE, expected: collector, ai, graph, search")?;

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let host = std::env::var("WORKER_HOST")
        .unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("WORKER_PORT")
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .context("invalid WORKER_PORT")?;
    let heartbeat_secs: u64 = std::env::var("WORKER_HEARTBEAT_SECS")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .context("invalid WORKER_HEARTBEAT_SECS")?;

    let client = redis::Client::open(redis_url.as_str())
        .context("connecting to Redis")?;
    let manager = ConnectionManager::new(client)
        .await
        .context("creating Redis connection manager")?;

    let node_info = NodeInfo::new(role.clone(), host, port);
    let membership = ClusterMembership::new(manager.clone());
    let task_queue = TaskQueue::new(manager.clone());

    membership
        .register(&node_info, heartbeat_secs)
        .await
        .map_err(|e| anyhow::anyhow!(e))
        .context("registering in cluster")?;
    info!(
        role = %role.as_str(),
        id = %node_info.id,
        "worker registered in cluster"
    );

    let worker: Arc<dyn Worker> = match &role {
        NodeRole::Collector => Arc::new(CollectorWorker::new()),
        NodeRole::AiWorker => Arc::new(AiWorker::new()),
        NodeRole::GraphWorker => Arc::new(GraphWorker::new()),
        NodeRole::SearchWorker => Arc::new(SearchWorker::new()),
        _ => anyhow::bail!("no worker implementation for role: {}", role.as_str()),
    };

    let membership_clone = membership;
    let node_info_clone = node_info.clone();
    let hb_secs = heartbeat_secs;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(hb_secs / 2)).await;
            membership_clone.heartbeat(&node_info_clone, hb_secs).await.ok();
        }
    });

    info!(
        worker = worker.name(),
        stream = %role.task_stream(),
        "worker listening for tasks"
    );

    let consumer_group = format!("tordex:workers:{}", role.as_str());
    let consumer_name = format!("{}-{}", role.as_str(), node_info.id);

    loop {
        match task_queue
            .dequeue(role.task_stream(), &consumer_group, &consumer_name, 5000)
            .await
        {
            Ok(Some(task)) => {
                info!(task_id = %task.id, kind = ?task.payload, "processing task");
                let result = worker.process(&task).await;
                task_queue.publish_result(&result).await.ok();
                if result.success {
                    info!(task_id = %task.id, "task completed");
                } else {
                    tracing::warn!(
                        task_id = %task.id,
                        error = ?result.error,
                        "task failed"
                    );
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(error = %e, "dequeue error");
            }
        }
    }
}

// ─── Collector Worker ───────────────────────────────────────────────────

struct CollectorWorker;

impl CollectorWorker {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Worker for CollectorWorker {
    fn name(&self) -> &str {
        "collector"
    }

    async fn process(&self, task: &ClusterTask) -> TaskResult {
        use tordex_cluster::task::TaskPayload;
        match &task.payload {
            TaskPayload::Collect { url, depth, .. } => {
                let http_driver = tordex_drivers::http::HttpDriver::new();
                let result = http_driver
                    .execute("fetch_html", serde_json::json!({"url": url}));
                match result {
                    Ok(data) => TaskResult::ok(
                        task.id,
                        serde_json::json!({
                            "url": url,
                            "depth": depth,
                            "data": data,
                            "content_type": "text/html",
                        }),
                        "collector".to_string(),
                    ),
                    Err(e) => TaskResult::fail(task.id, e.to_string(), "collector".to_string()),
                }
            }
            _ => TaskResult::fail(
                task.id,
                "unexpected task type for collector".to_string(),
                "collector".to_string(),
            ),
        }
    }
}

// ─── AI Worker ──────────────────────────────────────────────────────────

struct AiWorker;

impl AiWorker {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Worker for AiWorker {
    fn name(&self) -> &str {
        "ai"
    }

    async fn process(&self, task: &ClusterTask) -> TaskResult {
        use tordex_cluster::task::{AiOperation, TaskPayload};
        match &task.payload {
            TaskPayload::Ai { model, operation, input } => {
                let runtime = tordex_ai_runtime::AiRuntime::new();
                let result = match operation {
                    AiOperation::Classify => {
                        let text = input.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        serde_json::to_value(runtime.classifier.classify(text))
                    }
                    AiOperation::Summarize => {
                        let text = input.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        serde_json::to_value(runtime.summarizer.summarize(text))
                    }
                    AiOperation::ExtractEntities => {
                        let text = input.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        let ner = NEREngine::new();
                        serde_json::to_value(ner.extract(text))
                    }
                    AiOperation::GenerateEmbedding => {
                        let text = input.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        serde_json::to_value(runtime.embeddings.embed(text))
                    }
                    AiOperation::Translate { target_lang } => {
                        let text = input.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        let target = match target_lang.as_str() {
                            "es" => Language::Spanish,
                            "fr" => Language::French,
                            "de" => Language::German,
                            "pt" => Language::Portuguese,
                            "ru" => Language::Russian,
                            "ja" => Language::Japanese,
                            "zh" => Language::ChineseSimplified,
                            "ar" => Language::Arabic,
                            _ => Language::Spanish,
                        };
                        let config = TranslationConfig {
                            source_language: Some(Language::English),
                            target_language: target,
                            allow_phrase_fallback: true,
                        };
                        serde_json::to_value(runtime.translator.translate(text, &config))
                    }
                    AiOperation::Reason { rules } => {
                        let facts_input = input.get("facts").and_then(|v| v.as_array());
                        let facts: Vec<Fact> = facts_input
                            .map(|f| {
                                f.iter()
                                    .filter_map(|v| v.as_str())
                                    .map(|s| Fact::new("derived", s))
                                    .collect()
                            })
                            .unwrap_or_default();
                        serde_json::to_value(runtime.reasoner.reason(&facts))
                    }
                };
                match result {
                    Ok(data) => TaskResult::ok(task.id, data, model.clone()),
                    Err(e) => TaskResult::fail(
                        task.id,
                        format!("AI serialization error: {e}"),
                        model.clone(),
                    ),
                }
            }
            _ => TaskResult::fail(
                task.id,
                "unexpected task type for AI worker".to_string(),
                "ai".to_string(),
            ),
        }
    }
}

// ─── Graph Worker ───────────────────────────────────────────────────────

struct GraphWorker;

impl GraphWorker {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Worker for GraphWorker {
    fn name(&self) -> &str {
        "graph"
    }

    async fn process(&self, task: &ClusterTask) -> TaskResult {
        use tordex_cluster::task::{GraphOperation, TaskPayload};
        use time::OffsetDateTime;
        match &task.payload {
            TaskPayload::Graph { operation, .. } => {
                let mut graph = tordex_temporal_graph::TemporalGraph::new();
                let now = OffsetDateTime::now_utc();
                let result = match operation {
                    GraphOperation::Snapshot { .. } => {
                        graph.snapshot(now);
                        serde_json::to_value(serde_json::json!({
                            "snapshot_count": graph.snapshot_count(),
                            "timestamp": now.to_string(),
                        }))
                    }
                    GraphOperation::Evolution { .. } => {
                        let ev = graph.evolution();
                        serde_json::to_value(ev)
                    }
                    GraphOperation::Predict { .. } => {
                        let pred = graph.predict(now);
                        serde_json::to_value(pred)
                    }
                    GraphOperation::Query { pattern } => {
                        Ok(pattern.clone())
                    }
                };
                match result {
                    Ok(data) => TaskResult::ok(task.id, data, "graph".to_string()),
                    Err(e) => TaskResult::fail(
                        task.id,
                        format!("graph serialization error: {e}"),
                        "graph".to_string(),
                    ),
                }
            }
            _ => TaskResult::fail(
                task.id,
                "unexpected task type for graph worker".to_string(),
                "graph".to_string(),
            ),
        }
    }
}

// ─── Search Worker ──────────────────────────────────────────────────────

struct SearchWorker;

impl SearchWorker {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Worker for SearchWorker {
    fn name(&self) -> &str {
        "search"
    }

    async fn process(&self, task: &ClusterTask) -> TaskResult {
        use tordex_cluster::task::{SearchOperation, TaskPayload};
        match &task.payload {
            TaskPayload::Search { operation, .. } => {
                let mut engine = tordex_search::SearchEngine::new();
                let result = match operation {
                    SearchOperation::Index { documents } => {
                        for doc in documents {
                            let document = tordex_search::Document {
                                id: doc.id.clone(),
                                title: doc.title.clone(),
                                body: doc.body.clone(),
                                kind: doc.kind.clone(),
                                source: "cluster".to_string(),
                                timestamp: None,
                                metadata: doc.metadata.clone(),
                            };
                            engine.index_document(document);
                        }
                        Ok(serde_json::json!({
                            "indexed": documents.len(),
                        }))
                    }
                    SearchOperation::Query { expression, max_results } => {
                        let query_expr: tordex_search::QueryExpr =
                            serde_json::from_value(expression.clone())
                                .unwrap_or(tordex_search::QueryExpr::Keyword(
                                    expression
                                        .get("keyword")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                ));
                        let results = engine.search(&query_expr, *max_results);
                        serde_json::to_value(results)
                    }
                    SearchOperation::Delete { document_ids } => {
                        Ok(serde_json::json!({
                            "deleted": document_ids.len(),
                            "note": "in-memory index; deletion is a no-op in current impl"
                        }))
                    }
                };
                match result {
                    Ok(data) => TaskResult::ok(task.id, data, "search".to_string()),
                    Err(e) => TaskResult::fail(
                        task.id,
                        format!("search error: {e}"),
                        "search".to_string(),
                    ),
                }
            }
            _ => TaskResult::fail(
                task.id,
                "unexpected task type for search worker".to_string(),
                "search".to_string(),
            ),
        }
    }
}
