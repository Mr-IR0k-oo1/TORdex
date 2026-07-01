//! Virtual Intelligence Filesystem (VIFS)
//!
//! A path-based abstraction over the kernel storage layer. Applications navigate
//! intelligence data like files — `/entities/`, `/artifacts/`, `/knowledge/`,
//! `/events/`, `/graphs/`, `/timelines/`, etc.
//!
//! Every UOM type maps to a top-level directory. Objects are addressed by path:
//! ```ignore
//! read("/entities/01ARZ3NDEKTSV4RRFFQ69G5FAV")
//! list("/entities/")
//! write("/entities/01ARZ3NDEKTSV4RRFFQ69G5FAV", entity_json)
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use ulid::Ulid;

use tordex_core::event_store::{EventStore, SystemEvent};
use tordex_core::id::{
    ArtifactId, DecisionId, EntityId, EvidenceId, FindingId, InvestigationId,
    KnowledgeId, ObservationId, RelationshipId, ServiceId, TimelineId,
};
use tordex_core::storage::StorageManager;
use tordex_types::{
    Artifact, Decision, Entity, Evidence, Finding, Investigation, Knowledge,
    Observation, Relationship, Service, Timeline,
};

// ─── Paths ───────────────────────────────────────────────────────────────────

/// Well-known VIFS root directories.
pub mod vifs_paths {
    pub const SERVICES: &str = "/services/";
    pub const ENTITIES: &str = "/entities/";
    pub const ARTIFACTS: &str = "/artifacts/";
    pub const KNOWLEDGE: &str = "/knowledge/";
    pub const EVENTS: &str = "/events/";
    pub const GRAPHS: &str = "/graphs/";
    pub const VECTORS: &str = "/vectors/";
    pub const TIMELINES: &str = "/timelines/";
    pub const REPOSITORIES: &str = "/repositories/";
    pub const OBSERVATIONS: &str = "/observations/";
    pub const EVIDENCE: &str = "/evidence/";
    pub const FINDINGS: &str = "/findings/";
    pub const DECISIONS: &str = "/decisions/";
    pub const INVESTIGATIONS: &str = "/investigations/";
    pub const RELATIONSHIPS: &str = "/relationships/";
}

const VIFS_PREFIX: &str = "vifs";

/// A validated virtual path (e.g. `/entities/01ARZ3NDEKTSV4RRFFQ69G5FAV`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VifsPath(String);

impl VifsPath {
    /// Root directory.
    #[must_use]
    pub fn root() -> Self {
        Self("/".to_string())
    }

    /// Build a path from components.
    #[must_use]
    pub fn join(base: &str, child: &str) -> Self {
        let base = base.strip_suffix('/').unwrap_or(base);
        let child = child.strip_prefix('/').unwrap_or(child);
        Self(format!("{base}/{child}"))
    }

    /// Path to an entity by ID.
    #[must_use]
    pub fn entity(id: &EntityId) -> Self {
        Self::join(vifs_paths::ENTITIES, &id.to_string())
    }

    /// Path to an artifact by ID.
    #[must_use]
    pub fn artifact(id: &ArtifactId) -> Self {
        Self::join(vifs_paths::ARTIFACTS, &id.to_string())
    }

    /// Path to an observation by ID.
    #[must_use]
    pub fn observation(id: &ObservationId) -> Self {
        Self::join(vifs_paths::OBSERVATIONS, &id.to_string())
    }

    /// Path to evidence by ID.
    #[must_use]
    pub fn evidence(id: &EvidenceId) -> Self {
        Self::join(vifs_paths::EVIDENCE, &id.to_string())
    }

    /// Path to a relationship by ID.
    #[must_use]
    pub fn relationship(id: &RelationshipId) -> Self {
        Self::join(vifs_paths::RELATIONSHIPS, &id.to_string())
    }

    /// Path to knowledge by ID.
    #[must_use]
    pub fn knowledge(id: &KnowledgeId) -> Self {
        Self::join(vifs_paths::KNOWLEDGE, &id.to_string())
    }

    /// Path to a finding by ID.
    #[must_use]
    pub fn finding(id: &FindingId) -> Self {
        Self::join(vifs_paths::FINDINGS, &id.to_string())
    }

    /// Path to a decision by ID.
    #[must_use]
    pub fn decision(id: &DecisionId) -> Self {
        Self::join(vifs_paths::DECISIONS, &id.to_string())
    }

    /// Path to a timeline by ID.
    #[must_use]
    pub fn timeline(id: &TimelineId) -> Self {
        Self::join(vifs_paths::TIMELINES, &id.to_string())
    }

    /// Path to an investigation by ID.
    #[must_use]
    pub fn investigation(id: &InvestigationId) -> Self {
        Self::join(vifs_paths::INVESTIGATIONS, &id.to_string())
    }

    /// Path to a service by ID.
    #[must_use]
    pub fn service(id: &ServiceId) -> Self {
        Self::join(vifs_paths::SERVICES, &id.to_string())
    }

    /// The raw virtual path string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parent directory path.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        let s = self.0.strip_suffix('/').unwrap_or(&self.0);
        let idx = s.rfind('/')?;
        if idx == 0 {
            return Some(Self("/".to_string()));
        }
        Some(Self(format!("/{}", &s[1..=idx])))
    }

    /// Last path component.
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        let s = self.0.strip_suffix('/').unwrap_or(&self.0);
        s.rsplit('/').next().filter(|n| !n.is_empty())
    }

    /// Whether this path looks like a directory (ends with `/`).
    #[must_use]
    pub fn is_directory(&self) -> bool {
        self.0.ends_with('/')
    }

    /// Convert to storage key (prepend `vifs/`).
    #[must_use]
    pub fn to_key(&self) -> String {
        let clean = self.0.strip_prefix('/').unwrap_or(&self.0);
        if clean.is_empty() {
            return VIFS_PREFIX.to_string();
        }
        format!("{VIFS_PREFIX}/{clean}")
    }
}

