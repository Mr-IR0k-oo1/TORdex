use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::canonical::CanonicalForm;
use crate::confidence::Confidence;
use crate::fingerprint::Fingerprint;
use crate::provenance::Provenance;
use crate::versioning::{VersionChain, VersionManager};

/// An **immutable** knowledge record.
///
/// Once created, a `KnowledgeRecord` is never mutated. Any change produces a
/// new record linked via the version chain. This enforces the immutability
/// principle of the Knowledge Core.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRecord {
    /// Unique identifier (ULID string).
    pub id: String,
    /// Content-based fingerprint for dedup and identity.
    pub fingerprint: Fingerprint,
    /// Discriminator: "pattern", "inference", "correlation", etc.
    pub kind: String,
    /// The knowledge payload.
    pub content: serde_json::Value,
    /// Canonical form of the content (for comparison).
    pub canonical_form: CanonicalForm,
    /// Confidence score.
    pub confidence: Confidence,
    /// Version chain (immutable version history).
    pub version: VersionChain,
    /// Provenance chain (origin and derivation history).
    pub provenance: Provenance,
    /// Source observation/artifact/evidence IDs.
    pub source_ids: Vec<String>,
    /// Extensible key-value metadata.
    pub metadata: std::collections::HashMap<String, String>,
    /// When this record was created.
    pub created_at: OffsetDateTime,
}

impl KnowledgeRecord {
    /// Create a new immutable knowledge record.
    #[must_use]
    pub fn new(
        id: String,
        kind: String,
        content: serde_json::Value,
        canonical_form: CanonicalForm,
        confidence: Confidence,
        provenance: Provenance,
    ) -> Self {
        let fingerprint = Fingerprint::sha256(&content);
        let source_ids = provenance.source_ids.clone();
        Self {
            id,
            fingerprint,
            kind,
            content,
            canonical_form,
            confidence,
            version: VersionManager::initial(),
            provenance,
            source_ids,
            metadata: std::collections::HashMap::new(),
            created_at: OffsetDateTime::now_utc(),
        }
    }

    /// Attach metadata to this record.
    ///
    /// Since records are immutable, this returns a **new** record with the
    /// metadata merged.
    #[must_use]
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Check if this record is a duplicate of another based on fingerprint.
    #[must_use]
    pub fn is_duplicate_of(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
    }

    /// Compute Hamming distance to another record's fingerprint.
    #[must_use]
    pub fn fingerprint_distance(&self, other: &Self) -> Option<u32> {
        self.fingerprint.hamming_distance(&other.fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(id: &str) -> KnowledgeRecord {
        let content = serde_json::json!({"key": "value"});
        let canonical = crate::canonical::Canonicalizer::new(
            crate::canonical::Normalization::SortedKeys,
        ).canonicalize(&content);
        let provenance = Provenance::new(vec![], "test");
        KnowledgeRecord::new(
            id.to_string(),
            "test".to_string(),
            content,
            canonical,
            Confidence::CERTAIN,
            provenance,
        )
    }

    #[test]
    fn record_creates_with_fingerprint() {
        let r = make_record("kn_001");
        assert!(matches!(r.fingerprint, Fingerprint::Sha256(_)));
        assert_eq!(r.version.version, 1);
        assert!(r.version.previous_id.is_none());
    }

    #[test]
    fn duplicate_detection() {
        let a = make_record("kn_001");
        let b = make_record("kn_002");
        assert!(a.is_duplicate_of(&b));
    }

    #[test]
    fn different_content_not_duplicate() {
        let a = make_record("kn_001");
        let content = serde_json::json!({"key": "different"});
        let canonical = crate::canonical::Canonicalizer::new(
            crate::canonical::Normalization::SortedKeys,
        ).canonicalize(&content);
        let provenance = Provenance::new(vec![], "test");
        let b = KnowledgeRecord::new(
            "kn_002".to_string(),
            "test".to_string(),
            content,
            canonical,
            Confidence::CERTAIN,
            provenance,
        );
        assert!(!a.is_duplicate_of(&b));
    }

    #[test]
    fn metadata_returns_new_record() {
        let r = make_record("kn_001");
        let r2 = r.clone().with_metadata("key", "val");
        assert!(r.metadata.is_empty());
        assert_eq!(r2.metadata.get("key").unwrap(), "val");
    }

    #[test]
    fn record_serialization_roundtrip() {
        let r = make_record("kn_001");
        let json = serde_json::to_string(&r).unwrap();
        let back: KnowledgeRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(r.id, back.id);
        assert_eq!(r.fingerprint, back.fingerprint);
        assert_eq!(r.confidence.raw(), back.confidence.raw());
    }
}
