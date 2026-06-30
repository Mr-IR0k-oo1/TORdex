//! Universal Object Model (UOM)
//!
//! Every future module uses these types. Each type is:
//! - A pure data structure (no behavior)
//! - Self-describing (kind, metadata)
//! - Traceable (provenance via source_ids)
//! - Temporal (created_at, timestamps)
//! - Serializable (serde)

pub mod events;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

pub use tordex_core::id::{
    ArtifactId, CollectionId, DecisionId, EntityId, EventId, EvidenceId, FindingId,
    InvestigationId, KnowledgeId, ObservationId, RelationshipId, ServiceId, SessionId, SourceId,
    TimelineId,
};

// ─── Shared Primitives ───────────────────────────────────────────────────────

/// Severity level for findings and decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Status of a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Proposed,
    Approved,
    Rejected,
    Executed,
    Failed,
}

/// Status of an investigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvestigationStatus {
    Open,
    Closed,
    Archived,
}

/// Status of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Active,
    Inactive,
    Error,
}

/// Generic metadata container.
pub type Metadata = HashMap<String, String>;

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

// ─── 1. Observation ──────────────────────────────────────────────────────────

/// A raw observation of something in the world — a DNS query, an HTTP request,
/// a file-system event, a sensor reading. Observations are the atomic unit of
/// evidence collection; everything else is derived from them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    /// Discriminator: "dns_query", "http_request", "file_read", etc.
    pub kind: String,
    /// Raw payload of the observation.
    pub data: Vec<u8>,
    /// MIME type or format label for `data`.
    pub content_type: Option<String>,
    /// Human-readable description of the observation source.
    pub source: String,
    /// When the thing was observed in the world.
    pub observed_at: OffsetDateTime,
    /// When this record was created.
    pub created_at: OffsetDateTime,
    /// Extensible key-value baggage.
    pub metadata: Metadata,
}

impl Observation {
    #[must_use]
    pub fn new(
        kind: &str,
        data: Vec<u8>,
        content_type: Option<&str>,
        source: &str,
        observed_at: OffsetDateTime,
    ) -> Self {
        Self {
            id: ObservationId::generate(),
            kind: kind.to_string(),
            data,
            content_type: content_type.map(String::from),
            source: source.to_string(),
            observed_at,
            created_at: now(),
            metadata: Metadata::new(),
        }
    }
}

// ─── 2. Artifact ─────────────────────────────────────────────────────────────

/// A collected piece of data produced by a collection session. Artifacts are
/// stored content — the HTML of a page, the bytes of a downloaded file, or
/// the result of running a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    /// The session that produced this artifact.
    pub session_id: SessionId,
    /// Discriminator: "html", "javascript", "binary", "screenshot", etc.
    pub kind: String,
    /// MIME type if known.
    pub content_type: Option<String>,
    /// Size in bytes.
    pub byte_count: u64,
    /// SHA-256 hex digest of the content.
    pub sha256: String,
    /// Storage backend path (e.g. MinIO key).
    pub storage_path: String,
    /// When this artifact was created.
    pub created_at: OffsetDateTime,
    /// Extensible metadata.
    pub metadata: Metadata,
}

// ─── 3. Evidence ─────────────────────────────────────────────────────────────

/// Processed or extracted evidence derived from one or more artifacts.
/// Evidence is the output of a processor — credentials found in a page,
/// endpoints extracted from JavaScript, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: EvidenceId,
    /// The artifact(s) this evidence was extracted from.
    pub artifact_id: ArtifactId,
    /// Discriminator: "credential", "endpoint", "dom_element", etc.
    pub kind: String,
    /// The extracted value — can be any valid JSON.
    pub value: serde_json::Value,
    /// Confidence in [0.0, 1.0].
    pub confidence: f64,
    pub created_at: OffsetDateTime,
    pub metadata: Metadata,
}

// ─── 4. Entity ───────────────────────────────────────────────────────────────

