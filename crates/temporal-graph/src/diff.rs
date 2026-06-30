use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::graph::{GraphSnapshot, TemporalEdge, TemporalNode};

/// A change to a node between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeChange {
    pub id: String,
    pub attribute_changes: Vec<AttributeDelta>,
    pub interval_changed: bool,
}

/// A change to an edge between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeChange {
    pub id: String,
    pub property_changes: Vec<AttributeDelta>,
    pub interval_changed: bool,
}

/// A single attribute delta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDelta {
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

/// The difference between two graph snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDiff {
    /// Timestamp of the older snapshot.
    pub from: OffsetDateTime,
    /// Timestamp of the newer snapshot.
    pub to: OffsetDateTime,
    /// Nodes that appeared in the newer snapshot.
    pub added_nodes: Vec<TemporalNode>,
    /// Node IDs that disappeared from the newer snapshot.
    pub removed_nodes: Vec<String>,
    /// Edges that appeared in the newer snapshot.
    pub added_edges: Vec<TemporalEdge>,
    /// Edge IDs that disappeared from the newer snapshot.
    pub removed_edges: Vec<String>,
    /// Nodes that changed attributes or interval.
    pub modified_nodes: Vec<NodeChange>,
    /// Edges that changed properties or interval.
    pub modified_edges: Vec<EdgeChange>,
}

impl GraphDiff {
    /// Compute the diff between two snapshots.
    #[must_use]
    pub fn between(before: &GraphSnapshot, after: &GraphSnapshot) -> Self {
        let before_nodes: std::collections::HashMap<&str, &TemporalNode> =
            before.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let after_nodes: std::collections::HashMap<&str, &TemporalNode> =
            after.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        let before_edges: std::collections::HashMap<&str, &TemporalEdge> =
            before.edges.iter().map(|e| (e.id.as_str(), e)).collect();
        let after_edges: std::collections::HashMap<&str, &TemporalEdge> =
            after.edges.iter().map(|e| (e.id.as_str(), e)).collect();

        let mut added_nodes = Vec::new();
        let mut removed_nodes = Vec::new();
        let mut modified_nodes = Vec::new();

        for (id, node) in &after_nodes {
            match before_nodes.get(id) {
                None => added_nodes.push((*node).clone()),
                Some(before) => {
                    let changes = diff_node_attributes(before, node);
                    let interval_changed = before.interval != node.interval;
                    if !changes.is_empty() || interval_changed {
                        modified_nodes.push(NodeChange {
                            id: id.to_string(),
                            attribute_changes: changes,
                            interval_changed,
                        });
                    }
                }
            }
        }
        for (id, _) in &before_nodes {
            if !after_nodes.contains_key(id) {
                removed_nodes.push((*id).to_string());
            }
        }

        let mut added_edges = Vec::new();
        let mut removed_edges = Vec::new();
        let mut modified_edges = Vec::new();

        for (id, edge) in &after_edges {
            match before_edges.get(id) {
                None => added_edges.push((*edge).clone()),
                Some(before) => {
                    let changes = diff_edge_properties(before, edge);
                    let interval_changed = before.interval != edge.interval;
                    if !changes.is_empty() || interval_changed {
                        modified_edges.push(EdgeChange {
                            id: id.to_string(),
                            property_changes: changes,
                            interval_changed,
                        });
                    }
                }
            }
        }
        for (id, _) in &before_edges {
            if !after_edges.contains_key(id) {
                removed_edges.push((*id).to_string());
            }
        }

        Self {
            from: before.timestamp,
            to: after.timestamp,
            added_nodes,
            removed_nodes,
            added_edges,
            removed_edges,
            modified_nodes,
            modified_edges,
        }
    }

    /// Total number of changes.
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.added_nodes.len()
            + self.removed_nodes.len()
            + self.added_edges.len()
            + self.removed_edges.len()
            + self.modified_nodes.len()
            + self.modified_edges.len()
    }

    /// Whether any changes exist.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        self.change_count() > 0
    }

    /// Rate of change (changes per second).
    #[must_use]
    pub fn change_rate(&self) -> f64 {
        let duration_secs = (self.to - self.from).whole_seconds() as f64;
        if duration_secs <= 0.0 {
            return 0.0;
        }
        self.change_count() as f64 / duration_secs
    }
}

fn diff_node_attributes(
    before: &TemporalNode,
    after: &TemporalNode,
) -> Vec<AttributeDelta> {
    let mut changes = Vec::new();
    let mut all_keys: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for k in before.attributes.keys() {
        all_keys.insert(k.as_str());
    }
    for k in after.attributes.keys() {
        all_keys.insert(k.as_str());
    }
    for key in all_keys {
        let old = before.attributes.get(key).cloned();
        let new = after.attributes.get(key).cloned();
        if old != new {
            changes.push(AttributeDelta {
                key: key.to_string(),
                old_value: old,
                new_value: new,
            });
        }
    }
    changes
}

