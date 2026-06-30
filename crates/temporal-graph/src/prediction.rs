use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::evolution::EvolutionAnalyzer;
use crate::graph::GraphSnapshot;

use tordex_knowledge::Confidence;

/// A predicted future state of the temporal graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPrediction {
    /// When the prediction was made.
    pub predicted_at: OffsetDateTime,
    /// The point in time being predicted.
    pub target_time: OffsetDateTime,
    /// Confidence in the prediction.
    pub confidence: Confidence,
    /// Predicted node count.
    pub predicted_node_count: f64,
    /// Predicted edge count.
    pub predicted_edge_count: f64,
    /// Predicted new nodes (extrapolated from growth trend).
    pub predicted_new_nodes: Vec<PredictedNode>,
    /// Predicted new edges (extrapolated from growth trend).
    pub predicted_new_edges: Vec<PredictedEdge>,
    /// Confidence interval (±nodes).
    pub node_uncertainty: f64,
}

/// A predicted node that may appear in the future.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedNode {
    pub kind: String,
    pub probability: f64,
}

/// A predicted edge that may appear in the future.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedEdge {
    pub kind: String,
    pub probability: f64,
}

/// Makes predictions about future graph states based on historical evolution.
#[derive(Debug, Default)]
pub struct GraphPredictor {
    /// Observed node types and their appearance frequencies.
    node_type_frequencies: std::collections::HashMap<String, usize>,
    /// Observed edge types and their appearance frequencies.
    edge_type_frequencies: std::collections::HashMap<String, usize>,
    /// Total observations.
    total_observations: usize,
}

impl GraphPredictor {
    /// Create a new predictor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed historical snapshots to train the predictor.
    pub fn observe(&mut self, snapshots: &[GraphSnapshot]) {
        for snap in snapshots {
            for node in &snap.nodes {
                *self
                    .node_type_frequencies
                    .entry(node.kind.clone())
                    .or_insert(0) += 1;
            }
            for edge in &snap.edges {
                *self
                    .edge_type_frequencies
                    .entry(edge.kind.clone())
                    .or_insert(0) += 1;
            }
            self.total_observations += snap.nodes.len() + snap.edges.len();
        }
    }