/// A real-world thing with identity — a person, an IP address, a domain name,
/// an organization. Entities are the nodes in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    /// Discriminator: "person", "ip_address", "domain", "organization", etc.
    pub kind: String,
    /// Primary display name.
    pub name: String,
    /// Arbitrary key-value attributes (e.g. {"asn": "15169", "country": "US"}).
    pub attributes: Metadata,
    /// First time this entity was observed.
    pub first_seen: OffsetDateTime,
    /// Most recent time this entity was observed.
    pub last_seen: OffsetDateTime,
    pub created_at: OffsetDateTime,
    pub metadata: Metadata,
}

// ─── 5. Relationship ─────────────────────────────────────────────────────────

/// A directed edge between two typed things. The source and target are each
/// identified by a (type, id) pair so relationships can link any combination
/// of Observation, Artifact, Evidence, Entity, Knowledge, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: RelationshipId,
    /// Discriminator: "resolves_to", "communicates_with", "owns", "contains", etc.
    pub kind: String,
    /// Type name of the source node ("entity", "evidence", "observation", etc.).
    pub source_type: String,
    pub source_id: String,
    /// Type name of the target node.
    pub target_type: String,
    pub target_id: String,
    /// Relationship-specific properties.
    pub properties: Metadata,
    /// First time this relationship was observed.
    pub first_seen: OffsetDateTime,
    /// Most recent time this relationship was observed.
    pub last_seen: OffsetDateTime,
    pub created_at: OffsetDateTime,
    pub metadata: Metadata,
}

// ─── 6. Knowledge ────────────────────────────────────────────────────────────

/// Structured knowledge produced by an analyzer. Knowledge is the result of
/// correlating, inferring, or otherwise reasoning about evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Knowledge {
    pub id: KnowledgeId,
    /// Discriminator: "pattern", "inference", "correlation", "classification", etc.
    pub kind: String,
    /// The knowledge payload.
    pub content: serde_json::Value,
    /// Confidence in [0.0, 1.0].
    pub confidence: f64,
    /// The evidence/observations that produced this knowledge.
    pub source_ids: Vec<String>,
    pub created_at: OffsetDateTime,
    pub metadata: Metadata,
}

// ─── 7. Finding ──────────────────────────────────────────────────────────────

/// A conclusion drawn from analysis — an indicator of compromise, a
/// vulnerability, an anomaly, a point of interest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: FindingId,
    /// The investigation this finding belongs to.
    pub investigation_id: InvestigationId,
    /// Discriminator: "indicator", "vulnerability", "anomaly", "intelligence", etc.
    pub kind: String,
    pub title: String,
    pub description: String,
    pub severity: Severity,
    /// Confidence in [0.0, 1.0].
    pub confidence: f64,
    /// Source evidence or knowledge that supports this finding.
    pub source_ids: Vec<String>,
    pub created_at: OffsetDateTime,
    pub metadata: Metadata,
}

// ─── 8. Decision ─────────────────────────────────────────────────────────────

/// An action or decision made in response to findings. Examples: "alert SOC",
/// "block IP", "escalate to tier 2", "ignore false positive".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: DecisionId,
    /// The findings that triggered this decision.
    pub finding_ids: Vec<FindingId>,
    /// Discriminator: "alert", "block", "ignore", "escalate", "report", etc.
    pub kind: String,
    /// Human-readable justification.
    pub rationale: String,
    /// Who or what made the decision.
    pub actor: String,
    pub status: DecisionStatus,
    pub created_at: OffsetDateTime,
    /// When the decision was carried out (if applicable).
    pub executed_at: Option<OffsetDateTime>,
    pub metadata: Metadata,
}

// ─── 9. Timeline ─────────────────────────────────────────────────────────────

/// A temporal sequence of events, observations, findings, or other timestamped
/// items scoped to an investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub id: TimelineId,
    pub investigation_id: InvestigationId,
    pub entries: Vec<TimelineEntry>,
    pub created_at: OffsetDateTime,
    pub metadata: Metadata,
}

/// A single entry in a timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub id: String,
    /// ISO-8601 timestamp of the event.
    pub timestamp: OffsetDateTime,
    /// Discriminator: "observation", "artifact", "evidence", "finding", etc.
    pub kind: String,
    /// The typed ID of the source item.
    pub source_id: String,
    /// One-line summary.
    pub summary: String,
    pub metadata: Metadata,
}

