use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    extract::ws,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tower_http::cors::CorsLayer;
use tracing::info;
use ulid::Ulid;

use tordex_core::event_store::EventEnvelope;
use tordex_core::object::ObjectId;
use tordex_cluster::{
    AiOperation, ClusterScheduler, GraphOperation, SearchOperation,
};
use tordex_core::Kernel;
use tordex_decision::question::Question;
use tordex_search::{Document, QueryExpr, SearchEngine};

// ─── Shared State ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub kernel: Arc<Kernel>,
    pub search_engine: Arc<Mutex<SearchEngine>>,
    pub cluster_scheduler: Option<Arc<ClusterScheduler>>,
}

// ─── Router ───────────────────────────────────────────────────────────────

pub fn build_router(
    kernel: Arc<Kernel>,
    cluster_scheduler: Option<Arc<ClusterScheduler>>,
) -> Router {
    let state = AppState {
        kernel,
        search_engine: Arc::new(Mutex::new(SearchEngine::new())),
        cluster_scheduler,
    };

    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/kernel/status", get(kernel_status))
        .route("/api/v1/kernel/agents", get(list_agents))
        .route("/api/v1/kernel/agents/{id}", get(get_agent))
        .route("/api/v1/kernel/agents/{id}/tick", post(tick_agent))
        .route("/api/v1/kernel/events", get(list_events).post(publish_event))
        .route("/api/v1/kernel/events/count", get(event_counts))
        .route("/api/v1/kernel/objects", get(list_objects).post(create_object))
        .route(
            "/api/v1/kernel/objects/{id}",
            get(get_object).put(update_object).delete(delete_object),
        )
        .route("/api/v1/kernel/objects/{id}/links", get(get_object_links))
        .route("/api/v1/kernel/objects/kinds", get(object_kinds))
        .route("/api/v1/kernel/storage", post(store_data))
        .route("/api/v1/kernel/storage/{*key}", get(load_data).delete(delete_data))
        .route("/api/v1/kernel/storage/list/{*prefix}", get(list_storage))
        .route("/api/v1/intel/search", post(search_intel))
        .route("/api/v1/intel/decision", post(ask_decision))
        .route("/api/v1/monitoring/status", get(monitoring_status))
        .route("/api/v1/events/stream", get(ws_handler))
        .route("/api/v1/kernel/drivers", get(list_drivers))
        .route("/api/v1/kernel/processors", get(list_processors))
        .route("/api/v1/cluster/nodes", get(cluster_list_nodes))
        .route("/api/v1/cluster/nodes/{id}", get(cluster_get_node))
        .route("/api/v1/cluster/dispatch/collect", post(cluster_dispatch_collect))
        .route("/api/v1/cluster/dispatch/ai", post(cluster_dispatch_ai))
        .route("/api/v1/cluster/dispatch/graph", post(cluster_dispatch_graph))
        .route("/api/v1/cluster/dispatch/search", post(cluster_dispatch_search))
        .route("/api/v1/cluster/results", get(cluster_results))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// ─── Health ───────────────────────────────────────────────────────────────

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "tordex-kernel",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": OffsetDateTime::now_utc().unix_timestamp(),
    }))
}

// ─── Kernel Status ────────────────────────────────────────────────────────

async fn kernel_status(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let kernel = &state.kernel;
    let mem = kernel.memory.stats();
    Json(serde_json::json!({
        "agents": kernel.agents.list().len(),
        "drivers": kernel.drivers.list().len(),
        "events": kernel.event_store.total_count(),
        "objects": kernel.objects.find_by_kind("").len(),
        "running_tasks": kernel.scheduler.running_count(),
        "memory": {
            "allocated_bytes": mem.allocated_bytes,
            "deallocated_bytes": mem.deallocated_bytes,
            "live_allocations": mem.live_allocations,
            "peak_allocated_bytes": mem.peak_allocated_bytes,
        },
    }))
}

// ─── Agents ───────────────────────────────────────────────────────────────

async fn list_agents(
    State(state): State<AppState>,
) -> Json<Vec<serde_json::Value>> {
    let agents = state.kernel.agents.list();
    Json(
        agents
            .into_iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id.to_string(),
                    "name": m.name,
                    "kind": m.kind,
                    "version": m.version,
                    "description": m.description,
                    "status": format!("{:?}", m.status),
                })
            })
            .collect(),
    )
}

