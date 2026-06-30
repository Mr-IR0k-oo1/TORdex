use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// An immutable version chain for a knowledge record.
///
/// Each version links to its predecessor, forming an append-only chain.
/// Records are never mutated — new versions are always appended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionChain {
    /// The current version number (1-indexed).
    pub version: u64,
    /// The knowledge ID of the previous version, if any.
    pub previous_id: Option<String>,
    /// The knowledge ID of the next version, if any.
    pub next_id: Option<String>,
    /// When this version was created.
    pub created_at: OffsetDateTime,
    /// Human-readable reason for the new version.
    pub reason: String,
}

/// Manages version chains for knowledge records.
#[derive(Debug, Default)]
pub struct VersionManager {
    chains: Vec<VersionChain>,
}

impl VersionManager {
    /// Create a new empty version manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create the first version (v1) for a knowledge record.
    #[must_use]
    pub fn initial() -> VersionChain {
        VersionChain {
            version: 1,
            previous_id: None,
            next_id: None,
            created_at: OffsetDateTime::now_utc(),
            reason: "initial".to_string(),
        }
    }

    /// Create a new version linked from a previous one.
    ///
    /// Returns a new `VersionChain` representing the next version.
    /// This does **not** mutate the previous chain (immutable pattern).
    #[must_use]
    pub fn next_version(previous: &VersionChain, knowledge_id: &str, reason: &str) -> VersionChain {
        VersionChain {
            version: previous.version + 1,
            previous_id: previous.next_id.clone(),
            next_id: Some(knowledge_id.to_string()),
            created_at: OffsetDateTime::now_utc(),
            reason: reason.to_string(),
        }
    }

    /// Record a version chain.
    pub fn record(&mut self, chain: VersionChain) {
        self.chains.push(chain);
    }

    /// Find the version chain for a given knowledge record ID (the `next_id`
    /// field of the chain that links to this record). Returns the chain that
    /// produced this version.
    #[must_use]
    pub fn find(&self, knowledge_id: &str) -> Option<&VersionChain> {
        self.chains.iter().find(|c| {
            c.next_id.as_deref() == Some(knowledge_id)
        })
    }

    /// Walk the version chain backwards from a knowledge ID to the first version.
    #[must_use]
    pub fn history<'a>(&'a self, knowledge_id: &str) -> Vec<&'a VersionChain> {
        let mut result = Vec::new();
        let mut current_id = Some(knowledge_id.to_string());
        while let Some(cid) = current_id {
            if let Some(chain) = self.chains.iter().find(|c| c.next_id.as_deref() == Some(&cid)) {
                result.push(chain);
                current_id = chain.previous_id.clone();
            } else {
                // Check if this is the initial version
                if let Some(chain) = self.chains.iter().find(|c| {
                    c.version == 1 && c.next_id.as_deref() == Some(&cid)
                }) {
                    result.push(chain);
                }
                break;
            }
        }
        result
    }

    /// Number of recorded version chains.
    #[must_use]
    pub fn count(&self) -> usize {
        self.chains.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_version_is_v1() {
        let v = VersionManager::initial();
        assert_eq!(v.version, 1);
        assert!(v.previous_id.is_none());
        assert!(v.next_id.is_none());
        assert_eq!(v.reason, "initial");
    }

    #[test]
    fn next_version_increments() {
        let v1 = VersionManager::initial();
        let v2 = VersionManager::next_version(&v1, "kn_002", "refined");
        assert_eq!(v2.version, 2);
        assert_eq!(v2.reason, "refined");
        assert_eq!(v2.next_id.as_deref(), Some("kn_002"));
    }

    #[test]
    fn initial_is_immutable() {
        let v1 = VersionManager::initial();
        let _v2 = VersionManager::next_version(&v1, "kn_002", "updated");
        assert!(v1.next_id.is_none());
        assert_eq!(v1.version, 1);
    }

    #[test]
    fn record_and_find() {
        let mut vm = VersionManager::new();
        let v1 = VersionManager::initial();
        let v2 = VersionManager::next_version(&v1, "kn_002", "corrected");
        vm.record(v1);
        vm.record(v2);
        let found = vm.find("kn_002");
        assert!(found.is_some());
        assert_eq!(found.unwrap().version, 2);
    }
}