// ─── 10. Investigation ───────────────────────────────────────────────────────

/// A case, collection mission, or investigation that scopes work. Everything
/// (observations, artifacts, findings, decisions) belongs to an investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Investigation {
    pub id: InvestigationId,
    pub title: String,
    pub description: String,
    pub status: InvestigationStatus,
    /// Who owns this investigation.
    pub owner: String,
    pub tags: Vec<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub metadata: Metadata,
}

// ─── 11. Service ─────────────────────────────────────────────────────────────

/// A data source or target service that TORdex interacts with — a website to
/// collect from, an API to query, a database to store results in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: ServiceId,
    pub name: String,
    /// Discriminator: "website", "api", "database", "sensor", etc.
    pub kind: String,
    /// Connection URI or locator string.
    pub locator: String,
    pub status: ServiceStatus,
    /// Service version string if known.
    pub version: Option<String>,
    pub created_at: OffsetDateTime,
    pub metadata: Metadata,
}

// ─── Collection Types (kept from Phase 1) ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorKind {
    Http,
    BrowserLightpanda,
    BrowserChromium,
}

impl CollectorKind {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::BrowserLightpanda => "browser_lightpanda",
            Self::BrowserChromium => "browser_chromium",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "http" => Self::Http,
            "browser_lightpanda" => Self::BrowserLightpanda,
            "browser_chromium" => Self::BrowserChromium,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CollectionContext {
    pub collection_id: CollectionId,
    pub source_id: SourceId,
    pub url: String,
    pub cancel: tokio_util::sync::CancellationToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionStatus {
    Succeeded,
    Failed,
    Cancelled,
    RateLimited,
}

impl CollectionStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::RateLimited => "rate_limited",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "rate_limited" => Self::RateLimited,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CollectionResult {
    pub id: CollectionId,
    pub source_id: SourceId,
    pub collector: CollectorKind,
    pub status: CollectionStatus,
    pub started_at: OffsetDateTime,
    pub completed_at: OffsetDateTime,
    pub final_url: Option<String>,
    pub content_type: Option<String>,
    pub byte_count: u64,
    pub http_status: Option<http::StatusCode>,
    pub body: Option<bytes::Bytes>,
    pub error: Option<String>,
}

impl CollectionResult {
    #[must_use]
    pub fn failure(ctx: &CollectionContext, collector: CollectorKind, error: impl Into<String>) -> Self {
        let now = tordex_core::now();
        Self {
            id: ctx.collection_id,
            source_id: ctx.source_id,
            collector,
            status: CollectionStatus::Failed,
            started_at: now,
            completed_at: now,
            final_url: None,
            content_type: None,
            byte_count: 0,
            http_status: None,
            body: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Error)]
pub enum CollectionError {
    #[error("network error: {0}")]
    Network(String),
    #[error("timeout")]
    Timeout,
    #[error("rate limited")]
    RateLimited,
    #[error("cancelled")]
    Cancelled,
    #[error("browser backend unavailable: {0}")]
    BrowserUnavailable(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

// ─── Core Traits ─────────────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait Collector: Send + Sync {
    fn kind(&self) -> CollectorKind;
    async fn collect(&self, ctx: &CollectionContext) -> Result<CollectionResult, CollectionError>;
}

#[async_trait::async_trait]
pub trait Processor: Send + Sync {
    async fn process(&self, artifact: &Artifact) -> Result<Vec<Knowledge>, ProcessorError>;
}

#[async_trait::async_trait]
pub trait Analyzer: Send + Sync {
    async fn analyze(&self, knowledge: &Knowledge) -> Result<Vec<KnowledgeId>, AnalyzerError>;
}

#[async_trait::async_trait]
pub trait ArtifactStore: Send + Sync {
    async fn put(&self, key: &str, data: &[u8], content_type: Option<&str>) -> Result<String, StoreError>;
    async fn get(&self, key: &str) -> Result<Vec<u8>, StoreError>;
}

// ─── Error Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ProcessorError {
    #[error("processing failed: {0}")]
    Processing(String),
    #[error("unsupported artifact kind: {0}")]
    UnsupportedKind(String),
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
pub enum AnalyzerError {
    #[error("analysis failed: {0}")]
    Analysis(String),
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Observation ───────────────────────────────────────────────────────

    #[test]
    fn observation_new() {
        let obs = Observation::new(
            "dns_query",
            b"payload".to_vec(),
            Some("application/octet"),
            "resolver-1",
            OffsetDateTime::now_utc(),
        );
        assert_eq!(obs.kind, "dns_query");
        assert_eq!(obs.data, b"payload");
    }

    #[test]
    fn observation_serde_roundtrip() {
        let obs = Observation::new("test", vec![1, 2, 3], None, "src", OffsetDateTime::now_utc());
        let json = serde_json::to_string(&obs).unwrap();
        let deserialized: Observation = serde_json::from_str(&json).unwrap();
        assert_eq!(obs.id, deserialized.id);
        assert_eq!(obs.data, deserialized.data);
    }

    // ── Artifact ──────────────────────────────────────────────────────────

    #[test]
    fn artifact_struct() {
        let art = Artifact {
            id: ArtifactId::generate(),
            session_id: SessionId::generate(),
            kind: "html".into(),
            content_type: Some("text/html".into()),
            byte_count: 1024,
            sha256: "abc".into(),
            storage_path: "sessions/xxx/abc".into(),
            created_at: OffsetDateTime::now_utc(),
            metadata: Metadata::new(),
        };
        assert_eq!(art.kind, "html");
        assert_eq!(art.byte_count, 1024);
    }

    // ── Evidence ──────────────────────────────────────────────────────────

    #[test]
    fn evidence_defaults() {
        let ev = Evidence {
            id: EvidenceId::generate(),
            artifact_id: ArtifactId::generate(),
            kind: "credential".into(),
            value: serde_json::json!({"user": "admin", "pass": "hunter2"}),
            confidence: 0.95,
            created_at: OffsetDateTime::now_utc(),
            metadata: Metadata::new(),
        };
        assert_eq!(ev.kind, "credential");
        assert!((ev.confidence - 0.95).abs() < f64::EPSILON);
    }

    // ── Entity ────────────────────────────────────────────────────────────

    #[test]
    fn entity_attributes() {
        let mut attrs = Metadata::new();
        attrs.insert("asn".into(), "15169".into());
        let ent = Entity {
            id: EntityId::generate(),
            kind: "ip_address".into(),
            name: "8.8.8.8".into(),
            attributes: attrs,
            first_seen: OffsetDateTime::now_utc(),
            last_seen: OffsetDateTime::now_utc(),
            created_at: OffsetDateTime::now_utc(),
            metadata: Metadata::new(),
        };
        assert_eq!(ent.attributes.get("asn").unwrap(), "15169");
    }

    // ── Relationship ──────────────────────────────────────────────────────

    #[test]
    fn relationship_can_link_any_types() {
        let rel = Relationship {
            id: RelationshipId::generate(),
            kind: "resolves_to".into(),
            source_type: "entity".into(),
            source_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            target_type: "entity".into(),
            target_id: "01ARZ3NDEKTSV4RRFFQ69G5FBW".into(),
            properties: Metadata::new(),
            first_seen: OffsetDateTime::now_utc(),
            last_seen: OffsetDateTime::now_utc(),
            created_at: OffsetDateTime::now_utc(),
            metadata: Metadata::new(),
        };
        assert_eq!(rel.source_type, "entity");
        assert_eq!(rel.kind, "resolves_to");
    }

    // ── Knowledge ─────────────────────────────────────────────────────────

    #[test]
    fn knowledge_source_tracking() {
        let kn = Knowledge {
            id: KnowledgeId::generate(),
            kind: "correlation".into(),
            content: serde_json::json!({"pattern": "same-ip-different-domains"}),
            confidence: 0.8,
            source_ids: vec!["ev_001".into(), "ev_002".into()],
            created_at: OffsetDateTime::now_utc(),
            metadata: Metadata::new(),
        };
        assert_eq!(kn.source_ids.len(), 2);
    }

    // ── Finding ───────────────────────────────────────────────────────────

    #[test]
    fn finding_severity_ordering() {
        let f = Finding {
            id: FindingId::generate(),
            investigation_id: InvestigationId::generate(),
            kind: "indicator".into(),
            title: "Suspicious IP".into(),
            description: "IP 198.51.100.1 contacted known C2".into(),
            severity: Severity::High,
            confidence: 0.9,
            source_ids: vec![],
            created_at: OffsetDateTime::now_utc(),
            metadata: Metadata::new(),
        };
        assert_eq!(f.severity, Severity::High);
        assert!(Severity::Critical > Severity::High);
    }

    // ── Decision ──────────────────────────────────────────────────────────

    #[test]
    fn decision_lifecycle() {
        let d = Decision {
            id: DecisionId::generate(),
            finding_ids: vec![FindingId::generate()],
            kind: "block".into(),
            rationale: "Known malicious IP".into(),
            actor: "automation".into(),
            status: DecisionStatus::Proposed,
            created_at: OffsetDateTime::now_utc(),
            executed_at: None,
            metadata: Metadata::new(),
        };
        assert_eq!(d.status, DecisionStatus::Proposed);
        assert!(d.executed_at.is_none());
    }

    // ── Timeline ──────────────────────────────────────────────────────────

    #[test]
    fn timeline_entries() {
        let t = Timeline {
            id: TimelineId::generate(),
            investigation_id: InvestigationId::generate(),
            entries: vec![TimelineEntry {
                id: "entry_1".into(),
                timestamp: OffsetDateTime::now_utc(),
                kind: "observation".into(),
                source_id: "obs_001".into(),
                summary: "Initial observation".into(),
                metadata: Metadata::new(),
            }],
            created_at: OffsetDateTime::now_utc(),
            metadata: Metadata::new(),
        };
        assert_eq!(t.entries.len(), 1);
        assert_eq!(t.entries[0].summary, "Initial observation");
    }

    // ── Investigation ─────────────────────────────────────────────────────

    #[test]
    fn investigation_status_progression() {
        let inv = Investigation {
            id: InvestigationId::generate(),
            title: "APT-2026-001".into(),
            description: "Investigation into anomalous DNS traffic".into(),
            status: InvestigationStatus::Open,
            owner: "analyst@example.com".into(),
            tags: vec!["apt".into(), "dns".into()],
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            metadata: Metadata::new(),
        };
        assert_eq!(inv.status, InvestigationStatus::Open);
        assert_eq!(inv.tags.len(), 2);
    }

    // ── Service ───────────────────────────────────────────────────────────

    #[test]
    fn service_definition() {
        let svc = Service {
            id: ServiceId::generate(),
            name: "Example News".into(),
            kind: "website".into(),
            locator: "https://example.com".into(),
            status: ServiceStatus::Active,
            version: None,
            created_at: OffsetDateTime::now_utc(),
            metadata: Metadata::new(),
        };
        assert_eq!(svc.name, "Example News");
        assert_eq!(svc.status, ServiceStatus::Active);
    }

    // ── Enums ─────────────────────────────────────────────────────────────

    #[test]
    fn severity_deserialization() {
        let json = "\"critical\"";
        let s: Severity = serde_json::from_str(json).unwrap();
        assert_eq!(s, Severity::Critical);
    }

    #[test]
    fn decision_status_deserialization() {
        let json = "\"approved\"";
        let s: DecisionStatus = serde_json::from_str(json).unwrap();
        assert_eq!(s, DecisionStatus::Approved);
    }

    // ── Collector types (legacy) ──────────────────────────────────────────

    #[test]
    fn collector_kind_roundtrip() {
        for kind in &[
            CollectorKind::Http,
            CollectorKind::BrowserLightpanda,
            CollectorKind::BrowserChromium,
        ] {
            let s = kind.as_str();
            let parsed = CollectorKind::from_str(s);
            assert_eq!(parsed, Some(*kind));
        }
    }

    #[test]
    fn collection_status_roundtrip() {
        for status in &[
            CollectionStatus::Succeeded,
            CollectionStatus::Failed,
            CollectionStatus::Cancelled,
            CollectionStatus::RateLimited,
        ] {
            let s = status.as_str();
            let parsed = CollectionStatus::from_str(s);
            assert_eq!(parsed, Some(*status));
        }
    }
}
