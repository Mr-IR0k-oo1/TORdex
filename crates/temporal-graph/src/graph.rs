use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::interval::TimeInterval;

/// A node in the temporal graph, with a time-bounded existence interval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalNode {
    /// Unique identifier.
    pub id: String,
    /// Discriminator: "entity", "observation", "artifact", etc.
    pub kind: String,
    /// Human-readable label.
    pub label: String,
    /// When this node exists in the graph.
    pub interval: TimeInterval,
    /// Key-value attributes.
    pub attributes: HashMap<String, String>,
}

/// A directed edge in the temporal graph, with a time-bounded existence interval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalEdge {
    /// Unique identifier.
    pub id: String,
    /// Discriminator: "resolves_to", "communicates_with", "contains", etc.
    pub kind: String,
    /// Source node ID.
    pub source_id: String,
    /// Target node ID.
    pub target_id: String,
    /// When this edge exists in the graph.
    pub interval: TimeInterval,
    /// Edge-specific properties.
    pub properties: HashMap<String, String>,
}

/// A point-in-time snapshot of the temporal graph state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    /// When this snapshot was taken.
    pub timestamp: OffsetDateTime,
    /// All nodes active at this point in time.
    pub nodes: Vec<TemporalNode>,
    /// All edges active at this point in time.
    pub edges: Vec<TemporalEdge>,
}

impl GraphSnapshot {
    /// Count of nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Count of edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

/// The current state of the temporal graph (all active nodes and edges).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphState {
    /// Active nodes keyed by ID.
    pub nodes: HashMap<String, TemporalNode>,
    /// Active edges keyed by ID.
    pub edges: HashMap<String, TemporalEdge>,
}

impl GraphState {
    /// Create an empty graph state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a node.
    pub fn add_node(&mut self, node: TemporalNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Remove a node by ID.
    pub fn remove_node(&mut self, id: &str) {
        self.nodes.remove(id);
        self.edges.retain(|_, e| e.source_id != id && e.target_id != id);
    }

    /// Add or update an edge.
    pub fn add_edge(&mut self, edge: TemporalEdge) {
        self.edges.insert(edge.id.clone(), edge);
    }

    /// Remove an edge by ID.
    pub fn remove_edge(&mut self, id: &str) {
        self.edges.remove(id);
    }

    /// Snapshot the current state at a point in time.
    #[must_use]
    pub fn snapshot(&self, timestamp: OffsetDateTime) -> GraphSnapshot {
        let nodes: Vec<_> = self
            .nodes
            .values()
            .filter(|n| n.interval.contains(timestamp))
            .cloned()
            .collect();
        let edges: Vec<_> = self
            .edges
            .values()
            .filter(|e| e.interval.contains(timestamp))
            .cloned()
            .collect();
        GraphSnapshot {
            timestamp,
            nodes,
            edges,
        }
    }

    /// Number of active nodes at a given time.
    #[must_use]
    pub fn active_node_count(&self, t: OffsetDateTime) -> usize {
        self.nodes
            .values()
            .filter(|n| n.interval.contains(t))
            .count()
    }

    /// Number of active edges at a given time.
    #[must_use]
    pub fn active_edge_count(&self, t: OffsetDateTime) -> usize {
        self.edges
            .values()
            .filter(|e| e.interval.contains(t))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn node(id: &str, start: OffsetDateTime, end: OffsetDateTime) -> TemporalNode {
        TemporalNode {
            id: id.to_string(),
            kind: "test".to_string(),
            label: id.to_string(),
            interval: TimeInterval::bounded(start, end),
            attributes: HashMap::new(),
        }
    }

    fn edge(
        id: &str,
        src: &str,
        tgt: &str,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> TemporalEdge {
        TemporalEdge {
            id: id.to_string(),
            kind: "related".to_string(),
            source_id: src.to_string(),
            target_id: tgt.to_string(),
            interval: TimeInterval::bounded(start, end),
            properties: HashMap::new(),
        }
    }

    #[test]
    fn snapshot_filters_by_time() {
        let mut state = GraphState::new();
        let t1 = datetime!(2024-01-01 0:00 UTC);
        let t2 = datetime!(2024-06-01 0:00 UTC);
        let t3 = datetime!(2024-12-01 0:00 UTC);

        state.add_node(node("n1", t1, t2));
        state.add_node(node("n2", t1, t3));

        let snap = state.snapshot(t2);

        // At t2, n1 is excluded (interval is [t1, t2), exclusive end)
        assert_eq!(snap.node_count(), 1);
        assert_eq!(snap.nodes[0].id, "n2");
    }

    #[test]
    fn remove_node_cascades_to_edges() {
        let mut state = GraphState::new();
        let t = datetime!(2024-01-01 0:00 UTC);

        state.add_node(node("n1", t, t + time::Duration::days(30)));
        state.add_node(node("n2", t, t + time::Duration::days(30)));
        state.add_edge(edge("e1", "n1", "n2", t, t + time::Duration::days(30)));

        assert_eq!(state.nodes.len(), 2);
        assert_eq!(state.edges.len(), 1);

        state.remove_node("n1");
        assert_eq!(state.nodes.len(), 1);
        assert_eq!(state.edges.len(), 0);
    }

    #[test]
    fn active_count_at_time() {
        let mut state = GraphState::new();
        let t1 = datetime!(2024-01-01 0:00 UTC);
        let t2 = datetime!(2024-06-01 0:00 UTC);

        state.add_node(node("n1", t1, t1 + time::Duration::days(60)));
        state.add_node(node("n2", t2, t2 + time::Duration::days(60)));

        assert_eq!(state.active_node_count(datetime!(2024-02-01 0:00 UTC)), 1);
        assert_eq!(state.active_node_count(datetime!(2024-07-01 0:00 UTC)), 1);
    }

    #[test]
    fn snapshot_serialization_roundtrip() {
        let mut state = GraphState::new();
        let t = datetime!(2024-01-01 0:00 UTC);
        state.add_node(node("n1", t, t + time::Duration::days(30)));

        let snap = state.snapshot(t);
        let json = serde_json::to_string(&snap).unwrap();
        let back: GraphSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.node_count(), 1);
        assert_eq!(back.nodes[0].id, "n1");
    }
}