impl std::fmt::Display for VifsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for VifsPath {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// ─── Directory Entries ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VifsEntry {
    pub name: String,
    pub kind: EntryKind,
}

// ─── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum VifsError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("already exists: {0}")]
    AlreadyExists(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

impl From<serde_json::Error> for VifsError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

// ─── Vifs Trait ──────────────────────────────────────────────────────────────

/// The core filesystem interface. All operations are path-based.
pub trait Vifs: Send + Sync {
    /// Read the raw bytes at a path.
    fn read(&self, path: &VifsPath) -> Result<Vec<u8>, VifsError>;

    /// Write raw bytes to a path.
    fn write(&self, path: &VifsPath, data: &[u8]) -> Result<(), VifsError>;

    /// Delete the object at a path.
    fn delete(&self, path: &VifsPath) -> Result<(), VifsError>;

    /// List the contents of a directory.
    fn list(&self, path: &VifsPath) -> Result<Vec<VifsEntry>, VifsError>;

    /// Check if a path exists.
    fn exists(&self, path: &VifsPath) -> bool;

    /// Rename/move an object from one path to another.
    fn rename(&self, from: &VifsPath, to: &VifsPath) -> Result<(), VifsError>;
}

// ─── VirtualIntelligenceFilesystem ───────────────────────────────────────────

/// Default VIFS implementation backed by the kernel's `StorageManager`.
///
/// Paths are stored as flat keys prefixed with `vifs/`. Directory structure is
/// inferred from key prefixes.
pub struct VirtualIntelligenceFilesystem {
    storage: Arc<dyn StorageManager>,
}

impl VirtualIntelligenceFilesystem {
    #[must_use]
    pub fn new(storage: Arc<dyn StorageManager>) -> Self {
        Self { storage }
    }

    // ── Typed Write Helpers ───────────────────────────────────────────────

    pub fn write_observation(&self, obs: &Observation) -> Result<VifsPath, VifsError> {
        let path = VifsPath::observation(&obs.id);
        let data = serde_json::to_vec(obs)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_artifact(&self, art: &Artifact) -> Result<VifsPath, VifsError> {
        let path = VifsPath::artifact(&art.id);
        let data = serde_json::to_vec(art)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_evidence(&self, ev: &Evidence) -> Result<VifsPath, VifsError> {
        let path = VifsPath::evidence(&ev.id);
        let data = serde_json::to_vec(ev)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_entity(&self, entity: &Entity) -> Result<VifsPath, VifsError> {
        let path = VifsPath::entity(&entity.id);
        let data = serde_json::to_vec(entity)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_relationship(&self, rel: &Relationship) -> Result<VifsPath, VifsError> {
        let path = VifsPath::relationship(&rel.id);
        let data = serde_json::to_vec(rel)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_knowledge(&self, kn: &Knowledge) -> Result<VifsPath, VifsError> {
        let path = VifsPath::knowledge(&kn.id);
        let data = serde_json::to_vec(kn)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_finding(&self, finding: &Finding) -> Result<VifsPath, VifsError> {
        let path = VifsPath::finding(&finding.id);
        let data = serde_json::to_vec(finding)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_decision(&self, decision: &Decision) -> Result<VifsPath, VifsError> {
        let path = VifsPath::decision(&decision.id);
        let data = serde_json::to_vec(decision)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_timeline(&self, tl: &Timeline) -> Result<VifsPath, VifsError> {
        let path = VifsPath::timeline(&tl.id);
        let data = serde_json::to_vec(tl)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_investigation(&self, inv: &Investigation) -> Result<VifsPath, VifsError> {
        let path = VifsPath::investigation(&inv.id);
        let data = serde_json::to_vec(inv)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    pub fn write_service(&self, svc: &Service) -> Result<VifsPath, VifsError> {
        let path = VifsPath::service(&svc.id);
        let data = serde_json::to_vec(svc)?;
        self.write(&path, &data)?;
        Ok(path)
    }

    // ── Typed Read Helpers ────────────────────────────────────────────────

    pub fn read_observation(&self, id: &ObservationId) -> Result<Observation, VifsError> {
        let data = self.read(&VifsPath::observation(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_artifact(&self, id: &ArtifactId) -> Result<Artifact, VifsError> {
        let data = self.read(&VifsPath::artifact(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_evidence(&self, id: &EvidenceId) -> Result<Evidence, VifsError> {
        let data = self.read(&VifsPath::evidence(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_entity(&self, id: &EntityId) -> Result<Entity, VifsError> {
        let data = self.read(&VifsPath::entity(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_relationship(&self, id: &RelationshipId) -> Result<Relationship, VifsError> {
        let data = self.read(&VifsPath::relationship(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_knowledge(&self, id: &KnowledgeId) -> Result<Knowledge, VifsError> {
        let data = self.read(&VifsPath::knowledge(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_finding(&self, id: &FindingId) -> Result<Finding, VifsError> {
        let data = self.read(&VifsPath::finding(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_decision(&self, id: &DecisionId) -> Result<Decision, VifsError> {
        let data = self.read(&VifsPath::decision(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_timeline(&self, id: &TimelineId) -> Result<Timeline, VifsError> {
        let data = self.read(&VifsPath::timeline(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_investigation(&self, id: &InvestigationId) -> Result<Investigation, VifsError> {
        let data = self.read(&VifsPath::investigation(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn read_service(&self, id: &ServiceId) -> Result<Service, VifsError> {
        let data = self.read(&VifsPath::service(id))?;
        Ok(serde_json::from_slice(&data)?)
    }

    // ── Event-Sourced Projection ──────────────────────────────────────────

    /// Project a single `SystemEvent` into VIFS, creating or updating the
    /// corresponding UOM object.
    pub fn project_event(&self, event: &SystemEvent) -> Result<(), VifsError> {
        match event {
            SystemEvent::EntityCreated {
                id,
                kind,
                name,
                attributes,
                first_seen,
            } => {
                let eid: EntityId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid entity id: {id}"))
                })?;
                self.write_entity(&Entity {
                    id: eid,
                    kind: kind.clone(),
                    name: name.clone(),
                    attributes: attributes.clone(),
                    first_seen: *first_seen,
                    last_seen: *first_seen,
                    created_at: OffsetDateTime::now_utc(),
                    metadata: HashMap::new(),
                })?;
            }
            SystemEvent::EntityUpdated {
                id,
                kind,
                name,
                attributes,
            } => {
                let eid: EntityId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid entity id: {id}"))
                })?;
                let path = VifsPath::entity(&eid);
                if self.exists(&path) {
                    let mut entity = self.read_entity(&eid)?;
                    if let Some(k) = kind {
                        entity.kind = k.clone();
                    }
                    if let Some(n) = name {
                        entity.name = n.clone();
                    }
                    if let Some(a) = attributes {
                        entity.attributes = a.clone();
                    }
                    entity.last_seen = OffsetDateTime::now_utc();
                    self.write_entity(&entity)?;
                }
            }
            SystemEvent::EntityDeleted { id } => {
                let eid: EntityId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid entity id: {id}"))
                })?;
                self.delete(&VifsPath::entity(&eid))?;
            }

            SystemEvent::ObservationRecorded {
                id,
                kind,
                data,
                content_type,
                source,
                observed_at,
            } => {
                let oid: ObservationId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid observation id: {id}"))
                })?;
                self.write_observation(&Observation {
                    id: oid,
                    kind: kind.clone(),
                    data: data.clone(),
                    content_type: content_type.clone(),
                    source: source.clone(),
                    observed_at: *observed_at,
                    created_at: OffsetDateTime::now_utc(),
                    metadata: HashMap::new(),
                })?;
            }

            SystemEvent::ArtifactStored {
                id,
                session_id,
                kind,
                content_type,
                byte_count,
                sha256,
                storage_path,
            } => {
                let aid: ArtifactId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid artifact id: {id}"))
                })?;
                let sid: tordex_core::id::SessionId = session_id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid session id: {session_id}"))
                })?;
                self.write_artifact(&Artifact {
                    id: aid,
                    session_id: sid,
                    kind: kind.clone(),
                    content_type: content_type.clone(),
                    byte_count: *byte_count,
                    sha256: sha256.clone(),
                    storage_path: storage_path.clone(),
                    created_at: OffsetDateTime::now_utc(),
                    metadata: HashMap::new(),
                })?;
            }
            SystemEvent::ArtifactDeleted { id } => {
                let aid: ArtifactId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid artifact id: {id}"))
                })?;
                self.delete(&VifsPath::artifact(&aid))?;
            }

            SystemEvent::EvidenceExtracted {
                id,
                artifact_id,
                kind,
                value,
                confidence,
            } => {
                let eid: EvidenceId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid evidence id: {id}"))
                })?;
                let aid: ArtifactId = artifact_id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid artifact id: {artifact_id}"))
                })?;
                self.write_evidence(&Evidence {
                    id: eid,
                    artifact_id: aid,
                    kind: kind.clone(),
                    value: value.clone(),
                    confidence: *confidence,
                    created_at: OffsetDateTime::now_utc(),
                    metadata: HashMap::new(),
                })?;
            }

            SystemEvent::RelationshipEstablished {
                id,
                kind,
                source_type,
                source_id,
                target_type,
                target_id,
            } => {
                let rid: RelationshipId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid relationship id: {id}"))
                })?;
                self.write_relationship(&Relationship {
                    id: rid,
                    kind: kind.clone(),
                    source_type: source_type.clone(),
                    source_id: source_id.clone(),
                    target_type: target_type.clone(),
                    target_id: target_id.clone(),
                    properties: HashMap::new(),
                    first_seen: OffsetDateTime::now_utc(),
                    last_seen: OffsetDateTime::now_utc(),
                    created_at: OffsetDateTime::now_utc(),
                    metadata: HashMap::new(),
                })?;
            }
            SystemEvent::RelationshipDeleted { id } => {
                let rid: RelationshipId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid relationship id: {id}"))
                })?;
                self.delete(&VifsPath::relationship(&rid))?;
            }

            SystemEvent::KnowledgeProduced {
                id,
                kind,
                content,
                confidence,
                source_ids,
            } => {
                let kid: KnowledgeId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid knowledge id: {id}"))
                })?;
                self.write_knowledge(&Knowledge {
                    id: kid,
                    kind: kind.clone(),
                    content: content.clone(),
                    confidence: *confidence,
                    source_ids: source_ids.clone(),
                    created_at: OffsetDateTime::now_utc(),
                    metadata: HashMap::new(),
                })?;
            }

            SystemEvent::FindingCreated {
                id,
                investigation_id,
                kind,
                title,
                description,
                severity,
                confidence,
                source_ids,
            } => {
                let fid: FindingId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid finding id: {id}"))
                })?;
                let iid: InvestigationId = investigation_id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid investigation id: {investigation_id}"))
                })?;
                let sev = parse_severity(severity);
                self.write_finding(&Finding {
                    id: fid,
                    investigation_id: iid,
                    kind: kind.clone(),
                    title: title.clone(),
                    description: description.clone(),
                    severity: sev,
                    confidence: *confidence,
                    source_ids: source_ids.clone(),
                    created_at: OffsetDateTime::now_utc(),
                    metadata: HashMap::new(),
                })?;
            }
            SystemEvent::FindingUpdated {
                id,
                title,
                description,
                severity,
                confidence,
            } => {
                let fid: FindingId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid finding id: {id}"))
                })?;
                let path = VifsPath::finding(&fid);
                if self.exists(&path) {
                    let mut finding = self.read_finding(&fid)?;
                    if let Some(t) = title {
                        finding.title = t.clone();
                    }
                    if let Some(d) = description {
                        finding.description = d.clone();
                    }
                    if let Some(s) = severity {
                        finding.severity = parse_severity(s);
                    }
                    if let Some(c) = confidence {
                        finding.confidence = *c;
                    }
                    self.write_finding(&finding)?;
                }
            }

            SystemEvent::DecisionMade {
                id,
                finding_ids,
                kind,
                rationale,
                actor,
                status,
            } => {
                let did: DecisionId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid decision id: {id}"))
                })?;
                let fids: Vec<FindingId> = finding_ids
                    .iter()
                    .filter_map(|f| f.parse().ok())
                    .collect();
                let dst = parse_decision_status(status);
                self.write_decision(&Decision {
                    id: did,
                    finding_ids: fids,
                    kind: kind.clone(),
                    rationale: rationale.clone(),
                    actor: actor.clone(),
                    status: dst,
                    created_at: OffsetDateTime::now_utc(),
                    executed_at: None,
                    metadata: HashMap::new(),
                })?;
            }
            SystemEvent::DecisionExecuted { id } => {
                let did: DecisionId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid decision id: {id}"))
                })?;
                let path = VifsPath::decision(&did);
                if self.exists(&path) {
                    let mut decision = self.read_decision(&did)?;
                    decision.status = tordex_types::DecisionStatus::Executed;
                    decision.executed_at = Some(OffsetDateTime::now_utc());
                    self.write_decision(&decision)?;
                }
            }

            SystemEvent::ServiceRegistered {
                id,
                name,
                kind,
                locator,
                status,
                version,
            } => {
                let sid: ServiceId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid service id: {id}"))
                })?;
                let sst = parse_service_status(status);
                self.write_service(&Service {
                    id: sid,
                    name: name.clone(),
                    kind: kind.clone(),
                    locator: locator.clone(),
                    status: sst,
                    version: version.clone(),
                    created_at: OffsetDateTime::now_utc(),
                    metadata: HashMap::new(),
                })?;
            }
            SystemEvent::ServiceUpdated {
                id,
                name,
                status,
                version,
            } => {
                let sid: ServiceId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid service id: {id}"))
                })?;
                let path = VifsPath::service(&sid);
                if self.exists(&path) {
                    let mut svc = self.read_service(&sid)?;
                    if let Some(n) = name {
                        svc.name = n.clone();
                    }
                    if let Some(s) = status {
                        svc.status = parse_service_status(s);
                    }
                    if let Some(v) = version {
                        svc.version = Some(v.clone());
                    }
                    self.write_service(&svc)?;
                }
            }
            SystemEvent::ServiceDeleted { id } => {
                let sid: ServiceId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid service id: {id}"))
                })?;
                self.delete(&VifsPath::service(&sid))?;
            }

            SystemEvent::InvestigationOpened {
                id,
                title,
                description,
                owner,
                tags,
            } => {
                let iid: InvestigationId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid investigation id: {id}"))
                })?;
                self.write_investigation(&Investigation {
                    id: iid,
                    title: title.clone(),
                    description: description.clone(),
                    status: tordex_types::InvestigationStatus::Open,
                    owner: owner.clone(),
                    tags: tags.clone(),
                    created_at: OffsetDateTime::now_utc(),
                    updated_at: OffsetDateTime::now_utc(),
                    metadata: HashMap::new(),
                })?;
            }
            SystemEvent::InvestigationClosed { id } => {
                let iid: InvestigationId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid investigation id: {id}"))
                })?;
                let path = VifsPath::investigation(&iid);
                if self.exists(&path) {
                    let mut inv = self.read_investigation(&iid)?;
                    inv.status = tordex_types::InvestigationStatus::Closed;
                    inv.updated_at = OffsetDateTime::now_utc();
                    self.write_investigation(&inv)?;
                }
            }
            SystemEvent::InvestigationArchived { id } => {
                let iid: InvestigationId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid investigation id: {id}"))
                })?;
                let path = VifsPath::investigation(&iid);
                if self.exists(&path) {
                    let mut inv = self.read_investigation(&iid)?;
                    inv.status = tordex_types::InvestigationStatus::Archived;
                    inv.updated_at = OffsetDateTime::now_utc();
                    self.write_investigation(&inv)?;
                }
            }

            SystemEvent::TimelineEntryAdded {
                id,
                investigation_id,
                timestamp,
                kind,
                source_id,
                summary,
            } => {
                let tid: TimelineId = id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid timeline id: {id}"))
                })?;
                let iid: InvestigationId = investigation_id.parse().map_err(|_| {
                    VifsError::InvalidPath(format!("invalid investigation id: {investigation_id}"))
                })?;
                let entry = tordex_types::TimelineEntry {
                    id: id.clone(),
                    timestamp: *timestamp,
                    kind: kind.clone(),
                    source_id: source_id.clone(),
                    summary: summary.clone(),
                    metadata: HashMap::new(),
                };
                // Append to existing timeline or create new one
                let tpath = VifsPath::timeline(&tid);
                let mut timeline = if self.exists(&tpath) {
                    self.read_timeline(&tid)?
                } else {
                    Timeline {
                        id: tid,
                        investigation_id: iid,
                        entries: Vec::new(),
                        created_at: OffsetDateTime::now_utc(),
                        metadata: HashMap::new(),
                    }
                };
                timeline.entries.push(entry);
                self.write_timeline(&timeline)?;
            }

            SystemEvent::AgentStarted { id, name, kind, version } => {
                let record = serde_json::json!({
                    "id": id,
                    "name": name,
                    "kind": kind,
                    "version": version,
                    "status": "running",
                    "timestamp": OffsetDateTime::now_utc().to_string(),
                });
                let path = VifsPath::from(&format!("/agents/{id}") as &str);
                self.write(&path, &serde_json::to_vec(&record).unwrap())?;
            }

            SystemEvent::AgentStopped { id, reason } => {
                let record = serde_json::json!({
                    "id": id,
                    "status": "stopped",
                    "reason": reason,
                    "timestamp": OffsetDateTime::now_utc().to_string(),
                });
                let path = VifsPath::from(&format!("/agents/{id}") as &str);
                self.write(&path, &serde_json::to_vec(&record).unwrap())?;
            }

            SystemEvent::AgentHeartbeat { id, status, timestamp: _ } => {
                let record = serde_json::json!({
                    "agent_id": id,
                    "status": status,
                    "timestamp": OffsetDateTime::now_utc().to_string(),
                });
                let path = VifsPath::from(
                    &format!("/agents/{id}/heartbeats/{}", Ulid::new()) as &str,
                );
                self.write(&path, &serde_json::to_vec(&record).unwrap())?;
            }
        }
        Ok(())
    }

    /// Replay all events from an `EventStore` through the projector,
    /// populating VIFS with the current state.
    ///
    /// Returns the total number of events projected.
    pub fn replay_from_event_store(&self, store: &dyn EventStore) -> Result<u64, VifsError> {
        let agg_types = [
            "Entity", "Observation", "Artifact", "Evidence", "Relationship",
            "Knowledge", "Finding", "Decision", "Service", "Investigation", "Timeline",
        ];
        let mut total = 0u64;
        for agg_type in &agg_types {
            let events = store
                .read_all(agg_type)
                .map_err(|e| VifsError::Storage(e.to_string()))?;
            for envelope in &events {
                let system_event: SystemEvent = serde_json::from_value(envelope.data.clone())
                    .map_err(|e| VifsError::Serialization(e.to_string()))?;
                self.project_event(&system_event)?;
                total += 1;
            }
        }
        Ok(total)
    }
}