async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let agent_id =
        Ulid::from_string(&id).map_err(|_| ApiError::BadRequest("invalid agent id".into()))?;
    let manifest = state
        .kernel
        .agents
        .list()
        .into_iter()
        .find(|m| m.id == agent_id)
        .ok_or_else(|| ApiError::NotFound("agent not found".into()))?;

    Ok(Json(serde_json::json!({
        "id": manifest.id.to_string(),
        "name": manifest.name,
        "kind": manifest.kind,
        "version": manifest.version,
        "description": manifest.description,
        "status": format!("{:?}", manifest.status),
    })))
}

async fn tick_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let agent_id =
        Ulid::from_string(&id).map_err(|_| ApiError::BadRequest("invalid agent id".into()))?;
    let agent = state
        .kernel
        .agents
        .get(agent_id)
        .ok_or_else(|| ApiError::NotFound("agent not found".into()))?;
    let kernel = &state.kernel;
    tokio::task::block_in_place(|| agent.tick(kernel))
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "ticked", "agent_id": id})))
}

// ─── Events ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListEventsParams {
    aggregate_type: Option<String>,
    aggregate_id: Option<String>,
    since: Option<String>,
    limit: Option<usize>,
}

async fn list_events(
    State(state): State<AppState>,
    Query(params): Query<ListEventsParams>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let events: Vec<EventEnvelope> = if let Some(agg_id) = &params.aggregate_id {
        state
            .kernel
            .event_store
            .read_events(agg_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else if let Some(agg_type) = &params.aggregate_type {
        state
            .kernel
            .event_store
            .read_all(agg_type)
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else if let Some(since) = &params.since {
        let since_id =
            Ulid::from_string(since).map_err(|_| ApiError::BadRequest("invalid ULID".into()))?;
        state
            .kernel
            .event_store
            .read_since(since_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        Vec::new()
    };

    let limit = params.limit.unwrap_or(100);
    let events: Vec<_> = events
        .into_iter()
        .take(limit)
        .map(|e| {
            serde_json::json!({
                "id": e.id.to_string(),
                "aggregate_id": e.aggregate_id,
                "aggregate_type": e.aggregate_type,
                "event_type": e.event_type,
                "version": e.version,
                "data": e.data,
                "metadata": e.metadata,
                "timestamp": e.timestamp.to_string(),
            })
        })
        .collect();

    Ok(Json(events))
}

#[derive(Deserialize, Serialize)]
struct PublishEventPayload {
    aggregate_type: String,
    aggregate_id: String,
    event_type: String,
    data: serde_json::Value,
}

async fn publish_event(
    State(state): State<AppState>,
    Json(payload): Json<PublishEventPayload>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let agg_id = &payload.aggregate_id;
    let version = state.kernel.event_store.latest_version(agg_id) + 1;

    let envelope = EventEnvelope::new(
        agg_id.clone(),
        &payload.aggregate_type,
        &payload.event_type,
        version,
        payload.data.clone(),
    );

    state
        .kernel
        .event_store
        .append(envelope)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let msg = serde_json::to_vec(&payload).unwrap_or_default();
    state.kernel.event.publish(&payload.event_type, &msg);

    Ok(Json(serde_json::json!({
        "status": "published",
        "aggregate_id": agg_id,
        "version": version,
    })))
}

async fn event_counts(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "total": state.kernel.event_store.total_count(),
    }))
}

// ─── Objects ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListObjectsParams {
    kind: Option<String>,
    label: Option<String>,
}

async fn list_objects(
    State(state): State<AppState>,
    Query(params): Query<ListObjectsParams>,
) -> Json<Vec<serde_json::Value>> {
    let objects = if let Some(kind) = &params.kind {
        state.kernel.objects.find_by_kind(kind)
    } else if let Some(label) = &params.label {
        state.kernel.objects.find_by_label(label)
    } else {
        Vec::new()
    };

    Json(
        objects
            .into_iter()
            .map(|o| {
                serde_json::json!({
                    "id": o.id.to_string(),
                    "kind": o.kind,
                    "label": o.label,
                    "data_size": o.data.len(),
                    "created_at": o.created_at,
                    "updated_at": o.updated_at,
                })
            })
            .collect(),
    )
}

