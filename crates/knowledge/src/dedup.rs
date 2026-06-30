use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::fingerprint::Fingerprint;

/// Strategy to use when a duplicate knowledge record is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DedupStrategy {
    /// Discard the new duplicate entirely.
    Discard,
    /// Keep both records and link them as duplicates.
    KeepLinked,
    /// Replace the old record with the new one (only if new confidence is
    /// higher).
    ReplaceIfHigher,
    /// Merge the metadata of both records.
    MergeMetadata,
}

/// Result of a deduplication check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupResult {
    /// Whether a duplicate was found.
    pub is_duplicate: bool,
    /// ID of the existing duplicate record, if any.
    pub existing_id: Option<String>,
    /// Similarity score (0.0 = identical, 1.0 = completely different).
    pub similarity: f64,
    /// The strategy applied.
    pub strategy: DedupStrategy,
}

/// A deduplication engine that uses fingerprints to detect duplicate knowledge.
#[derive(Debug, Default)]
pub struct DedupEngine {
    /// Maps fingerprint hex -> knowledge ID for exact dedup.
    exact_index: HashMap<String, String>,
    /// Maps knowledge ID -> fingerprint hex for reverse lookup.
    reverse_index: HashMap<String, String>,
    /// SimHash fingerprints for near-duplicate detection.
    simhashes: Vec<(String, u64)>,
}

impl DedupEngine {
    /// Create a new empty dedup engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a knowledge record's fingerprint for future dedup checks.
    pub fn register(&mut self, knowledge_id: &str, fingerprint: &Fingerprint) {
        let hex = fingerprint.hex();
        self.exact_index.insert(hex.clone(), knowledge_id.to_string());
        self.reverse_index.insert(knowledge_id.to_string(), hex);
        if let Fingerprint::SimHash(h) = fingerprint {
            self.simhashes.push((knowledge_id.to_string(), *h));
        }
    }

    /// Check if a fingerprint already exists (exact dedup).
    #[must_use]
    pub fn check_exact(&self, fingerprint: &Fingerprint) -> Option<DedupResult> {
        let hex = fingerprint.hex();
        self.exact_index.get(&hex).map(|existing_id| DedupResult {
            is_duplicate: true,
            existing_id: Some(existing_id.clone()),
            similarity: 0.0,
            strategy: DedupStrategy::Discard,
        })
    }

    /// Check for near-duplicates using SimHash fingerprint.
    ///
    /// Returns the closest match whose Hamming distance is below `threshold`.
    #[must_use]
    pub fn check_simhash(&self, fingerprint: &Fingerprint, threshold: u32) -> Option<DedupResult> {
        let fp = match fingerprint {
            Fingerprint::SimHash(h) => *h,
            _ => return None,
        };
        let mut best: Option<(String, u32)> = None;
        for (kid, h) in &self.simhashes {
            let dist = (fp ^ h).count_ones();
            if dist <= threshold {
                match best {
                    Some((_, best_dist)) if dist < best_dist => {
                        best = Some((kid.clone(), dist));
                    }
                    None => {
                        best = Some((kid.clone(), dist));
                    }
                    _ => {}
                }
            }
        }
        best.map(|(id, dist)| DedupResult {
            is_duplicate: true,
            existing_id: Some(id),
            similarity: f64::from(dist) / 64.0,
            strategy: DedupStrategy::KeepLinked,
        })
    }

    /// Unregister a knowledge record.
    pub fn unregister(&mut self, knowledge_id: &str) {
        if let Some(hex) = self.reverse_index.remove(knowledge_id) {
            self.exact_index.remove(&hex);
        }
        self.simhashes.retain(|(k, _)| k != knowledge_id);
    }

    /// Number of registered records.
    #[must_use]
    pub fn count(&self) -> usize {
        self.exact_index.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::Fingerprint;

    #[test]
    fn exact_dedup_detects_identical() {
        let mut engine = DedupEngine::new();
        let fp = Fingerprint::content_hash(b"hello");
        engine.register("kn_001", &fp);
        let result = engine.check_exact(&fp);
        assert!(result.is_some());
        assert!(result.unwrap().is_duplicate);
    }

    #[test]
    fn exact_dedup_no_match() {
        let engine = DedupEngine::new();
        let fp = Fingerprint::content_hash(b"hello");
        assert!(engine.check_exact(&fp).is_none());
    }

    #[test]
    fn simhash_near_dedup() {
        let mut engine = DedupEngine::new();
        let fp1 = Fingerprint::simhash(b"hello world foo bar".split(|&b| b == b' '));
        engine.register("kn_001", &fp1);
        let fp2 = Fingerprint::simhash(b"hello world foo baz".split(|&b| b == b' '));
        let result = engine.check_simhash(&fp2, 10);
        assert!(result.is_some());
        assert!(result.unwrap().is_duplicate);
    }

    #[test]
    fn simhash_threshold_excludes_different() {
        let mut engine = DedupEngine::new();
        let fp1 = Fingerprint::simhash(b"aaaa bbbb cccc".split(|&b| b == b' '));
        engine.register("kn_001", &fp1);
        let fp2 = Fingerprint::simhash(b"dddd eeee ffff".split(|&b| b == b' '));
        assert!(engine.check_simhash(&fp2, 5).is_none());
    }

    #[test]
    fn unregister_removes_entry() {
        let mut engine = DedupEngine::new();
        let fp = Fingerprint::content_hash(b"data");
        engine.register("kn_001", &fp);
        engine.unregister("kn_001");
        assert!(engine.check_exact(&fp).is_none());
        assert_eq!(engine.count(), 0);
    }
}