/// Parse a severity string from an event.
fn parse_severity(s: &str) -> tordex_types::Severity {
    match s {
        "info" => tordex_types::Severity::Info,
        "low" => tordex_types::Severity::Low,
        "medium" => tordex_types::Severity::Medium,
        "high" => tordex_types::Severity::High,
        "critical" => tordex_types::Severity::Critical,
        _ => tordex_types::Severity::Info,
    }
}

/// Parse a decision status string from an event.
fn parse_decision_status(s: &str) -> tordex_types::DecisionStatus {
    match s {
        "proposed" => tordex_types::DecisionStatus::Proposed,
        "approved" => tordex_types::DecisionStatus::Approved,
        "rejected" => tordex_types::DecisionStatus::Rejected,
        "executed" => tordex_types::DecisionStatus::Executed,
        "failed" => tordex_types::DecisionStatus::Failed,
        _ => tordex_types::DecisionStatus::Proposed,
    }
}

/// Parse a service status string from an event.
fn parse_service_status(s: &str) -> tordex_types::ServiceStatus {
    match s {
        "active" => tordex_types::ServiceStatus::Active,
        "inactive" => tordex_types::ServiceStatus::Inactive,
        "error" => tordex_types::ServiceStatus::Error,
        _ => tordex_types::ServiceStatus::Active,
    }
}