fn diff_edge_properties(
    before: &TemporalEdge,
    after: &TemporalEdge,
) -> Vec<AttributeDelta> {
    let mut changes = Vec::new();
    let mut all_keys: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for k in before.properties.keys() {
        all_keys.insert(k.as_str());
    }
    for k in after.properties.keys() {
        all_keys.insert(k.as_str());
    }
    for key in all_keys {
        let old = before.properties.get(key).cloned();
        let new = after.properties.get(key).cloned();
        if old != new {
            changes.push(AttributeDelta {
                key: key.to_string(),
                old_value: old,
                new_value: new,
            });
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphSnapshot, TemporalNode, TemporalEdge};
    use crate::interval::TimeInterval;
    use std::collections::HashMap;
    use time::macros::datetime;

    fn make_node(id: &str, kind: &str, attrs: Vec<(&str, &str)>) -> TemporalNode {
        let mut attributes = HashMap::new();
        for (k, v) in attrs {
            attributes.insert(k.to_string(), v.to_string());
        }
        TemporalNode {
            id: id.to_string(),
            kind: kind.to_string(),
            label: id.to_string(),
            interval: TimeInterval::starting_at(datetime!(2024-01-01 0:00 UTC)),
            attributes,
        }
    }

    fn make_edge(id: &str, src: &str, tgt: &str) -> TemporalEdge {
        TemporalEdge {
            id: id.to_string(),
            kind: "related".to_string(),
            source_id: src.to_string(),
            target_id: tgt.to_string(),
            interval: TimeInterval::starting_at(datetime!(2024-01-01 0:00 UTC)),
            properties: HashMap::new(),
        }
    }

    #[test]
    fn diff_identifies_added_nodes() {
        let before = GraphSnapshot {
            timestamp: datetime!(2024-01-01 0:00 UTC),
            nodes: vec![],
            edges: vec![],
        };
        let after = GraphSnapshot {
            timestamp: datetime!(2024-01-02 0:00 UTC),
            nodes: vec![make_node("n1", "entity", vec![])],
            edges: vec![],
        };
        let diff = GraphDiff::between(&before, &after);
        assert_eq!(diff.added_nodes.len(), 1);
        assert_eq!(diff.removed_nodes.len(), 0);
        assert!(diff.has_changes());
    }

    #[test]
    fn diff_identifies_removed_nodes() {
        let before = GraphSnapshot {
            timestamp: datetime!(2024-01-01 0:00 UTC),
            nodes: vec![make_node("n1", "entity", vec![])],
            edges: vec![],
        };
        let after = GraphSnapshot {
            timestamp: datetime!(2024-01-02 0:00 UTC),
            nodes: vec![],
            edges: vec![],
        };
        let diff = GraphDiff::between(&before, &after);
        assert_eq!(diff.removed_nodes.len(), 1);
        assert_eq!(diff.added_nodes.len(), 0);
    }

    #[test]
    fn diff_identifies_modified_attributes() {
        let before = GraphSnapshot {
            timestamp: datetime!(2024-01-01 0:00 UTC),
            nodes: vec![make_node("n1", "entity", vec![("color", "red")])],
            edges: vec![],
        };
        let after = GraphSnapshot {
            timestamp: datetime!(2024-01-02 0:00 UTC),
            nodes: vec![make_node("n1", "entity", vec![("color", "blue")])],
            edges: vec![],
        };
        let diff = GraphDiff::between(&before, &after);
        assert_eq!(diff.modified_nodes.len(), 1);
        assert_eq!(diff.modified_nodes[0].attribute_changes.len(), 1);
        assert_eq!(diff.modified_nodes[0].attribute_changes[0].old_value.as_deref(), Some("red"));
        assert_eq!(diff.modified_nodes[0].attribute_changes[0].new_value.as_deref(), Some("blue"));
    }

    #[test]
    fn diff_edge_changes() {
        let before = GraphSnapshot {
            timestamp: datetime!(2024-01-01 0:00 UTC),
            nodes: vec![],
            edges: vec![make_edge("e1", "n1", "n2")],
        };
        let after = GraphSnapshot {
            timestamp: datetime!(2024-01-02 0:00 UTC),
            nodes: vec![],
            edges: vec![],
        };
        let diff = GraphDiff::between(&before, &after);
        assert_eq!(diff.removed_edges.len(), 1);
    }

    #[test]
    fn no_changes_returns_empty_diff() {
        let snap = GraphSnapshot {
            timestamp: datetime!(2024-01-01 0:00 UTC),
            nodes: vec![make_node("n1", "entity", vec![])],
            edges: vec![],
        };
        let diff = GraphDiff::between(&snap, &snap.clone());
        assert!(!diff.has_changes());
        assert_eq!(diff.change_count(), 0);
    }

    #[test]
    fn change_rate_computes_correctly() {
        let before = GraphSnapshot {
            timestamp: datetime!(2024-01-01 0:00 UTC),
            nodes: vec![],
            edges: vec![],
        };
        let after = GraphSnapshot {
            timestamp: datetime!(2024-01-02 0:00 UTC),
            nodes: vec![
                make_node("n1", "entity", vec![]),
                make_node("n2", "entity", vec![]),
            ],
            edges: vec![],
        };
        let diff = GraphDiff::between(&before, &after);
        // 86400 seconds in a day, 2 changes
        assert!((diff.change_rate() - 2.0 / 86400.0).abs() < 1e-10);
    }
}
