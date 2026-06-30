use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A resolved entity identity, linking all known aliases to a single canonical
/// identifier.
///
/// Identity resolution is **immutable** — once an entity is resolved, new
/// aliases produce a new resolution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedIdentity {
    /// The canonical entity ID (ULID string).
    pub canonical_id: String,
    /// All known aliases for this entity, keyed by source namespace.
    pub aliases: HashMap<String, String>,
    /// When this resolution was first created.
    pub created_at: time::OffsetDateTime,
}

/// An identity resolution rule that determines when two identifiers refer to
/// the same real-world entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionRule {
    /// Two identifiers match exactly (same value in the same namespace).
    Exact,
    /// Two identifiers match via a known equivalence (e.g., email ↔ username).
    Equivalence {
        namespace_a: String,
        namespace_b: String,
    },
    /// Match via fingerprint (content-based identity).
    FingerprintMatch,
    /// Fuzzy match via SimHash distance below a threshold.
    Fuzzy { threshold: u32 },
}

/// The identity resolution engine.
///
/// Maintains a registry of resolved identities and applies rules to match
/// new identifiers against known entities.
#[derive(Debug, Default)]
pub struct IdentityResolver {
    resolved: HashMap<String, ResolvedIdentity>,
    alias_index: HashMap<(String, String), String>, // (namespace, value) -> canonical_id
}

impl IdentityResolver {
    /// Create a new empty resolver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a known resolution.
    pub fn register(&mut self, identity: ResolvedIdentity) {
        let cid = identity.canonical_id.clone();
        for (ns, val) in &identity.aliases {
            self.alias_index.insert((ns.clone(), val.clone()), cid.clone());
        }
        self.resolved.insert(cid.clone(), identity);
    }

    /// Try to resolve an identifier to a canonical entity.
    ///
    /// Returns `Some(canonical_id)` if a match is found, `None` otherwise.
    #[must_use]
    pub fn resolve(&self, namespace: &str, value: &str) -> Option<&str> {
        self.alias_index
            .get(&(namespace.to_string(), value.to_string()))
            .map(|s| s.as_str())
    }

    /// Resolve using a fingerprint. If the fingerprint matches any known entity,
    /// return its canonical ID.
    #[must_use]
    pub fn resolve_by_fingerprint(
        &self,
        fingerprint_hex: &str,
    ) -> Option<&str> {
        let ns = "fingerprint";
        self.alias_index
            .get(&(ns.to_string(), fingerprint_hex.to_string()))
            .map(|s| s.as_str())
    }

    /// Find possible fuzzy matches by comparing all known aliases against a
    /// candidate value using a distance function. Returns matches sorted by
    /// decreasing similarity.
    pub fn fuzzy_resolve(
        &self,
        _namespace: &str,
        value: &str,
        threshold: u32,
    ) -> Vec<(String, u32)> {
        let value_bytes = value.as_bytes();
        let mut results = Vec::new();
        for ((ns, alias), cid) in &self.alias_index {
            let dist = levenshtein_distance(value_bytes, alias.as_bytes());
            if dist <= threshold {
                results.push((format!("{}:{}:{}", ns, alias, cid), dist));
            }
        }
        results.sort_by_key(|(_, d)| *d);
        results
    }

    /// List all resolved identities.
    #[must_use]
    pub fn all(&self) -> impl Iterator<Item = &ResolvedIdentity> {
        self.resolved.values()
    }

    /// Number of resolved identities.
    #[must_use]
    pub fn count(&self) -> usize {
        self.resolved.len()
    }
}

fn levenshtein_distance(a: &[u8], b: &[u8]) -> u32 {
    let m = a.len();
    let n = b.len();
    let mut prev: Vec<u32> = (0..=n as u32).collect();
    for i in 1..=m {
        let mut curr = vec![i as u32; n + 1];
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (curr[j - 1] + 1)
                .min(prev[j] + 1)
                .min(prev[j - 1] + cost);
        }
        prev = curr;
    }
    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    #[test]
    fn exact_resolution() {
        let mut resolver = IdentityResolver::new();
        let mut aliases = HashMap::new();
        aliases.insert("email".into(), "user@example.com".into());
        aliases.insert("username".into(), "jdoe".into());
        let identity = ResolvedIdentity {
            canonical_id: "ent_001".into(),
            aliases,
            created_at: OffsetDateTime::now_utc(),
        };
        resolver.register(identity);
        assert_eq!(resolver.resolve("email", "user@example.com"), Some("ent_001"));
        assert_eq!(resolver.resolve("username", "jdoe"), Some("ent_001"));
        assert_eq!(resolver.resolve("email", "other@example.com"), None);
    }

    #[test]
    fn fingerprint_resolution() {
        let mut resolver = IdentityResolver::new();
        let mut aliases = HashMap::new();
        aliases.insert("fingerprint".into(), "abc123".into());
        let identity = ResolvedIdentity {
            canonical_id: "ent_002".into(),
            aliases,
            created_at: OffsetDateTime::now_utc(),
        };
        resolver.register(identity);
        assert_eq!(resolver.resolve_by_fingerprint("abc123"), Some("ent_002"));
        assert_eq!(resolver.resolve_by_fingerprint("xyz"), None);
    }

    #[test]
    fn fuzzy_resolve_finds_close_matches() {
        let mut resolver = IdentityResolver::new();
        let mut aliases = HashMap::new();
        aliases.insert("domain".into(), "example.com".into());
        resolver.register(ResolvedIdentity {
            canonical_id: "ent_003".into(),
            aliases,
            created_at: OffsetDateTime::now_utc(),
        });
        let matches = resolver.fuzzy_resolve("domain", "examp1e.com", 2);
        assert!(!matches.is_empty());
    }

    #[test]
    fn no_match_returns_none() {
        let resolver = IdentityResolver::new();
        assert_eq!(resolver.resolve("email", "test@test.com"), None);
    }

    #[test]
    fn count_tracks_registrations() {
        let mut resolver = IdentityResolver::new();
        assert_eq!(resolver.count(), 0);
        resolver.register(ResolvedIdentity {
            canonical_id: "e1".into(),
            aliases: HashMap::new(),
            created_at: OffsetDateTime::now_utc(),
        });
        assert_eq!(resolver.count(), 1);
    }

    #[test]
    fn levenshtein_distance_works() {
        assert_eq!(levenshtein_distance(b"kitten", b"sitting"), 3);
        assert_eq!(levenshtein_distance(b"hello", b"hello"), 0);
        assert_eq!(levenshtein_distance(b"abc", b""), 3);
    }
}