    /// Predict the graph state at a future time, given historical snapshots.
    #[must_use]
    pub fn predict(
        &self,
        snapshots: &[GraphSnapshot],
        target_time: OffsetDateTime,
    ) -> Option<GraphPrediction> {
        let summary = EvolutionAnalyzer::analyze(snapshots)?;
        let last = snapshots.last()?;

        let time_delta = (target_time - last.timestamp).whole_seconds() as f64;

        // Extrapolate node/edge counts using the trend line
        let time_elapsed =
            (last.timestamp - summary.period.start).whole_seconds() as f64;

        let predicted_node_count =
            (summary.node_trend.slope * (time_elapsed + time_delta)
                + summary.node_trend.intercept)
                .max(0.0);
        let predicted_edge_count =
            (summary.edge_trend.slope * (time_elapsed + time_delta)
                + summary.edge_trend.intercept)
                .max(0.0);

        // Uncertainty grows with time
        let base_uncertainty = (last.node_count() as f64 * 0.1).max(1.0);
        let node_uncertainty =
            base_uncertainty * (1.0 + time_delta / 86400.0);

        // Predicted new node/edge types based on observed frequency
        let total_node_obs: usize = self.node_type_frequencies.values().sum();
        let predicted_new_nodes: Vec<PredictedNode> = self
            .node_type_frequencies
            .iter()
            .map(|(kind, count)| PredictedNode {
                kind: kind.clone(),
                probability: *count as f64 / total_node_obs.max(1) as f64,
            })
            .collect();

        let total_edge_obs: usize = self.edge_type_frequencies.values().sum();
        let predicted_new_edges: Vec<PredictedEdge> = self
            .edge_type_frequencies
            .iter()
            .map(|(kind, count)| PredictedEdge {
                kind: kind.clone(),
                probability: *count as f64 / total_edge_obs.max(1) as f64,
            })
            .collect();

        // Confidence decreases with prediction horizon
        let horizon_days = time_delta / 86400.0;
        let confidence = Confidence::new(
            (summary.node_trend.r_squared.max(0.0) * 0.7
                + summary.edge_trend.r_squared.max(0.0) * 0.3)
                * (1.0 / (1.0 + horizon_days * 0.1)),
        );

        Some(GraphPrediction {
            predicted_at: OffsetDateTime::now_utc(),
            target_time,
            confidence,
            predicted_node_count,
            predicted_edge_count,
            predicted_new_nodes,
            predicted_new_edges,
            node_uncertainty,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::TemporalNode;
    use crate::interval::TimeInterval;
    use std::collections::HashMap;
    use time::macros::datetime;
    use time::OffsetDateTime;

    fn snapshot(
        t: OffsetDateTime,
        node_ids: &[&str],
        kinds: &[&str],
    ) -> GraphSnapshot {
        let nodes: Vec<_> = node_ids
            .iter()
            .zip(kinds.iter())
            .map(|(id, kind)| TemporalNode {
                id: id.to_string(),
                kind: kind.to_string(),
                label: id.to_string(),
                interval: TimeInterval::starting_at(t),
                attributes: HashMap::new(),
            })
            .collect();
        GraphSnapshot {
            timestamp: t,
            nodes,
            edges: vec![],
        }
    }

    #[test]
    fn predict_requires_at_least_two_snapshots() {
        let predictor = GraphPredictor::new();
        let snap = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"], &["entity"]);
        let result = predictor.predict(&[snap], datetime!(2024-01-03 0:00 UTC));
        assert!(result.is_none());
    }

    #[test]
    fn predict_produces_forecast() {
        let mut predictor = GraphPredictor::new();
        let s1 = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"], &["entity"]);
        let s2 = snapshot(
            datetime!(2024-01-02 0:00 UTC),
            &["n1", "n2"],
            &["entity", "ip"],
        );
        let s3 = snapshot(
            datetime!(2024-01-03 0:00 UTC),
            &["n1", "n2", "n3"],
            &["entity", "ip", "domain"],
        );
        predictor.observe(&[s1.clone(), s2.clone(), s3.clone()]);
        let result = predictor.predict(&[s1, s2, s3], datetime!(2024-01-04 0:00 UTC));
        assert!(result.is_some());
        let pred = result.unwrap();
        assert!(pred.predicted_node_count > 0.0);
        assert!(pred.confidence.raw() > 0.0);
        assert_eq!(pred.predicted_new_nodes.len(), 3);
    }

    #[test]
    fn confidence_decreases_with_horizon() {
        let mut predictor = GraphPredictor::new();
        let s1 = snapshot(
            datetime!(2024-01-01 0:00 UTC),
            &["n1", "n2"],
            &["entity", "entity"],
        );
        let s2 = snapshot(
            datetime!(2024-01-02 0:00 UTC),
            &["n1", "n2", "n3"],
            &["entity", "entity", "ip"],
        );
        predictor.observe(&[s1.clone(), s2.clone()]);

        let near = predictor
            .predict(&[s1.clone(), s2.clone()], datetime!(2024-01-03 0:00 UTC))
            .unwrap();
        let far = predictor
            .predict(&[s1, s2], datetime!(2024-02-01 0:00 UTC))
            .unwrap();
        assert!(near.confidence.raw() >= far.confidence.raw());
    }

    #[test]
    fn uncertainty_increases_with_time() {
        let s1 = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"], &["entity"]);
        let s2 = snapshot(datetime!(2024-01-02 0:00 UTC), &["n1", "n2"], &["entity", "entity"]);
        let predictor = GraphPredictor::new();
        let near = predictor
            .predict(&[s1.clone(), s2.clone()], datetime!(2024-01-03 0:00 UTC))
            .unwrap();
        let far = predictor
            .predict(&[s1, s2], datetime!(2024-01-10 0:00 UTC))
            .unwrap();
        assert!(far.node_uncertainty > near.node_uncertainty);
    }
}