impl Vifs for VirtualIntelligenceFilesystem {
    fn read(&self, path: &VifsPath) -> Result<Vec<u8>, VifsError> {
        let key = path.to_key();
        let entry = self
            .storage
            .load(&key)
            .ok_or_else(|| VifsError::NotFound(path.to_string()))?;
        Ok(entry.value)
    }

    fn write(&self, path: &VifsPath, data: &[u8]) -> Result<(), VifsError> {
        let key = path.to_key();
        if key == VIFS_PREFIX {
            return Err(VifsError::InvalidPath("cannot write to root".into()));
        }
        self.storage.store(&key, data, Some("application/json"));
        Ok(())
    }

    fn delete(&self, path: &VifsPath) -> Result<(), VifsError> {
        let key = path.to_key();
        if self.storage.delete(&key) {
            Ok(())
        } else {
            Err(VifsError::NotFound(path.to_string()))
        }
    }

    fn list(&self, path: &VifsPath) -> Result<Vec<VifsEntry>, VifsError> {
        let prefix = path.to_key();
        let prefix = if prefix.ends_with('/') {
            prefix
        } else {
            format!("{prefix}/")
        };

        let keys = self.storage.list(&prefix);
        let mut seen: HashMap<String, bool> = HashMap::new();

        for key in &keys {
            let suffix = key.strip_prefix(&prefix).unwrap_or(key);
            if suffix.is_empty() {
                continue;
            }
            let parts: Vec<&str> = suffix.split('/').collect();
            if parts.is_empty() || parts[0].is_empty() {
                continue;
            }
            let name = parts[0].to_string();
            let is_dir = parts.len() > 1;
            seen.entry(name)
                .and_modify(|e| *e |= is_dir)
                .or_insert(is_dir);
        }

        let mut entries: Vec<VifsEntry> = seen
            .into_iter()
            .map(|(name, is_dir)| VifsEntry {
                name,
                kind: if is_dir { EntryKind::Directory } else { EntryKind::File },
            })
            .collect();

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn exists(&self, path: &VifsPath) -> bool {
        self.storage.exists(&path.to_key())
    }

    fn rename(&self, from: &VifsPath, to: &VifsPath) -> Result<(), VifsError> {
        let data = self.read(from)?;
        self.write(to, &data)?;
        self.delete(from)?;
        Ok(())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tordex_core::event_store::{EventEnvelope, InMemoryEventStore};
    use tordex_core::storage::InMemoryStorage;
    use time::OffsetDateTime;

    fn test_vifs() -> VirtualIntelligenceFilesystem {
        let storage = Arc::new(InMemoryStorage::new());
        VirtualIntelligenceFilesystem::new(storage)
    }

    // ── Path Tests ────────────────────────────────────────────────────────

    #[test]
    fn path_entity_construction() {
        let id = EntityId::generate();
        let path = VifsPath::entity(&id);
        assert!(path.as_str().contains(&id.to_string()));
        assert!(path.as_str().starts_with("/entities/"));
    }

    #[test]
    fn path_to_key() {
        let path = VifsPath::from("/entities/abc123");
        assert_eq!(path.to_key(), "vifs/entities/abc123");
    }

    #[test]
    fn path_root_to_key() {
        let path = VifsPath::root();
        assert_eq!(path.to_key(), "vifs");
    }

    #[test]
    fn path_parent() {
        let path = VifsPath::from("/entities/id1/relationships/r1");
        let parent = path.parent().unwrap();
        assert_eq!(parent.as_str(), "/entities/id1/relationships/");
    }

    #[test]
    fn path_parent_of_root() {
        assert!(VifsPath::root().parent().is_none());
    }

    #[test]
    fn path_file_name() {
        let path = VifsPath::from("/entities/abc123");
        assert_eq!(path.file_name(), Some("abc123"));
        let path = VifsPath::from("/");
        assert!(path.file_name().is_none());
    }

    #[test]
    fn path_is_directory() {
        assert!(VifsPath::from("/entities/").is_directory());
        assert!(!VifsPath::from("/entities/id1").is_directory());
    }

    #[test]
    fn path_join() {
        let path = VifsPath::join("/entities", "abc123");
        assert_eq!(path.as_str(), "/entities/abc123");
    }

    #[test]
    fn path_display() {
        let path = VifsPath::from("/services/my-svc");
        assert_eq!(format!("{path}"), "/services/my-svc");
    }

    // ── Filesystem Operations ─────────────────────────────────────────────

    #[test]
    fn write_and_read() {
        let vifs = test_vifs();
        let path = VifsPath::from("/entities/test-entity");
        vifs.write(&path, b"entity data").unwrap();
        let data = vifs.read(&path).unwrap();
        assert_eq!(data, b"entity data");
    }

    #[test]
    fn read_missing_errors() {
        let vifs = test_vifs();
        let path = VifsPath::from("/entities/nonexistent");
        let result = vifs.read(&path);
        assert!(matches!(result, Err(VifsError::NotFound(_))));
    }

    #[test]
    fn delete_existing() {
        let vifs = test_vifs();
        let path = VifsPath::from("/entities/to-delete");
        vifs.write(&path, b"data").unwrap();
        assert!(vifs.exists(&path));
        vifs.delete(&path).unwrap();
        assert!(!vifs.exists(&path));
    }

    #[test]
    fn delete_missing_errors() {
        let vifs = test_vifs();
        let path = VifsPath::from("/entities/ghost");
        let result = vifs.delete(&path);
        assert!(matches!(result, Err(VifsError::NotFound(_))));
    }

    #[test]
    fn rename_moves_content() {
        let vifs = test_vifs();
        let src = VifsPath::from("/entities/src");
        let dst = VifsPath::from("/entities/dst");
        vifs.write(&src, b"hello").unwrap();
        vifs.rename(&src, &dst).unwrap();
        assert!(!vifs.exists(&src));
        assert_eq!(vifs.read(&dst).unwrap(), b"hello");
    }

    #[test]
    fn list_root_shows_directories() {
        let vifs = test_vifs();
        vifs.write(&VifsPath::from("/entities/a"), b"1").unwrap();
        vifs.write(&VifsPath::from("/entities/b"), b"2").unwrap();
        vifs.write(&VifsPath::from("/artifacts/x"), b"3").unwrap();

        let root = vifs.list(&VifsPath::root()).unwrap();
        let names: Vec<&str> = root.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"entities"));
        assert!(names.contains(&"artifacts"));
    }