#[derive(Deserialize)]
struct CreateObjectPayload {
    kind: String,
    label: String,
    data: Option<String>,
}

async fn create_object(
    State(state): State<AppState>,
    Json(payload): Json<CreateObjectPayload>,
) -> Json<serde_json::Value> {
    let data = payload.data.unwrap_or_default();
    let id = state
        .kernel
        .objects
        .create(&payload.kind, &payload.label, data.as_bytes());
    Json(serde_json::json!({
        "id": id.to_string(),
        "kind": payload.kind,
        "label": payload.label,
    }))
}

async fn get_object(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let obj_id = ObjectId::from_string(&id)
        .map_err(|_| ApiError::BadRequest("invalid object id".into()))?;
    let obj = state
        .kernel
        .objects
        .read(obj_id)
        .ok_or_else(|| ApiError::NotFound("object not found".into()))?;

    Ok(Json(serde_json::json!({
        "id": obj.id.to_string(),
        "kind": obj.kind,
        "label": obj.label,
        "data": String::from_utf8_lossy(&obj.data),
        "created_at": obj.created_at,
        "updated_at": obj.updated_at,
    })))
}

#[derive(Deserialize)]
struct UpdateObjectPayload {
    data: String,
}

async fn update_object(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateObjectPayload>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let obj_id = ObjectId::from_string(&id)
        .map_err(|_| ApiError::BadRequest("invalid object id".into()))?;
    let updated = state.kernel.objects.update(obj_id, payload.data.as_bytes());
    if !updated {
        return Err(ApiError::NotFound("object not found".into()));
    }
    Ok(Json(serde_json::json!({"status": "updated"})))
}

async fn delete_object(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let obj_id = ObjectId::from_string(&id)
        .map_err(|_| ApiError::BadRequest("invalid object id".into()))?;
    let deleted = state.kernel.objects.delete(obj_id);
    if !deleted {
        return Err(ApiError::NotFound("object not found".into()));
    }
    Ok(Json(serde_json::json!({"status": "deleted"})))
}

async fn get_object_links(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let obj_id = ObjectId::from_string(&id)
        .map_err(|_| ApiError::BadRequest("invalid object id".into()))?;
    let links = state.kernel.objects.links(obj_id);
    Ok(Json(
        links
            .into_iter()
            .map(|l| {
                serde_json::json!({
                    "id": l.id.to_string(),
                    "source_id": l.source_id.to_string(),
                    "target_id": l.target_id.to_string(),
                    "kind": l.kind,
                    "created_at": l.created_at,
                })
            })
            .collect(),
    ))
}

async fn object_kinds(
    State(state): State<AppState>,
) -> Json<Vec<String>> {
    let mut kinds: Vec<String> = state
        .kernel
        .objects
        .find_by_kind("")
        .into_iter()
        .map(|o| o.kind)
        .collect();
    kinds.sort();
    kinds.dedup();
    Json(kinds)
}

// ─── Storage ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct StoreDataPayload {
    key: String,
    value: String,
    content_type: Option<String>,
}

async fn store_data(
    State(state): State<AppState>,
    Json(payload): Json<StoreDataPayload>,
) -> Json<serde_json::Value> {
    state.kernel.storage.store(
        &payload.key,
        payload.value.as_bytes(),
        payload.content_type.as_deref(),
    );
    Json(serde_json::json!({"status": "stored", "key": payload.key}))
}

