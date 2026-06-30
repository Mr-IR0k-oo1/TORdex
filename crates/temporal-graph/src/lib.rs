#![forbid(unsafe_code)]
#![allow(clippy::module_name_repetitions)]

//! # TORdex Temporal Graph Engine
//!
//! A temporal graph engine where the graph understands time.
//!
//! ## Core Types
//!
//! - **`TemporalNode`** — a graph node with time-bounded existence
//! - **`TemporalEdge`** — a directed edge with time-bounded existence
//! - **`TimeInterval`** — start/end time interval
//! - **`GraphSnapshot`** — a point-in-time state of the graph
//! - **`GraphState`** — the current active state of the graph
//! - **`GraphHistory`** — ordered sequence of snapshots
//!
//! ## Subsystems
//!
//! - **History & Past State** (`GraphHistory`) — snapshot-based, query state at
//!   any point in time, compute diffs between snapshots
//! - **Diff** (`GraphDiff`) — delta computation between snapshots (added,
//!   removed, modified nodes/edges)
//! - **Evolution** (`EvolutionAnalyzer`) — trend analysis, growth/churn/survival
//!   rates over time
//! - **Prediction** (`GraphPredictor`) — extrapolate future states using linear
//!   regression on historical trends

pub mod diff;
pub mod evolution;
pub mod graph;
pub mod history;
pub mod interval;
pub mod prediction;

pub use diff::{AttributeDelta, EdgeChange, GraphDiff, NodeChange};
pub use evolution::{EvolutionAnalyzer, EvolutionSummary, TrendLine};
pub use graph::{GraphSnapshot, GraphState, TemporalEdge, TemporalNode};
pub use history::GraphHistory;
pub use interval::TimeInterval;
pub use prediction::{GraphPrediction, GraphPredictor, PredictedEdge, PredictedNode};

use tordex_types::Relationship;

/// The Temporal Graph Engine — a composition root that ties all subsystems
/// together.
#[derive(Debug, Default)]
pub struct TemporalGraph {
    /// Current graph state.
    pub state: GraphState,
    /// Full history of snapshots.
    pub history: GraphHistory,
    /// Evolution analyzer.
    pub analyzer: EvolutionAnalyzer,
    /// Graph predictor.
    pub predictor: GraphPredictor,
}

impl TemporalGraph {
    /// Create a new empty temporal graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingest a `tordex_types::Relationship` as a temporal edge.
    pub fn ingest_relationship(&mut self, rel: &Relationship) {
        let edge = TemporalEdge {
            id: rel.id.to_string(),
            kind: rel.kind.clone(),
            source_id: rel.source_id.clone(),
            target_id: rel.target_id.clone(),
            interval: TimeInterval::bounded(rel.first_seen, rel.last_seen),
            properties: rel.properties.clone(),
        };
        self.state.add_edge(edge);
    }

    /// Snapshot the current state and append to history.
    pub fn snapshot(&mut self, timestamp: time::OffsetDateTime) {
        let snap = self.state.snapshot(timestamp);
        self.history.append(snap);
        // Feed the new snapshot to the predictor
        if let Some(latest) = self.history.latest() {
            self.predictor.observe(&[latest.clone()]);
        }
    }

    /// Query the graph state at a past time.
    #[must_use]
    pub fn state_at(&self, timestamp: time::OffsetDateTime) -> Option<&GraphSnapshot> {
        self.history.state_at(timestamp)
    }

    /// Compute the evolution summary over the full history.
    #[must_use]
    pub fn evolution(&self) -> Option<EvolutionSummary> {
        EvolutionAnalyzer::analyze(self.history.all())
    }

    /// Predict the graph state at a future time.
    #[must_use]
    pub fn predict(&self, target: time::OffsetDateTime) -> Option<GraphPrediction> {
        self.predictor.predict(self.history.all(), target)
    }

    /// Number of snapshots in history.
    #[must_use]
    pub fn snapshot_count(&self) -> usize {
        self.history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn valid_ulid() -> tordex_core::RelationshipId {
        tordex_core::RelationshipId::generate()
    }

    fn relationship(src: &str, tgt: &str) -> Relationship {
        Relationship {
            id: valid_ulid(),
            kind: "connects".to_string(),
            source_type: "entity".to_string(),
            source_id: src.to_string(),
            target_type: "entity".to_string(),
            target_id: tgt.to_string(),
            properties: std::collections::HashMap::new(),
            first_seen: datetime!(2024-01-01 0:00 UTC),
            last_seen: datetime!(2024-12-31 23:59 UTC),
            created_at: datetime!(2024-01-01 0:00 UTC),
            metadata: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn ingest_relationship_adds_edge() {
        let mut tg = TemporalGraph::new();
        let rel = relationship("ent_001", "ent_002");
        let edge_id = rel.id.to_string();
        tg.ingest_relationship(&rel);
        assert_eq!(tg.state.edges.len(), 1);
        assert_eq!(tg.state.edges[&edge_id].source_id, "ent_001");
    }

    #[test]
    fn snapshot_appends_to_history() {
        let mut tg = TemporalGraph::new();
        tg.snapshot(datetime!(2024-01-01 0:00 UTC));
        tg.snapshot(datetime!(2024-01-02 0:00 UTC));
        assert_eq!(tg.snapshot_count(), 2);
    }

    #[test]
    fn state_at_returns_past_state() {
        let mut tg = TemporalGraph::new();
        let rel = relationship("ent_001", "ent_002");
        tg.ingest_relationship(&rel);
        tg.snapshot(datetime!(2024-06-15 0:00 UTC));
        // Query after the snapshot timestamp — the snapshot captured the state
        // at 2024-06-15, and the query time is within the edge's interval
        let past = tg.state_at(datetime!(2024-07-01 0:00 UTC));
        assert!(past.is_some(), "state_at should return the snapshot when query time is after snapshot time");
    }

    #[test]
    fn evolution_analyzes_history() {
        let mut tg = TemporalGraph::new();
        tg.snapshot(datetime!(2024-01-01 0:00 UTC));
        tg.snapshot(datetime!(2024-01-02 0:00 UTC));
        tg.snapshot(datetime!(2024-01-03 0:00 UTC));
        let evo = tg.evolution();
        assert!(evo.is_some());
    }

    #[test]
    fn predict_needs_history() {
        let tg = TemporalGraph::new();
        let pred = tg.predict(datetime!(2025-01-01 0:00 UTC));
        assert!(pred.is_none());
    }

    #[test]
    fn temporal_graph_serialization() {
        let mut tg = TemporalGraph::new();
        tg.snapshot(datetime!(2024-01-01 0:00 UTC));
        let json = serde_json::to_string(&tg.history).unwrap();
        let back: GraphHistory = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 1);
    }
}