    #[test]
    fn list_returns_files_and_dirs() {
        let vifs = test_vifs();
        vifs.write(&VifsPath::from("/entities/a"), b"{}").unwrap();
        vifs.write(&VifsPath::from("/entities/a/rels/r1"), b"{}").unwrap();
        vifs.write(&VifsPath::from("/entities/b"), b"{}").unwrap();

        let entries = vifs.list(&VifsPath::from("/entities/")).unwrap();
        let a = entries.iter().find(|e| e.name == "a").unwrap();
        assert_eq!(a.kind, EntryKind::Directory);
        let b = entries.iter().find(|e| e.name == "b").unwrap();
        assert_eq!(b.kind, EntryKind::File);
    }

    #[test]
    fn write_to_root_errors() {
        let vifs = test_vifs();
        let result = vifs.write(&VifsPath::root(), b"data");
        assert!(matches!(result, Err(VifsError::InvalidPath(_))));
    }

    // ── Typed Operations ──────────────────────────────────────────────────

    #[test]
    fn write_and_read_entity() {
        let vifs = test_vifs();
        let entity = Entity {
            id: EntityId::generate(),
            kind: "ip_address".into(),
            name: "8.8.8.8".into(),
            attributes: [("asn".into(), "15169".into())].into(),
            first_seen: time::OffsetDateTime::now_utc(),
            last_seen: time::OffsetDateTime::now_utc(),
            created_at: time::OffsetDateTime::now_utc(),
            metadata: [("source".into(), "dns".into())].into(),
        };
        let path = vifs.write_entity(&entity).unwrap();
        assert!(vifs.exists(&path));
        let read_back = vifs.read_entity(&entity.id).unwrap();
        assert_eq!(read_back.id, entity.id);
        assert_eq!(read_back.name, "8.8.8.8");
    }

