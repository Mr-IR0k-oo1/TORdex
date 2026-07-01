//! TORdex Agent Runtime — concrete agent implementations.
//!
//! Agents operate exclusively through kernel APIs. No direct database access.

use std::sync::{Arc, Mutex};

use serde_json::json;
use time::OffsetDateTime;
use tordex_core::event_store::EventEnvelope;
use tordex_core::{Agent, AgentId, AgentManifest, AgentStatus, Kernel, Result};
use ulid::Ulid;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

/// Register all built-in agents into the kernel's agent runtime.
pub fn register_all(kernel: &Kernel) -> Result<()> {
    let agents: Vec<Box<dyn Agent>> = vec![
        Box::new(ResearchAgent::new()),
        Box::new(ArchitectureAgent::new()),
        Box::new(MalwareAgent::new()),
        Box::new(MonitoringAgent::new()),
        Box::new(DocumentationAgent::new()),
        Box::new(ForensicsAgent::new()),
    ];
    for agent in agents {
        let name = agent.manifest().name.clone();
        kernel.agents.register(agent)?;
        tracing::info!(agent = %name, "registered agent");
    }
    Ok(())
}

// ─── Research Agent ──────────────────────────────────────────────────────────

/// Collects intelligence by dispatching driver capabilities.
///
/// Uses: kernel.drivers, kernel.objects, kernel.event_store, kernel.security
pub struct ResearchAgent {
    id: AgentId,
    state: Arc<Mutex<ResearchState>>,
}

struct ResearchState {
    queries_processed: u64,
    entities_discovered: u64,
}

impl ResearchAgent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: AgentId::new(),
            state: Arc::new(Mutex::new(ResearchState {
                queries_processed: 0,
                entities_discovered: 0,
            })),
        }
    }
}

impl Agent for ResearchAgent {
    fn manifest(&self) -> AgentManifest {
        AgentManifest {
            id: self.id,
            name: "research".into(),
            kind: "intelligence".into(),
            version: "0.1.0".into(),
            description: "Collects intelligence by dispatching driver capabilities (HTTP, DNS) and recording observations".into(),
            status: AgentStatus::Idle,
        }
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        kernel.event.subscribe("agent.research");
        kernel.event.subscribe("system.entity.created");
        tracing::info!(agent = "research", "initialized");
        Ok(())
    }

    fn tick(&self, kernel: &Kernel) -> Result<()> {
        // Discover available drivers for data collection
        let drivers = kernel.drivers.list();
        let has_http = drivers
            .iter()
            .any(|d| d.capabilities.iter().any(|c| c.name == "fetch_html"));
        let has_dns = drivers
            .iter()
            .any(|d| d.capabilities.iter().any(|c| c.name == "resolve_a"));

        // Store capability map as an object for other agents to query
        let cap_data = json!({
            "http_available": has_http,
            "dns_available": has_dns,
            "driver_count": drivers.len(),
            "timestamp": now().to_string(),
        });
        kernel.objects.create(
            "research_capability_map",
            "research-capabilities",
            &serde_json::to_vec(&cap_data).unwrap(),
        );

        let mut state = self.state.lock().unwrap();
        state.queries_processed += 1;
        Ok(())
    }

    fn handle_event(&self, kernel: &Kernel, event: &EventEnvelope) -> Result<()> {
        if event.event_type == "Stored" && event.aggregate_type == "Artifact" {
            // When new artifacts arrive, flag for analysis
            let note = json!({
                "agent": "research",
                "artifact_id": event.aggregate_id,
                "action": "pending_analysis",
                "timestamp": now().to_string(),
            });
            kernel.objects.create(
                "research_note",
                &format!("research-note-{}", Ulid::new()),
                &serde_json::to_vec(&note).unwrap(),
            );
        }
        Ok(())
    }

    fn stop(&self, _kernel: &Kernel) -> Result<()> {
        let state = self.state.lock().unwrap();
        tracing::info!(
            agent = "research",
            queries = state.queries_processed,
            entities = state.entities_discovered,
            "stopped"
        );
        Ok(())
    }
}

// ─── Architecture Agent ──────────────────────────────────────────────────────

/// Maps system architecture by discovering components and relationships.
///
/// Uses: kernel.drivers, kernel.objects, kernel.processor registry
pub struct ArchitectureAgent {
    id: AgentId,
    state: Arc<Mutex<ArchState>>,
}