async fn load_data(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Response, ApiError> {
    let entry = state
        .kernel
        .storage
        .load(&key)
        .ok_or_else(|| ApiError::NotFound("key not found".into()))?;
    Ok((
        StatusCode::OK,
        [(
            "content-type",
            entry
                .content_type
                .unwrap_or_else(|| "application/octet-stream".into()),
        )],
        entry.value,
    )
        .into_response())
}

async fn delete_data(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Json<serde_json::Value> {
    let deleted = state.kernel.storage.delete(&key);
    Json(serde_json::json!({"status": if deleted { "deleted" } else { "not_found" }}))
}

async fn list_storage(
    State(state): State<AppState>,
    Path(prefix): Path<String>,
) -> Json<Vec<String>> {
    Json(state.kernel.storage.list(&prefix))
}

// ─── Intelligence ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    kind: Option<String>,
    limit: Option<usize>,
}

async fn search_intel(
    State(state): State<AppState>,
    Json(query): Json<SearchQuery>,
) -> Json<serde_json::Value> {
    let objects = if let Some(kind) = &query.kind {
        if kind.is_empty() {
            Vec::new()
        } else {
            state.kernel.objects.find_by_kind(kind)
        }
    } else {
        Vec::new()
    };

    {
        let mut engine = state.search_engine.lock().unwrap();
        for obj in &objects {
            let doc = Document {
                id: obj.id.to_string(),
                title: obj.label.clone(),
                body: String::from_utf8_lossy(&obj.data).to_string(),
                kind: obj.kind.clone(),
                source: "kernel".to_string(),
                timestamp: None,
                metadata: HashMap::from([("object_id".to_string(), obj.id.to_string())]),
            };
            engine.index_document(doc);
        }
    }

    let query_expr = QueryExpr::Keyword(query.q.clone());
    let engine = state.search_engine.lock().unwrap();
    let results = engine.search(&query_expr, query.limit.unwrap_or(10));

    Json(serde_json::json!({
        "query": query.q,
        "total_objects": objects.len(),
        "results": results.results,
        "took_ns": results.took_ns,
    }))
}

#[derive(Deserialize)]
struct DecisionQuery {
    question_type: String,
    aggregate_id: Option<String>,
    kind: Option<String>,
    max_results: Option<usize>,
}

async fn ask_decision(
    State(state): State<AppState>,
    Json(query): Json<DecisionQuery>,
) -> Json<serde_json::Value> {
    let question = match query.question_type.as_str() {
        "what_changed" => Question::WhatChanged {
            aggregate_type: query.kind.clone(),
            since: None,
            until: None,
        },
        "why" => Question::Why {
            aggregate_id: query.aggregate_id.clone().unwrap_or_default(),
            max_depth: query.max_results,
        },
        "what_matters" => Question::WhatMatters {
            kind: query.kind.clone(),
            max_results: query.max_results,
        },
        "what_is_risky" => Question::WhatIsRisky {
            min_severity: query.kind.clone(),
            max_results: query.max_results,
        },
        "what_to_investigate" => Question::WhatShouldIInvestigate {
            max_results: query.max_results,
        },
        "what_is_similar" => Question::WhatIsSimilar {
            aggregate_id: query.aggregate_id.clone().unwrap_or_default(),
            kind: query.kind.clone(),
            max_results: query.max_results,
        },
        "predict" => Question::WhatWillLikelyHappen {
            kind: query.kind.clone(),
            horizon_hours: None,
        },
        _ => {
            return Json(serde_json::json!({
                "error": format!("unknown question type: {}", query.question_type),
                "valid_types": ["what_changed", "why", "what_matters", "what_is_risky",
                                "what_to_investigate", "what_is_similar", "predict"],
            }));
        }
    };

    let engine = tordex_decision::DecisionEngine::new();
    match engine.analyze(&state.kernel, &question) {
        Ok(answer) => Json(serde_json::json!({
            "question": question,
            "answer": {
                "id": answer.id,
                "summary": answer.summary,
                "confidence": answer.confidence,
                "severity": answer.severity,
                "evidence": answer.evidence,
                "recommendation": answer.recommendation,
            }
        })),
        Err(e) => Json(serde_json::json!({
            "error": e.to_string(),
        })),
    }
}

// ─── Monitoring ───────────────────────────────────────────────────────────

async fn monitoring_status(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let agents = state.kernel.agents.list();
    let monitoring_agent = agents.iter().find(|a| a.kind == "monitoring");

    Json(serde_json::json!({
        "monitoring_agent": monitoring_agent.map(|a| serde_json::json!({
            "id": a.id.to_string(),
            "name": a.name,
            "status": format!("{:?}", a.status),
        })),
        "agent_count": agents.len(),
    }))
}

// ─── WebSocket Events ─────────────────────────────────────────────────────

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: ws::WebSocket, state: AppState) {
    let sub = state.kernel.event.subscribe("all");
    info!("websocket client connected");

    loop {
        tokio::select! {
            events = futures::future::ready(state.kernel.event.poll(sub)) => {
                for event in events {
                    let msg = serde_json::json!({
                        "id": event.id.to_string(),
                        "topic": event.topic,
                        "payload": String::from_utf8_lossy(&event.payload),
                        "occurred_at": event.occurred_at,
                    });
                    if socket
                        .send(axum::extract::ws::Message::Text(
                            serde_json::to_string(&msg).unwrap_or_default(),
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
            _ = socket.recv() => {
                break;
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    state.kernel.event.unsubscribe(sub);
    info!("websocket client disconnected");
}

// ─── Drivers & Processors ────────────────────────────────────────────────

async fn list_drivers(
    State(state): State<AppState>,
) -> Json<Vec<serde_json::Value>> {
    let drivers = state.kernel.drivers.list();
    Json(
        drivers
            .into_iter()
            .map(|d| {
                serde_json::json!({
                    "name": d.name,
                    "description": d.description,
                    "capabilities": d.capabilities,
                })
            })
            .collect(),
    )
}

async fn list_processors(
    State(state): State<AppState>,
) -> Json<Vec<serde_json::Value>> {
    let plugins = state.kernel.plugin.list();
    Json(
        plugins
            .into_iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id.to_string(),
                    "name": p.name,
                    "version": p.version,
                    "description": p.description,
                    "capabilities": p.capabilities,
                })
            })
            .collect(),
    )
}

// ─── Cluster ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DispatchCollectParams {
    url: String,
    #[serde(default)]
    depth: u32,
    #[serde(default)]
    collect_images: bool,
    #[serde(default)]
    collect_links: bool,
}

#[derive(Deserialize)]
struct DispatchAiParams {
    model: String,
    operation: String,
    input: serde_json::Value,
}

#[derive(Deserialize)]
struct DispatchGraphParams {
    operation: String,
    params: serde_json::Value,
}

#[derive(Deserialize)]
struct DispatchSearchParams {
    operation: String,
    params: serde_json::Value,
}

async fn cluster_list_nodes(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let scheduler = state
        .cluster_scheduler
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("cluster not configured (no Redis)".into()))?;
    let membership = scheduler.cluster_membership();
    let ids = membership.list_nodes().await.map_err(|e| ApiError::Internal(e))?;
    let mut nodes = Vec::new();
    for id in ids {
        if let Some(info) = membership
            .get_node(id)
            .await
            .map_err(|e| ApiError::Internal(e))?
        {
            let alive = membership.is_alive(id).await.unwrap_or(false);
            nodes.push(serde_json::json!({
                "id": info.id.to_string(),
                "role": info.role.as_str(),
                "host": info.host,
                "port": info.port,
                "started_at": info.started_at,
                "status": if alive { "alive" } else { "dead" },
            }));
        }
    }
    Ok(Json(nodes))
}

async fn cluster_get_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scheduler = state
        .cluster_scheduler
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("cluster not configured".into()))?;
    let node_id =
        Ulid::from_string(&id).map_err(|_| ApiError::BadRequest("invalid node id".into()))?;
    let membership = scheduler.cluster_membership();
    let info = membership
        .get_node(node_id)
        .await
        .map_err(|e| ApiError::Internal(e))?
        .ok_or_else(|| ApiError::NotFound("node not found".into()))?;
    let alive = membership.is_alive(node_id).await.unwrap_or(false);
    Ok(Json(serde_json::json!({
        "id": info.id.to_string(),
        "role": info.role.as_str(),
        "host": info.host,
        "port": info.port,
        "started_at": info.started_at,
        "version": info.version,
        "status": if alive { "alive" } else { "dead" },
    })))
}