    #[test]
    fn write_and_read_observation() {
        let vifs = test_vifs();
        let obs = Observation::new(
            "dns_query",
            b"payload".to_vec(),
            None,
            "resolver",
            time::OffsetDateTime::now_utc(),
        );
        let path = vifs.write_observation(&obs).unwrap();
        let read_back = vifs.read_observation(&obs.id).unwrap();
        assert_eq!(read_back.kind, obs.kind);
        assert_eq!(read_back.data, obs.data);
    }

    #[test]
    fn write_and_read_artifact() {
        let vifs = test_vifs();
        let art = Artifact {
            id: ArtifactId::generate(),
            session_id: tordex_core::id::SessionId::generate(),
            kind: "html".into(),
            content_type: Some("text/html".into()),
            byte_count: 42,
            sha256: "abc".into(),
            storage_path: "sessions/x/abc".into(),
            created_at: time::OffsetDateTime::now_utc(),
            metadata: [("url".into(), "https://example.com".into())].into(),
        };
        let path = vifs.write_artifact(&art).unwrap();
        let read_back = vifs.read_artifact(&art.id).unwrap();
        assert_eq!(read_back.sha256, "abc");
    }

    #[test]
    fn write_and_read_service() {
        let vifs = test_vifs();
        let svc = Service {
            id: ServiceId::generate(),
            name: "Test Service".into(),
            kind: "website".into(),
            locator: "https://example.com".into(),
            status: tordex_types::ServiceStatus::Active,
            version: Some("1.0".into()),
            created_at: time::OffsetDateTime::now_utc(),
            metadata: [("team".into(), "sre".into())].into(),
        };
        let path = vifs.write_service(&svc).unwrap();
        let read_back = vifs.read_service(&svc.id).unwrap();
        assert_eq!(read_back.name, "Test Service");
    }

