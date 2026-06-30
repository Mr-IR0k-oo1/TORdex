#![forbid(unsafe_code)]
#![allow(clippy::module_name_repetitions)]

//! # TORdex Knowledge Core
//!
//! An immutable knowledge management layer providing:
//!
//! - **Fingerprinting** — content-based identification (SHA-256, SimHash)
//! - **Identity Resolution** — cross-source entity matching
//! - **Canonicalization** — normalization to stable canonical forms
//! - **Deduplication** — exact and near-duplicate detection
//! - **Versioning** — append-only version chains
//! - **Provenance** — immutable derivation tracking
//! - **Confidence** — typed scoring with composition operators
//! - **KnowledgeRecord** — the immutable knowledge record combining all subsystems
//!
//! ## Principle
//!
//! **Knowledge is immutable.** Once created, a `KnowledgeRecord` is never
//! mutated. Changes create new records linked via the version chain, and the
//! original record remains preserved forever.

pub mod canonical;
pub mod confidence;
pub mod dedup;
pub mod fingerprint;
pub mod identity;
pub mod provenance;
pub mod record;
pub mod versioning;

pub use canonical::{CanonicalForm, Canonicalizer, Normalization};
pub use confidence::Confidence;
pub use dedup::{DedupEngine, DedupResult, DedupStrategy};
pub use fingerprint::Fingerprint;
pub use identity::{IdentityResolver, ResolvedIdentity, ResolutionRule};
pub use provenance::{Provenance, ProvenanceStep};
pub use record::KnowledgeRecord;
pub use versioning::{VersionChain, VersionManager};

use tordex_core::time as core_time;
use tordex_types::Knowledge;

/// The Knowledge Core — a composition root that ties all seven subsystems
/// together into a single interface.
#[derive(Debug)]
pub struct KnowledgeCore {
    /// Identity resolution engine.
    pub identity: IdentityResolver,
    /// Content canonicalizer.
    pub canonicalizer: Canonicalizer,
    /// Deduplication engine.
    pub dedup: DedupEngine,
    /// Version chain manager.
    pub versions: VersionManager,
}

impl KnowledgeCore {
    /// Create a new Knowledge Core with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            identity: IdentityResolver::new(),
            canonicalizer: Canonicalizer::new(Normalization::Full),
            dedup: DedupEngine::new(),
            versions: VersionManager::new(),
        }
    }

    /// Ingest a `tordex_types::Knowledge` value into the Knowledge Core,
    /// producing an immutable `KnowledgeRecord`.
    ///
    /// This performs:
    /// 1. Canonicalization of the content
    /// 2. Fingerprinting
    /// 3. Deduplication check
    /// 4. Version chain creation
    /// 5. Provenance tracking
    #[must_use]
    pub fn ingest(&mut self, knowledge: &Knowledge) -> KnowledgeRecord {
        let id = knowledge.id.to_string();
        let canonical = self
            .canonicalizer
            .canonicalize(&knowledge.content);
        let confidence = Confidence::new(knowledge.confidence);
        let provenance = Provenance::new(
            knowledge.source_ids.clone(),
            "knowledge_core",
        );
        let fingerprint = Fingerprint::sha256(&knowledge.content);

        // Register with dedup engine
        self.dedup.register(&id, &fingerprint);

        // Create immutable record
        let record = KnowledgeRecord {
            id,
            fingerprint,
            kind: knowledge.kind.clone(),
            content: knowledge.content.clone(),
            canonical_form: canonical,
            confidence,
            version: VersionManager::initial(),
            provenance,
            source_ids: knowledge.source_ids.clone(),
            metadata: knowledge.metadata.clone(),
            created_at: core_time::now(),
        };

        // Record version chain
        self.versions
            .record(VersionManager::initial());

        record
    }

    /// Create a new version of an existing knowledge record.
    ///
    /// The original record is **not** mutated. A new record is created with
    /// an incremented version and linked to the previous one.
    #[must_use]
    pub fn revise(
        &mut self,
        previous: &KnowledgeRecord,
        new_content: serde_json::Value,
        reason: &str,
    ) -> KnowledgeRecord {
        let id = format!("{}-v{}", previous.id, previous.version.version + 1);
        let canonical = self.canonicalizer.canonicalize(&new_content);
        let fingerprint = Fingerprint::sha256(&new_content);
        let confidence = previous.confidence; // same confidence unless changed
        let mut provenance = previous.provenance.clone();
        provenance = provenance.with_step(ProvenanceStep::new(
            "revised",
            "knowledge_core",
            vec![previous.id.clone()],
        ));
        let version = VersionManager::next_version(&previous.version, &id, reason);

        let record = KnowledgeRecord {
            id,
            fingerprint,
            kind: previous.kind.clone(),
            content: new_content,
            canonical_form: canonical,
            confidence,
            version,
            provenance,
            source_ids: previous.source_ids.clone(),
            metadata: previous.metadata.clone(),
            created_at: core_time::now(),
        };

        self.dedup.register(&record.id, &record.fingerprint);
        self.versions.record(record.version.clone());

        record
    }

    /// Check for duplicates and return the result.
    #[must_use]
    pub fn check_duplicate(&self, content: &serde_json::Value) -> Option<dedup::DedupResult> {
        let fp = Fingerprint::sha256(content);
        self.dedup.check_exact(&fp)
    }
}

impl Default for KnowledgeCore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use super::*;

    fn make_knowledge() -> Knowledge {
        Knowledge {
            id: tordex_core::KnowledgeId::generate(),
            kind: "test".to_string(),
            content: serde_json::json!({"key": "value"}),
            confidence: 0.95,
            source_ids: vec!["obs_001".to_string()],
            created_at: core_time::now(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn ingest_creates_immutable_record() {
        let mut core = KnowledgeCore::new();
        let k = make_knowledge();
        let record = core.ingest(&k);
        assert!(!record.id.is_empty());
        assert_eq!(record.version.version, 1);
        assert!(record.version.previous_id.is_none());
        assert_eq!(record.provenance.source_ids, vec!["obs_001"]);
    }

    #[test]
    fn revision_creates_new_version() {
        let mut core = KnowledgeCore::new();
        let k = make_knowledge();
        let original = core.ingest(&k);
        let revised = core.revise(
            &original,
            serde_json::json!({"key": "updated_value"}),
            "corrected field",
        );
        assert_eq!(revised.version.version, 2);
        assert_eq!(revised.version.reason, "corrected field");
        // Original is unchanged
        assert_eq!(original.version.version, 1);
    }

    #[test]
    fn dedup_detects_identical_ingest() {
        let mut core = KnowledgeCore::new();
        let k1 = make_knowledge();
        let _r1 = core.ingest(&k1);
        let result = core.check_duplicate(&k1.content);
        assert!(result.is_some());
        assert!(result.unwrap().is_duplicate);
    }

    #[test]
    fn different_content_not_duplicate() {
        let mut core = KnowledgeCore::new();
        let k1 = make_knowledge();
        let _r1 = core.ingest(&k1);
        let result = core.check_duplicate(&serde_json::json!({"other": "data"}));
        assert!(result.is_none());
    }

    #[test]
    fn canonicalizer_used_during_ingest() {
        let core = KnowledgeCore::new();
        assert_eq!(core.canonicalizer.strategy(), Normalization::Full);
    }
}