struct ArchState {
    components_mapped: u64,
}

impl ArchitectureAgent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: AgentId::new(),
            state: Arc::new(Mutex::new(ArchState {
                components_mapped: 0,
            })),
        }
    }
}

impl Agent for ArchitectureAgent {
    fn manifest(&self) -> AgentManifest {
        AgentManifest {
            id: self.id,
            name: "architecture".into(),
            kind: "system".into(),
            version: "0.1.0".into(),
            description: "Maps system architecture by discovering components, drivers, processors, and their relationships".into(),
            status: AgentStatus::Idle,
        }
    }

    fn tick(&self, kernel: &Kernel) -> Result<()> {
        // Map all registered drivers
        let drivers = kernel.drivers.list();
        for driver in &drivers {
            let driver_obj = json!({
                "name": driver.name,
                "description": driver.description,
                "capabilities": driver.capabilities,
                "type": "driver",
            });
            let obj_id = kernel.objects.create(
                "architecture_component",
                &format!("driver:{}", driver.name),
                &serde_json::to_vec(&driver_obj).unwrap(),
            );

            // Link capabilities to the driver
            for cap in &driver.capabilities {
                let cap_obj = json!({
                    "name": cap.name,
                    "description": cap.description,
                    "type": "capability",
                });
                let cap_id = kernel.objects.create(
                    "architecture_capability",
                    &format!("cap:{}:{}", driver.name, cap.name),
                    &serde_json::to_vec(&cap_obj).unwrap(),
                );
                kernel.objects.link(obj_id, cap_id, "provides");
            }
        }

        // Map all objects by kind to understand data landscape
        let all_kinds = [
            "entity",
            "observation",
            "artifact",
            "evidence",
            "finding",
            "architecture_component",
        ];
        for kind in &all_kinds {
            let objects = kernel.objects.find_by_kind(kind);
            if !objects.is_empty() {
                let summary = json!({
                    "kind": kind,
                    "count": objects.len(),
                    "type": "data_summary",
                });
                kernel.objects.create(
                    "architecture_data_summary",
                    &format!("data:{}", kind),
                    &serde_json::to_vec(&summary).unwrap(),
                );
            }
        }

        let mut state = self.state.lock().unwrap();
        state.components_mapped += 1;
        Ok(())
    }
}

// ─── Malware Agent ───────────────────────────────────────────────────────────

/// Analyzes binary artifacts for malicious patterns using available drivers.
///
/// Uses: kernel.drivers, kernel.storage, kernel.event
pub struct MalwareAgent {
    id: AgentId,
    state: Arc<Mutex<MalwareState>>,
}

struct MalwareState {
    analyses_performed: u64,
    threats_detected: u64,
}

impl MalwareAgent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: AgentId::new(),
            state: Arc::new(Mutex::new(MalwareState {
                analyses_performed: 0,
                threats_detected: 0,
            })),
        }
    }
}

impl Agent for MalwareAgent {
    fn manifest(&self) -> AgentManifest {
        AgentManifest {
            id: self.id,
            name: "malware".into(),
            kind: "analysis".into(),
            version: "0.1.0".into(),
            description: "Analyzes binary artifacts for malicious patterns using available analysis drivers".into(),
            status: AgentStatus::Idle,
        }
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        kernel.event.subscribe("agent.malware");
        tracing::info!(agent = "malware", "initialized");
        Ok(())
    }

    fn tick(&self, kernel: &Kernel) -> Result<()> {
        // Check if analysis drivers are available
        let has_binary_analysis = kernel
            .drivers
            .find_by_capability("parse_binary")
            .is_empty();
        let has_elf_analysis = kernel.drivers.find_by_capability("extract_symbols").is_empty();

        let analysis_ready = json!({
            "binary_analysis_available": !has_binary_analysis,
            "symbol_extraction_available": !has_elf_analysis,
            "timestamp": now().to_string(),
        });
        kernel.objects.create(
            "malware_capability_status",
            &format!("malware-cap-{}", Ulid::new()),
            &serde_json::to_vec(&analysis_ready).unwrap(),
        );

        let mut state = self.state.lock().unwrap();
        state.analyses_performed += 1;
        Ok(())
    }