    #[test]
    fn write_and_read_finding() {
        let vifs = test_vifs();
        let finding = Finding {
            id: FindingId::generate(),
            investigation_id: InvestigationId::generate(),
            kind: "indicator".into(),
            title: "Bad IP".into(),
            description: "Known malicious".into(),
            severity: tordex_types::Severity::High,
            confidence: 0.95,
            source_ids: vec!["ev_001".into()],
            created_at: time::OffsetDateTime::now_utc(),
            metadata: [("mitre_id".into(), "T1071".into())].into(),
        };
        vifs.write_finding(&finding).unwrap();
        let read_back = vifs.read_finding(&finding.id).unwrap();
        assert_eq!(read_back.title, "Bad IP");
    }

    #[test]
    fn read_missing_typed() {
        let vifs = test_vifs();
        let result = vifs.read_entity(&EntityId::generate());
        assert!(matches!(result, Err(VifsError::NotFound(_))));
    }

    // ── Projector Tests ───────────────────────────────────────────────────

    fn test_event_store() -> InMemoryEventStore {
        InMemoryEventStore::new()
    }

    #[test]
    fn project_entity_created() {
        let vifs = test_vifs();
        let id = EntityId::generate();

        vifs.project_event(&SystemEvent::EntityCreated {
            id: id.to_string(),
            kind: "ip_address".into(),
            name: "10.0.0.1".into(),
            attributes: [("asn".into(), "64496".into())].into(),
            first_seen: OffsetDateTime::now_utc(),
        })
        .unwrap();

        let entity = vifs.read_entity(&id).unwrap();
        assert_eq!(entity.name, "10.0.0.1");
        assert_eq!(entity.kind, "ip_address");
    }

    #[test]
    fn project_entity_updated() {
        let vifs = test_vifs();
        let id = EntityId::generate();

        vifs.project_event(&SystemEvent::EntityCreated {
            id: id.to_string(),
            kind: "ip_address".into(),
            name: "10.0.0.1".into(),
            attributes: HashMap::new(),
            first_seen: OffsetDateTime::now_utc(),
        })
        .unwrap();

        vifs.project_event(&SystemEvent::EntityUpdated {
            id: id.to_string(),
            kind: None,
            name: Some("10.0.0.2".into()),
            attributes: None,
        })
        .unwrap();

        let entity = vifs.read_entity(&id).unwrap();
        assert_eq!(entity.name, "10.0.0.2");
    }

    #[test]
    fn project_entity_deleted() {
        let vifs = test_vifs();
        let id = EntityId::generate();

        vifs.project_event(&SystemEvent::EntityCreated {
            id: id.to_string(),
            kind: "ip".into(),
            name: "test".into(),
            attributes: HashMap::new(),
            first_seen: OffsetDateTime::now_utc(),
        })
        .unwrap();
        assert!(vifs.exists(&VifsPath::entity(&id)));

        vifs
            .project_event(&SystemEvent::EntityDeleted {
                id: id.to_string(),
            })
            .unwrap();
        assert!(!vifs.exists(&VifsPath::entity(&id)));
    }

    #[test]
    fn project_observation_recorded() {
        let vifs = test_vifs();
        let oid = ObservationId::generate();

        vifs
            .project_event(&SystemEvent::ObservationRecorded {
                id: oid.to_string(),
                kind: "dns_query".into(),
                data: b"example.com A record".to_vec(),
                content_type: Some("text/plain".into()),
                source: "resolver-1".into(),
                observed_at: OffsetDateTime::now_utc(),
            })
            .unwrap();

        let obs = vifs.read_observation(&oid).unwrap();
        assert_eq!(obs.kind, "dns_query");
        assert!(vifs.exists(&VifsPath::observation(&oid)));
    }

    #[test]
    fn project_artifact_stored() {
        let vifs = test_vifs();
        let aid = ArtifactId::generate();
        let sid = tordex_core::id::SessionId::generate();

        vifs
            .project_event(&SystemEvent::ArtifactStored {
                id: aid.to_string(),
                session_id: sid.to_string(),
                kind: "html".into(),
                content_type: Some("text/html".into()),
                byte_count: 100,
                sha256: "abc123".into(),
                storage_path: "sessions/x/abc123".into(),
            })
            .unwrap();

        let art = vifs.read_artifact(&aid).unwrap();
        assert_eq!(art.sha256, "abc123");
        assert_eq!(art.byte_count, 100);
    }

    #[test]
    fn project_evidence_extracted() {
        let vifs = test_vifs();
        let eid = EvidenceId::generate();
        let aid = ArtifactId::generate();

        vifs
            .project_event(&SystemEvent::EvidenceExtracted {
                id: eid.to_string(),
                artifact_id: aid.to_string(),
                kind: "credential".into(),
                value: serde_json::json!({"user": "admin"}),
                confidence: 0.95,
            })
            .unwrap();

        let ev = vifs.read_evidence(&eid).unwrap();
        assert_eq!(ev.kind, "credential");
    }