async fn cluster_dispatch_collect(
    State(state): State<AppState>,
    Json(params): Json<DispatchCollectParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scheduler = state
        .cluster_scheduler
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("cluster not configured".into()))?;
    let task_id = scheduler
        .dispatch_collect(&params.url, params.depth, params.collect_images, params.collect_links)
        .await
        .map_err(|e| ApiError::Internal(e))?;
    Ok(Json(serde_json::json!({"task_id": task_id.to_string(), "status": "queued"})))
}

async fn cluster_dispatch_ai(
    State(state): State<AppState>,
    Json(params): Json<DispatchAiParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scheduler = state
        .cluster_scheduler
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("cluster not configured".into()))?;
    let op = match params.operation.as_str() {
        "classify" => AiOperation::Classify,
        "summarize" => AiOperation::Summarize,
        "extract_entities" => AiOperation::ExtractEntities,
        "embedding" => AiOperation::GenerateEmbedding,
        "translate" => {
            let target = params
                .input
                .get("target_lang")
                .and_then(|v| v.as_str())
                .unwrap_or("en")
                .to_string();
            AiOperation::Translate { target_lang: target }
        }
        "reason" => {
            let rules: Vec<String> = params
                .input
                .get("rules")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            AiOperation::Reason { rules }
        }
        _ => return Err(ApiError::BadRequest(format!("unknown ai operation: {}", params.operation))),
    };
    let task_id = scheduler
        .dispatch_ai(&params.model, op, params.input)
        .await
        .map_err(|e| ApiError::Internal(e))?;
    Ok(Json(serde_json::json!({"task_id": task_id.to_string(), "status": "queued"})))
}