    fn handle_event(&self, kernel: &Kernel, event: &EventEnvelope) -> Result<()> {
        // React to new artifacts being stored — attempt analysis
        if event.event_type == "Stored" && event.aggregate_type == "Artifact" {
            // Store analysis record as an object (analysis via drivers would happen here)
            let analysis = json!({
                "artifact_id": event.aggregate_id,
                "agent": "malware",
                "verdict": "pending",
                "timestamp": now().to_string(),
            });
            kernel.objects.create(
                "malware_analysis",
                &format!("analysis-{}", event.aggregate_id),
                &serde_json::to_vec(&analysis).unwrap(),
            );

            let mut state = self.state.lock().unwrap();
            state.analyses_performed += 1;
        }
        Ok(())
    }

    fn stop(&self, kernel: &Kernel) -> Result<()> {
        let state = self.state.lock().unwrap();
        kernel.event.publish(
            "agent.malware.stopped",
            format!(
                "{{ \"analyses\": {}, \"threats\": {} }}",
                state.analyses_performed, state.threats_detected
            )
            .as_bytes(),
        );
        Ok(())
    }
}

// ─── Monitoring Agent ────────────────────────────────────────────────────────

/// Monitors system health by polling kernel metrics.
///
/// Uses: kernel.scheduler, kernel.memory, kernel.event, kernel.event_store
pub struct MonitoringAgent {
    id: AgentId,
    state: Arc<Mutex<MonitorState>>,
}

struct MonitorState {
    ticks: u64,
    alerts_raised: u64,
}

impl MonitoringAgent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: AgentId::new(),
            state: Arc::new(Mutex::new(MonitorState {
                ticks: 0,
                alerts_raised: 0,
            })),
        }
    }
}

impl Agent for MonitoringAgent {
    fn manifest(&self) -> AgentManifest {
        AgentManifest {
            id: self.id,
            name: "monitoring".into(),
            kind: "system".into(),
            version: "0.1.0".into(),
            description: "Monitors system health — scheduler, memory, event store — and publishes heartbeat events".into(),
            status: AgentStatus::Idle,
        }
    }

    fn init(&self, kernel: &Kernel) -> Result<()> {
        kernel.event.subscribe("system.health");
        kernel.event.subscribe("agent.*");
        tracing::info!(agent = "monitoring", "initialized");
        Ok(())
    }

    fn tick(&self, kernel: &Kernel) -> Result<()> {
        // Collect system metrics
        let running_tasks = kernel.scheduler.running_count();
        let mem_stats = kernel.memory.stats();
        let total_events = kernel.event_store.total_count();
        let agent_count = kernel.agents.list().len() as u64;

        // Publish health heartbeat
        let heartbeat = json!({
            "running_tasks": running_tasks,
            "live_allocations": mem_stats.live_allocations,
            "total_allocated_bytes": mem_stats.allocated_bytes,
            "total_events": total_events,
            "agent_count": agent_count,
            "timestamp": now().to_string(),
        });
        kernel
            .event
            .publish("system.health", &serde_json::to_vec(&heartbeat).unwrap());

        // Store periodic health snapshot
        kernel.objects.create(
            "health_snapshot",
            &format!("health-{}", Ulid::new()),
            &serde_json::to_vec(&heartbeat).unwrap(),
        );

        // Alert if something is wrong
        if mem_stats.live_allocations > 1_000_000 {
            kernel.event.publish(
                "system.alert",
                b"{\"severity\":\"warn\",\"message\":\"high memory allocation count\"}",
            );
            let mut state = self.state.lock().unwrap();
            state.alerts_raised += 1;
        }

        let mut state = self.state.lock().unwrap();
        state.ticks += 1;
        Ok(())
    }
}

// ─── Documentation Agent ─────────────────────────────────────────────────────

/// Documents system state by discovering and recording component metadata.
///
/// Uses: kernel.objects, kernel.storage, kernel.drivers
pub struct DocumentationAgent {
    id: AgentId,
}

impl DocumentationAgent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: AgentId::new(),
        }
    }
}

impl Agent for DocumentationAgent {
    fn manifest(&self) -> AgentManifest {
        AgentManifest {
            id: self.id,
            name: "documentation".into(),
            kind: "system".into(),
            version: "0.1.0".into(),
            description: "Documents system state by discovering and recording component metadata, drivers, and stored data".into(),
            status: AgentStatus::Idle,
        }
    }