    #[test]
    fn project_finding_created() {
        let vifs = test_vifs();
        let fid = FindingId::generate();
        let iid = InvestigationId::generate();

        vifs
            .project_event(&SystemEvent::FindingCreated {
                id: fid.to_string(),
                investigation_id: iid.to_string(),
                kind: "indicator".into(),
                title: "Bad IP".into(),
                description: "Known C2".into(),
                severity: "high".into(),
                confidence: 0.9,
                source_ids: vec!["ev_001".into()],
            })
            .unwrap();

        let finding = vifs.read_finding(&fid).unwrap();
        assert_eq!(finding.title, "Bad IP");
        assert_eq!(finding.severity, tordex_types::Severity::High);
    }

    #[test]
    fn project_service_registered() {
        let vifs = test_vifs();
        let sid = ServiceId::generate();

        vifs
            .project_event(&SystemEvent::ServiceRegistered {
                id: sid.to_string(),
                name: "Test Service".into(),
                kind: "website".into(),
                locator: "https://example.com".into(),
                status: "active".into(),
                version: Some("1.0".into()),
            })
            .unwrap();

        let svc = vifs.read_service(&sid).unwrap();
        assert_eq!(svc.name, "Test Service");
        assert_eq!(svc.status, tordex_types::ServiceStatus::Active);
    }

    #[test]
    fn project_investigation_opened() {
        let vifs = test_vifs();
        let iid = InvestigationId::generate();

        vifs
            .project_event(&SystemEvent::InvestigationOpened {
                id: iid.to_string(),
                title: "APT-2026".into(),
                description: "DNS anomaly".into(),
                owner: "analyst".into(),
                tags: vec!["apt".into(), "dns".into()],
            })
            .unwrap();

        let inv = vifs.read_investigation(&iid).unwrap();
        assert_eq!(inv.status, tordex_types::InvestigationStatus::Open);
        assert_eq!(inv.tags.len(), 2);
    }

    #[test]
    fn project_timeline_entry_added() {
        let vifs = test_vifs();
        let tid = TimelineId::generate();
        let iid = InvestigationId::generate();

        vifs
            .project_event(&SystemEvent::TimelineEntryAdded {
                id: tid.to_string(),
                investigation_id: iid.to_string(),
                timestamp: OffsetDateTime::now_utc(),
                kind: "observation".into(),
                source_id: "obs_001".into(),
                summary: "Initial observation".into(),
            })
            .unwrap();

        let tl = vifs.read_timeline(&tid).unwrap();
        assert_eq!(tl.entries.len(), 1);
        assert_eq!(tl.entries[0].summary, "Initial observation");
    }

    #[test]
    fn replay_from_event_store_projects_all_events() {
        let vifs = test_vifs();
        let store = test_event_store();

        // Append events directly to the store
        let eid = EntityId::generate();
        let oid = ObservationId::generate();
        let sid = ServiceId::generate();

        let entity_event = SystemEvent::EntityCreated {
            id: eid.to_string(),
            kind: "ip".into(),
            name: "10.0.0.1".into(),
            attributes: HashMap::new(),
            first_seen: OffsetDateTime::now_utc(),
        };
        store
            .append(EventEnvelope::new(
                eid.to_string(),
                "Entity",
                "Created",
                1,
                serde_json::to_value(&entity_event).unwrap(),
            ))
            .unwrap();

        let obs_event = SystemEvent::ObservationRecorded {
            id: oid.to_string(),
            kind: "dns".into(),
            data: vec![1, 2, 3],
            content_type: None,
            source: "resolver".into(),
            observed_at: OffsetDateTime::now_utc(),
        };
        store
            .append(EventEnvelope::new(
                oid.to_string(),
                "Observation",
                "Recorded",
                1,
                serde_json::to_value(&obs_event).unwrap(),
            ))
            .unwrap();

        let svc_event = SystemEvent::ServiceRegistered {
            id: sid.to_string(),
            name: "Svc".into(),
            kind: "api".into(),
            locator: "https://svc".into(),
            status: "active".into(),
            version: None,
        };
        store
            .append(EventEnvelope::new(
                sid.to_string(),
                "Service",
                "Registered",
                1,
                serde_json::to_value(&svc_event).unwrap(),
            ))
            .unwrap();

        // Replay events into VIFS
        let count = vifs.replay_from_event_store(&store).unwrap();
        assert_eq!(count, 3);

        // Verify objects exist in VIFS
        assert!(vifs.exists(&VifsPath::entity(&eid)));
        assert!(vifs.exists(&VifsPath::observation(&oid)));
        assert!(vifs.exists(&VifsPath::service(&sid)));
    }

    #[test]
    fn replay_empty_store_produces_zero() {
        let vifs = test_vifs();
        let store = test_event_store();
        let count = vifs.replay_from_event_store(&store).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn project_investigation_closed() {
        let vifs = test_vifs();
        let iid = InvestigationId::generate();

        vifs
            .project_event(&SystemEvent::InvestigationOpened {
                id: iid.to_string(),
                title: "Case".into(),
                description: "Test".into(),
                owner: "analyst".into(),
                tags: vec![],
            })
            .unwrap();

        vifs
            .project_event(&SystemEvent::InvestigationClosed {
                id: iid.to_string(),
            })
            .unwrap();

        let inv = vifs.read_investigation(&iid).unwrap();
        assert_eq!(inv.status, tordex_types::InvestigationStatus::Closed);
    }

    #[test]
    fn project_decision_made() {
        let vifs = test_vifs();
        let did = DecisionId::generate();

        vifs
            .project_event(&SystemEvent::DecisionMade {
                id: did.to_string(),
                finding_ids: vec![],
                kind: "block".into(),
                rationale: "Malicious".into(),
                actor: "automation".into(),
                status: "proposed".into(),
            })
            .unwrap();

        let d = vifs.read_decision(&did).unwrap();
        assert_eq!(d.kind, "block");
        assert_eq!(d.status, tordex_types::DecisionStatus::Proposed);
    }
}