async fn cluster_dispatch_graph(
    State(state): State<AppState>,
    Json(params): Json<DispatchGraphParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scheduler = state
        .cluster_scheduler
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("cluster not configured".into()))?;
    let op = match params.operation.as_str() {
        "snapshot" => GraphOperation::Snapshot {
            aggregate_type: params.params.get("aggregate_type")
                .and_then(|v| v.as_str())
                .unwrap_or("all")
                .to_string(),
        },
        "evolution" => GraphOperation::Evolution {
            aggregate_type: params.params.get("aggregate_type")
                .and_then(|v| v.as_str())
                .unwrap_or("all")
                .to_string(),
            window_hours: params.params.get("window_hours")
                .and_then(|v| v.as_f64())
                .unwrap_or(24.0),
        },
        "predict" => GraphOperation::Predict {
            aggregate_type: params.params.get("aggregate_type")
                .and_then(|v| v.as_str())
                .unwrap_or("all")
                .to_string(),
            horizon_hours: params.params.get("horizon_hours")
                .and_then(|v| v.as_f64())
                .unwrap_or(24.0),
        },
        "query" => GraphOperation::Query {
            pattern: params.params.get("pattern").cloned().unwrap_or(serde_json::Value::Null),
        },
        _ => return Err(ApiError::BadRequest(format!("unknown graph operation: {}", params.operation))),
    };
    let task_id = scheduler
        .dispatch_graph(op, params.params)
        .await
        .map_err(|e| ApiError::Internal(e))?;
    Ok(Json(serde_json::json!({"task_id": task_id.to_string(), "status": "queued"})))
}

async fn cluster_dispatch_search(
    State(state): State<AppState>,
    Json(params): Json<DispatchSearchParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scheduler = state
        .cluster_scheduler
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("cluster not configured".into()))?;
    let op = match params.operation.as_str() {
        "index" => SearchOperation::Index {
            documents: serde_json::from_value(params.params.get("documents").cloned()
                .unwrap_or(serde_json::Value::Null))
                .map_err(|e| ApiError::BadRequest(format!("invalid documents: {e}")))?,
        },
        "query" => SearchOperation::Query {
            expression: params.params.get("expression").cloned()
                .unwrap_or(serde_json::Value::Null),
            max_results: params.params.get("max_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as usize,
        },
        "delete" => SearchOperation::Delete {
            document_ids: serde_json::from_value(params.params.get("document_ids").cloned()
                .unwrap_or(serde_json::Value::Null))
                .map_err(|e| ApiError::BadRequest(format!("invalid document_ids: {e}")))?,
        },
        _ => return Err(ApiError::BadRequest(format!("unknown search operation: {}", params.operation))),
    };
    let task_id = scheduler
        .dispatch_search(op, params.params)
        .await
        .map_err(|e| ApiError::Internal(e))?;
    Ok(Json(serde_json::json!({"task_id": task_id.to_string(), "status": "queued"})))
}

async fn cluster_results(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let scheduler = state
        .cluster_scheduler
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("cluster not configured".into()))?;
    let results = scheduler
        .collect_pending_results(50)
        .await
        .map_err(|e| ApiError::Internal(e))?;
    let output: Vec<serde_json::Value> = results
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "task_id": r.task_id.to_string(),
                "success": r.success,
                "data": r.data,
                "error": r.error,
                "worker_id": r.worker_id,
                "completed_at": r.completed_at,
            })
        })
        .collect();
    Ok(Json(output))
}

// ─── Error Handling ───────────────────────────────────────────────────────

enum ApiError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(serde_json::json!({"error": message}))).into_response()
    }
}