    fn tick(&self, kernel: &Kernel) -> Result<()> {
        // Document all drivers
        let drivers = kernel.drivers.list();
        let driver_doc = json!({
            "count": drivers.len(),
            "drivers": drivers,
            "type": "driver_inventory",
        });
        kernel.objects.create(
            "documentation",
            "docs-driver-inventory",
            &serde_json::to_vec(&driver_doc).unwrap(),
        );

        // Document storage layout
        let storage_keys = kernel.storage.list("");
        let storage_doc = json!({
            "key_count": storage_keys.len(),
            "keys": storage_keys,
            "type": "storage_inventory",
        });
        kernel.objects.create(
            "documentation",
            "docs-storage-inventory",
            &serde_json::to_vec(&storage_doc).unwrap(),
        );

        // Document object kinds in use
        let obj_kinds = [
            "entity",
            "observation",
            "artifact",
            "evidence",
            "finding",
            "architecture_component",
            "research_note",
            "health_snapshot",
            "malware_analysis",
            "documentation",
            "forensics_timeline",
        ];
        let mut kind_counts: Vec<serde_json::Value> = Vec::new();
        for kind in &obj_kinds {
            let count = kernel.objects.find_by_kind(kind).len();
            if count > 0 {
                kind_counts.push(json!({"kind": kind, "count": count}));
            }
        }
        let kind_doc = json!({
            "object_kinds": kind_counts,
            "type": "object_inventory",
        });
        kernel.objects.create(
            "documentation",
            "docs-object-inventory",
            &serde_json::to_vec(&kind_doc).unwrap(),
        );

        Ok(())
    }
}

// ─── Forensics Agent ─────────────────────────────────────────────────────────

/// Investigates events by building timelines and tracing relationships.
///
/// Uses: kernel.event_store, kernel.objects, kernel.snapshots, kernel.security
pub struct ForensicsAgent {
    id: AgentId,
    state: Arc<Mutex<ForensicsState>>,
}

struct ForensicsState {
    events_processed: u64,
    timelines_built: u64,
}

impl ForensicsAgent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: AgentId::new(),
            state: Arc::new(Mutex::new(ForensicsState {
                events_processed: 0,
                timelines_built: 0,
            })),
        }
    }
}

impl Agent for ForensicsAgent {
    fn manifest(&self) -> AgentManifest {
        AgentManifest {
            id: self.id,
            name: "forensics".into(),
            kind: "investigation".into(),
            version: "0.1.0".into(),
            description: "Investigates events by building timelines, tracing object relationships, and recording provenance chains".into(),
            status: AgentStatus::Idle,
        }
    }

    fn tick(&self, kernel: &Kernel) -> Result<()> {
        // Read all events and build a timeline summary by aggregate type
        let aggregate_types = [
            "Entity",
            "Observation",
            "Artifact",
            "Evidence",
            "Relationship",
            "Knowledge",
            "Finding",
            "Decision",
            "Service",
            "Investigation",
            "Timeline",
            "Agent",
        ];

        for agg_type in &aggregate_types {
            let count = kernel.event_store.count(agg_type);
            if count > 0 {
                let timeline_entry = json!({
                    "aggregate_type": agg_type,
                    "event_count": count,
                    "type": "forensics_summary",
                    "timestamp": now().to_string(),
                });
                kernel.objects.create(
                    "forensics_timeline",
                    &format!("forensics-{}-{}", agg_type.to_lowercase(), Ulid::new()),
                    &serde_json::to_vec(&timeline_entry).unwrap(),
                );
            }
        }

        // Trace relationships between known object kinds
        let known_kinds = [
            "entity",
            "observation",
            "artifact",
            "architecture_component",
            "research_note",
            "malware_analysis",
        ];
        for kind in &known_kinds {
            let objects = kernel.objects.find_by_kind(kind);
            for obj in &objects {
                let links = kernel.objects.links(obj.id);
                if !links.is_empty() {
                    let link_trace = json!({
                        "source_id": obj.id.to_string(),
                        "source_label": obj.label,
                        "link_count": links.len(),
                        "links": links.iter().map(|l| {
                            json!({
                                "id": l.id.to_string(),
                                "kind": l.kind,
                                "target_id": l.target_id.to_string(),
                            })
                        }).collect::<Vec<_>>(),
                        "type": "relationship_trace",
                    });
                    kernel.objects.create(
                        "forensics_relationship_trace",
                        &format!("trace-{}", obj.id),
                        &serde_json::to_vec(&link_trace).unwrap(),
                    );
                }
            }
        }

        let mut state = self.state.lock().unwrap();
        state.timelines_built += 1;
        Ok(())
    }

