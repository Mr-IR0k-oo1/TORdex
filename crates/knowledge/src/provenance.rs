use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// A provenance chain tracks the origin and derivation history of a knowledge
/// record.
///
/// Provenance is **immutable** — each step in the chain is a record that
/// cannot be altered after creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// The original source identifiers (observations, artifacts, evidence).
    pub source_ids: Vec<String>,
    /// Which processor or analyzer produced this knowledge.
    pub producer: String,
    /// The chain of derivation steps leading to this record.
    pub chain: Vec<ProvenanceStep>,
    /// When this provenance was recorded.
    pub recorded_at: OffsetDateTime,
}

/// A single step in a provenance chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceStep {
    /// The action taken (e.g., "extracted", "correlated", "inferred").
    pub action: String,
    /// The actor that performed the action (processor name, analyzer name).
    pub actor: String,
    /// Input knowledge IDs that were consumed.
    pub inputs: Vec<String>,
    /// Any configuration or parameters used.
    pub params: serde_json::Value,
    /// When this step occurred.
    pub timestamp: OffsetDateTime,
}

impl Provenance {
    /// Build a new provenance record from source evidence.
    #[must_use]
    pub fn new(source_ids: Vec<String>, producer: &str) -> Self {
        Self {
            source_ids,
            producer: producer.to_string(),
            chain: Vec::new(),
            recorded_at: OffsetDateTime::now_utc(),
        }
    }

    /// Add a derivation step to the chain.
    ///
    /// Returns a new `Provenance` with the step appended (immutable pattern).
    #[must_use]
    pub fn with_step(mut self, step: ProvenanceStep) -> Self {
        self.chain.push(step);
        self
    }
}

impl ProvenanceStep {
    /// Create a new provenance step.
    #[must_use]
    pub fn new(action: &str, actor: &str, inputs: Vec<String>) -> Self {
        Self {
            action: action.to_string(),
            actor: actor.to_string(),
            inputs,
            params: serde_json::Value::Null,
            timestamp: OffsetDateTime::now_utc(),
        }
    }

    /// Attach parameters to this step.
    #[must_use]
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = params;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_creates_with_source_ids() {
        let p = Provenance::new(vec!["obs_001".into(), "obs_002".into()], "test_processor");
        assert_eq!(p.source_ids.len(), 2);
        assert_eq!(p.producer, "test_processor");
        assert!(p.chain.is_empty());
    }

    #[test]
    fn provenance_chain_immutable_append() {
        let p = Provenance::new(vec!["obs_001".into()], "proc_a");
        let step = ProvenanceStep::new("correlated", "analyzer_x", vec!["kn_001".into()]);
        let p2 = p.clone().with_step(step);
        assert!(p.chain.is_empty());
        assert_eq!(p2.chain.len(), 1);
    }

    #[test]
    fn step_with_params() {
        let step = ProvenanceStep::new("inferred", "ml_model", vec![])
            .with_params(serde_json::json!({"threshold": 0.8}));
        assert_eq!(step.params["threshold"], 0.8);
    }

    #[test]
    fn provenance_serialization_roundtrip() {
        let p = Provenance::new(vec!["obs_001".into()], "test_proc");
        let json = serde_json::to_string(&p).unwrap();
        let back: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(p.producer, back.producer);
        assert_eq!(p.source_ids, back.source_ids);
    }
}
