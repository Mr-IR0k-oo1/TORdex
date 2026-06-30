use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::diff::GraphDiff;
use crate::graph::GraphSnapshot;

/// The complete history of a temporal graph, stored as an ordered sequence
/// of snapshots. The history is **immutable** — snapshots are only appended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphHistory {
    /// Ordered snapshots (oldest first).
    snapshots: Vec<GraphSnapshot>,
}

impl GraphHistory {
    /// Create an empty graph history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    /// Append a snapshot to the history.
    pub fn append(&mut self, snapshot: GraphSnapshot) {
        self.snapshots.push(snapshot);
    }

    /// Number of snapshots in the history.
    #[must_use]
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether the history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Get all snapshots.
    #[must_use]
    pub fn all(&self) -> &[GraphSnapshot] {
        &self.snapshots
    }

    /// Get the latest snapshot.
    #[must_use]
    pub fn latest(&self) -> Option<&GraphSnapshot> {
        self.snapshots.last()
    }

    /// Get the earliest snapshot.
    #[must_use]
    pub fn earliest(&self) -> Option<&GraphSnapshot> {
        self.snapshots.first()
    }

    /// Query the state at a specific point in time.
    ///
    /// Returns the snapshot closest to (but not after) the given timestamp.
    #[must_use]
    pub fn state_at(&self, timestamp: OffsetDateTime) -> Option<&GraphSnapshot> {
        self.snapshots
            .iter()
            .rev()
            .find(|s| s.timestamp <= timestamp)
    }

    /// Compute the diff between two snapshots by index.
    #[must_use]
    pub fn diff_between(&self, from_idx: usize, to_idx: usize) -> Option<GraphDiff> {
        if from_idx >= self.snapshots.len() || to_idx >= self.snapshots.len() {
            return None;
        }
        Some(GraphDiff::between(
            &self.snapshots[from_idx],
            &self.snapshots[to_idx],
        ))
    }

    /// Get the diff between consecutive snapshots at the given index.
    #[must_use]
    pub fn diff_at(&self, idx: usize) -> Option<GraphDiff> {
        if idx == 0 || idx >= self.snapshots.len() {
            return None;
        }
        Some(GraphDiff::between(
            &self.snapshots[idx - 1],
            &self.snapshots[idx],
        ))
    }

    /// Get all consecutive diffs.
    #[must_use]
    pub fn all_diffs(&self) -> Vec<GraphDiff> {
        self.snapshots
            .windows(2)
            .map(|w| GraphDiff::between(&w[0], &w[1]))
            .collect()
    }
}

impl Default for GraphHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphSnapshot, TemporalNode};
    use crate::interval::TimeInterval;
    use std::collections::HashMap;
    use time::macros::datetime;

    fn snapshot(t: OffsetDateTime, node_ids: &[&str]) -> GraphSnapshot {
        let nodes: Vec<_> = node_ids
            .iter()
            .map(|id| TemporalNode {
                id: id.to_string(),
                kind: "entity".to_string(),
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
    fn empty_history() {
        let h = GraphHistory::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        assert!(h.latest().is_none());
    }

    #[test]
    fn append_and_latest() {
        let mut h = GraphHistory::new();
        h.append(snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"]));
        h.append(snapshot(datetime!(2024-01-02 0:00 UTC), &["n2"]));
        assert_eq!(h.len(), 2);
        assert_eq!(h.latest().unwrap().node_count(), 1);
        assert_eq!(h.latest().unwrap().nodes[0].id, "n2");
    }

    #[test]
    fn state_at_returns_closest_before() {
        let mut h = GraphHistory::new();
        h.append(snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"]));
        h.append(snapshot(datetime!(2024-01-03 0:00 UTC), &["n2"]));
        let state = h.state_at(datetime!(2024-01-02 0:00 UTC));
        assert!(state.is_some());
        assert_eq!(state.unwrap().nodes[0].id, "n1");
    }

    #[test]
    fn state_at_after_latest_returns_latest() {
        let mut h = GraphHistory::new();
        h.append(snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"]));
        let state = h.state_at(datetime!(2025-01-01 0:00 UTC));
        assert!(state.is_some());
        assert_eq!(state.unwrap().nodes[0].id, "n1");
    }

    #[test]
    fn state_at_before_earliest_returns_none() {
        let mut h = GraphHistory::new();
        h.append(snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"]));
        let state = h.state_at(datetime!(2023-01-01 0:00 UTC));
        assert!(state.is_none());
    }

    #[test]
    fn diff_between_valid_indices() {
        let mut h = GraphHistory::new();
        h.append(snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"]));
        h.append(snapshot(datetime!(2024-01-02 0:00 UTC), &["n1", "n2"]));
        let diff = h.diff_between(0, 1).unwrap();
        assert_eq!(diff.added_nodes.len(), 1);
        assert_eq!(diff.added_nodes[0].id, "n2");
    }

    #[test]
    fn diff_at_consecutive() {
        let mut h = GraphHistory::new();
        h.append(snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"]));
        h.append(snapshot(datetime!(2024-01-02 0:00 UTC), &["n1", "n2"]));
        let diff = h.diff_at(1).unwrap();
        assert_eq!(diff.added_nodes.len(), 1);
    }

    #[test]
    fn all_diffs_returns_all_consecutive_diffs() {
        let mut h = GraphHistory::new();
        h.append(snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"]));
        h.append(snapshot(datetime!(2024-01-02 0:00 UTC), &["n1", "n2"]));
        h.append(snapshot(datetime!(2024-01-03 0:00 UTC), &["n1"]));
        let diffs = h.all_diffs();
        assert_eq!(diffs.len(), 2);
    }
}