    fn handle_event(&self, kernel: &Kernel, event: &EventEnvelope) -> Result<()> {
        // Index every event as a forensics record
        let record = json!({
            "event_id": event.id.to_string(),
            "aggregate_id": event.aggregate_id,
            "aggregate_type": event.aggregate_type,
            "event_type": event.event_type,
            "version": event.version,
            "timestamp": event.timestamp.to_string(),
            "type": "event_record",
        });
        kernel.objects.create(
            "forensics_event_record",
            &format!("event-{}", event.id),
            &serde_json::to_vec(&record).unwrap(),
        );

        let mut state = self.state.lock().unwrap();
        state.events_processed += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tordex_core::Kernel;

    #[test]
    fn register_all_agents() {
        let kernel = Kernel::new();
        register_all(&kernel).unwrap();
        let agents = kernel.agents.list();
        assert_eq!(agents.len(), 6);

        let names: Vec<String> = agents.iter().map(|a| a.name.clone()).collect();
        assert!(names.contains(&"research".to_string()));
        assert!(names.contains(&"architecture".to_string()));
        assert!(names.contains(&"malware".to_string()));
        assert!(names.contains(&"monitoring".to_string()));
        assert!(names.contains(&"documentation".to_string()));
        assert!(names.contains(&"forensics".to_string()));
    }

    #[test]
    fn tick_all_agents() {
        let kernel = Kernel::new();
        register_all(&kernel).unwrap();

        // Seed some events so the forensics agent has data to summarize
        let event = EventEnvelope::new("e1".into(), "Entity", "Created", 1, json!({"name": "test"}));
        kernel.event_store.append(event).unwrap();
        let event = EventEnvelope::new("a1".into(), "Artifact", "Stored", 1, json!({"sha256": "abc"}));
        kernel.event_store.append(event).unwrap();

        // Initial tick — all agents run their logic
        kernel.agents.tick_all(&kernel).unwrap();

        // Verify agents left their mark via kernel objects
        let docs = kernel.objects.find_by_kind("documentation");
        assert!(!docs.is_empty(), "documentation agent should have created docs");

        let health = kernel.objects.find_by_kind("health_snapshot");
        assert!(!health.is_empty(), "monitoring agent should have created health snapshots");

        let timelines = kernel.objects.find_by_kind("forensics_timeline");
        assert!(!timelines.is_empty(), "forensics agent should have created timeline entries");
    }

    #[test]
    fn event_dispatching() {
        let kernel = Kernel::new();
        register_all(&kernel).unwrap();

        // Simulate an artifact being stored
        let envelope = EventEnvelope::new(
            "artifact-123".into(),
            "Artifact",
            "Stored",
            1,
            serde_json::json!({"sha256": "abc123"}),
        );
        kernel.agents.dispatch_event(&kernel, &envelope).unwrap();

        // Forensics agent should have recorded the event
        let records = kernel.objects.find_by_kind("forensics_event_record");
        assert!(!records.is_empty(), "forensics agent should have recorded the event");

        // Research agent should have created a note
        let notes = kernel.objects.find_by_kind("research_note");
        assert!(!notes.is_empty(), "research agent should have created a note");
    }

    #[test]
    fn agents_use_kernel_not_db() {
        let kernel = Kernel::new();
        register_all(&kernel).unwrap();

        // Tick all agents so they create objects
        kernel.agents.tick_all(&kernel).unwrap();

        // Verify all agents created objects through the kernel API
        let known_kinds = [
            "research_capability_map",
            "architecture_component",
            "architecture_data_summary",
            "malware_capability_status",
            "health_snapshot",
            "documentation",
            "forensics_timeline",
        ];
        let total: usize = known_kinds
            .iter()
            .map(|k| kernel.objects.find_by_kind(k).len())
            .sum();
        assert!(total > 0, "agents should create objects via kernel.objects");

        let all_storage = kernel.storage.list("");
        for key in &all_storage {
            assert!(
                key.starts_with("docs-"),
                "storage keys should only come from documentation agent inventory: {key}"
            );
        }
    }
}
