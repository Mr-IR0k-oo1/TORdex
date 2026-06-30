use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

/// A processor that bridges the Temporal Graph Engine into the observation
/// pipeline.
///
/// Accepts `application/x-temporal-graph` content type and performs temporal
/// graph operations based on the `action` metadata:
/// - `"snapshot"` — record a snapshot of the current graph state
/// - `"state_at"` — query graph state at a given timestamp
/// - `"diff"` — compute diff between two snapshots (use `from_idx`, `to_idx`)
/// - `"evolution"` — compute evolution summary over all history
/// - `"predict"` — predict future graph state (use `target_time` ISO-8601)
/// - `"ingest_relationship"` — ingest a relationship as a temporal edge
pub struct TemporalGraphProcessor {
    engine: std::sync::Mutex<tordex_temporal_graph::TemporalGraph>,
}

impl TemporalGraphProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: std::sync::Mutex::new(tordex_temporal_graph::TemporalGraph::new()),
        }
    }
}

impl Default for TemporalGraphProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for TemporalGraphProcessor {
    fn name(&self) -> &str {
        "TemporalGraphProcessor"
    }

    fn description(&self) -> &str {
        "Bridges the Temporal Graph Engine — snapshot, diff, evolution, prediction, past state queries"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-temporal-graph"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let action = metadata
            .get("action")
            .map(|s| s.as_str())
            .unwrap_or("snapshot");

        let mut engine = self.engine.lock().map_err(|e| {
            ProcessorError::ProcessingFailed(format!("lock error: {e}"))
        })?;

        match action {
            "snapshot" => {
                let ts = metadata
                    .get("timestamp")
                    .and_then(|s| time::OffsetDateTime::parse(s, &time::format_description::well_known::Iso8601::DEFAULT).ok())
                    .unwrap_or_else(tordex_core::now);
                engine.snapshot(ts);
                let output = serde_json::json!({
                    "snapshot_count": engine.snapshot_count(),
                    "timestamp": ts.to_string(),
                });
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "temporal.snapshot",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-temporal-graph+snapshot",
                )])
            }
            "state_at" => {
                let ts = metadata
                    .get("timestamp")
                    .and_then(|s| time::OffsetDateTime::parse(s, &time::format_description::well_known::Iso8601::DEFAULT).ok())
                    .ok_or_else(|| ProcessorError::InvalidInput("missing or invalid 'timestamp' metadata".into()))?;
                let state = engine.state_at(ts);
                let output = serde_json::json!({
                    "found": state.is_some(),
                    "node_count": state.map(|s| s.node_count()).unwrap_or(0),
                    "edge_count": state.map(|s| s.edge_count()).unwrap_or(0),
                    "timestamp": ts.to_string(),
                });
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "temporal.state_at",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-temporal-graph+state",
                )])
            }
            "diff" => {
                let from: usize = metadata
                    .get("from_idx")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| ProcessorError::InvalidInput("missing or invalid 'from_idx'".into()))?;
                let to: usize = metadata
                    .get("to_idx")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| ProcessorError::InvalidInput("missing or invalid 'to_idx'".into()))?;
                let diff = engine.history.diff_between(from, to);
                let output = serde_json::json!({
                    "has_changes": diff.as_ref().map_or(false, |d| d.has_changes()),
                    "change_count": diff.as_ref().map(|d| d.change_count()).unwrap_or(0),
                    "change_rate": diff.as_ref().map(|d| d.change_rate()).unwrap_or(0.0),
                });
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "temporal.diff",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-temporal-graph+diff",
                )])
            }
            "evolution" => {
                let summary = engine.evolution();
                let output = serde_json::json!({
                    "has_evolution": summary.is_some(),
                    "node_growth_rate": summary.as_ref().map(|s| s.node_growth_rate).unwrap_or(0.0),
                    "edge_growth_rate": summary.as_ref().map(|s| s.edge_growth_rate).unwrap_or(0.0),
                    "node_survival_rate": summary.as_ref().map(|s| s.node_survival_rate).unwrap_or(0.0),
                    "edge_survival_rate": summary.as_ref().map(|s| s.edge_survival_rate).unwrap_or(0.0),
                    "node_churn_rate": summary.as_ref().map(|s| s.node_churn_rate).unwrap_or(0.0),
                    "edge_churn_rate": summary.as_ref().map(|s| s.edge_churn_rate).unwrap_or(0.0),
                });
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "temporal.evolution",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-temporal-graph+evolution",
                )])
            }
            "predict" => {
                let ts = metadata
                    .get("target_time")
                    .and_then(|s| time::OffsetDateTime::parse(s, &time::format_description::well_known::Iso8601::DEFAULT).ok())
                    .ok_or_else(|| ProcessorError::InvalidInput("missing or invalid 'target_time' metadata".into()))?;
                let prediction = engine.predict(ts);
                let output = serde_json::json!({
                    "has_prediction": prediction.is_some(),
                    "predicted_node_count": prediction.as_ref().map(|p| p.predicted_node_count).unwrap_or(0.0),
                    "predicted_edge_count": prediction.as_ref().map(|p| p.predicted_edge_count).unwrap_or(0.0),
                    "confidence": prediction.as_ref().map(|p| p.confidence.raw()).unwrap_or(0.0),
                    "node_uncertainty": prediction.as_ref().map(|p| p.node_uncertainty).unwrap_or(0.0),
                    "target_time": ts.to_string(),
                });
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "temporal.prediction",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-temporal-graph+prediction",
                )])
            }
            "ingest_relationship" => {
                let json_value: serde_json::Value =
                    serde_json::from_slice(data).map_err(|e| {
                        ProcessorError::InvalidInput(format!("invalid JSON: {e}"))
                    })?;
                let rel: tordex_types::Relationship =
                    serde_json::from_value(json_value).map_err(|e| {
                        ProcessorError::InvalidInput(format!(
                            "invalid relationship: {e}"
                        ))
                    })?;
                engine.ingest_relationship(&rel);
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "temporal.ingested",
                    serde_json::to_vec(&serde_json::json!({
                        "edge_id": rel.id.to_string(),
                        "source_id": rel.source_id,
                        "target_id": rel.target_id,
                    }))
                    .unwrap_or_default(),
                    "application/x-temporal-graph+ingested",
                )])
            }
            _ => Err(ProcessorError::InvalidInput(format!(
                "unknown action: {action}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_and_content_types() {
        let p = TemporalGraphProcessor::new();
        assert_eq!(p.name(), "TemporalGraphProcessor");
        assert!(p.content_types().contains(&"application/x-temporal-graph"));
    }

    #[test]
    fn snapshot_action_records() {
        let p = TemporalGraphProcessor::new();
        let results = p
            .process(
                "obs_001",
                b"{}",
                Some("application/x-temporal-graph"),
                HashMap::from([("action".into(), "snapshot".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "temporal.snapshot");
    }

    #[test]
    fn state_at_without_timestamp_errors() {
        let p = TemporalGraphProcessor::new();
        let result = p.process(
            "obs_001",
            b"{}",
            Some("application/x-temporal-graph"),
            HashMap::from([("action".into(), "state_at".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn state_at_with_timestamp() {
        let p = TemporalGraphProcessor::new();
        // First snapshot
        p.process(
            "obs_001",
            b"{}",
            Some("application/x-temporal-graph"),
            HashMap::from([("action".into(), "snapshot".into())]),
        )
        .unwrap();
        // Then query state
        let now = tordex_core::now();
        let ts_str = now
            .format(&time::format_description::well_known::Iso8601::DEFAULT)
            .unwrap();
        let results = p
            .process(
                "obs_002",
                b"{}",
                Some("application/x-temporal-graph"),
                HashMap::from([
                    ("action".into(), "state_at".into()),
                    ("timestamp".into(), ts_str),
                ]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "temporal.state_at");
    }

    #[test]
    fn diff_action() {
        let p = TemporalGraphProcessor::new();
        p.process(
            "obs_001",
            b"{}",
            Some("application/x-temporal-graph"),
            HashMap::from([("action".into(), "snapshot".into())]),
        )
        .unwrap();
        p.process(
            "obs_002",
            b"{}",
            Some("application/x-temporal-graph"),
            HashMap::from([("action".into(), "snapshot".into())]),
        )
        .unwrap();
        let results = p
            .process(
                "obs_003",
                b"{}",
                Some("application/x-temporal-graph"),
                HashMap::from([
                    ("action".into(), "diff".into()),
                    ("from_idx".into(), "0".into()),
                    ("to_idx".into(), "1".into()),
                ]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "temporal.diff");
    }

    #[test]
    fn evolution_action() {
        let p = TemporalGraphProcessor::new();
        p.process(
            "obs_001",
            b"{}",
            Some("application/x-temporal-graph"),
            HashMap::from([("action".into(), "snapshot".into())]),
        )
        .unwrap();
        p.process(
            "obs_002",
            b"{}",
            Some("application/x-temporal-graph"),
            HashMap::from([("action".into(), "snapshot".into())]),
        )
        .unwrap();
        let results = p
            .process(
                "obs_003",
                b"{}",
                Some("application/x-temporal-graph"),
                HashMap::from([("action".into(), "evolution".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "temporal.evolution");
    }

    #[test]
    fn predict_action() {
        let p = TemporalGraphProcessor::new();
        p.process(
            "obs_001",
            b"{}",
            Some("application/x-temporal-graph"),
            HashMap::from([("action".into(), "snapshot".into())]),
        )
        .unwrap();
        p.process(
            "obs_002",
            b"{}",
            Some("application/x-temporal-graph"),
            HashMap::from([("action".into(), "snapshot".into())]),
        )
        .unwrap();
        let target = (tordex_core::now() + time::Duration::days(7))
            .format(&time::format_description::well_known::Iso8601::DEFAULT)
            .unwrap();
        let results = p
            .process(
                "obs_003",
                b"{}",
                Some("application/x-temporal-graph"),
                HashMap::from([
                    ("action".into(), "predict".into()),
                    ("target_time".into(), target),
                ]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "temporal.prediction");
    }

    #[test]
    fn ingest_relationship_action() {
        let p = TemporalGraphProcessor::new();
        // Build a proper Relationship and serialize it to get the correct
        // time format that matches the types crate's serde configuration
        let rel = tordex_types::Relationship {
            id: tordex_core::RelationshipId::generate(),
            kind: "connects".to_string(),
            source_type: "entity".to_string(),
            source_id: "ent_001".to_string(),
            target_type: "entity".to_string(),
            target_id: "ent_002".to_string(),
            properties: std::collections::HashMap::new(),
            first_seen: time::macros::datetime!(2024-01-01 0:00 UTC),
            last_seen: time::macros::datetime!(2024-12-31 23:59 UTC),
            created_at: time::macros::datetime!(2024-01-01 0:00 UTC),
            metadata: std::collections::HashMap::new(),
        };
        let serialized = serde_json::to_vec(&rel).unwrap();
        let results = p
            .process(
                "obs_001",
                &serialized,
                Some("application/x-temporal-graph"),
                HashMap::from([("action".into(), "ingest_relationship".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "temporal.ingested");
    }

    #[test]
    fn unknown_action_returns_error() {
        let p = TemporalGraphProcessor::new();
        let result = p.process(
            "obs_001",
            b"{}",
            Some("application/x-temporal-graph"),
            HashMap::from([("action".into(), "bogus".into())]),
        );
        assert!(result.is_err());
    }
}
