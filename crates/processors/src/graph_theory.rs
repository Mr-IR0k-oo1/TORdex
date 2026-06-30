use std::collections::{HashMap, HashSet};
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct GraphTheoryProcessor;

impl GraphTheoryProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GraphTheoryProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for GraphTheoryProcessor {
    fn name(&self) -> &str {
        "graph_theory"
    }

    fn description(&self) -> &str {
        "Analyzes graph structure from edge-list data, detects nodes, edges, and computes graph metrics"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-graph-theory", "application/x-mathematics"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let mut results = Vec::new();
        let text = std::str::from_utf8(data)
            .map_err(|_| ProcessorError::InvalidInput("graph data must be valid UTF-8".into()))?;

        let mut edges: Vec<(&str, &str)> = Vec::new();
        let mut nodes = HashSet::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(dash_pos) = line.find("--") {
                let a = line[..dash_pos].trim();
                let b = line[dash_pos + 2..].trim();
                if !a.is_empty() && !b.is_empty() {
                    nodes.insert(a);
                    nodes.insert(b);
                    edges.push((a, b));
                }
            } else if let Some(comma_pos) = line.find(',') {
                let a = line[..comma_pos].trim();
                let b = line[comma_pos + 1..].trim();
                if !a.is_empty() && !b.is_empty() {
                    nodes.insert(a);
                    nodes.insert(b);
                    edges.push((a, b));
                }
            }
        }

        if nodes.is_empty() {
            return Err(ProcessorError::ProcessingFailed(
                "no graph structure detected: no edge-list patterns found".into(),
            ));
        }

        let node_count = nodes.len();
        let edge_count = edges.len();
        let max_possible = node_count * (node_count - 1) / 2;
        let density = if max_possible > 0 {
            edge_count as f64 / max_possible as f64
        } else {
            0.0
        };

        let mut adjacency = HashMap::new();
        for (a, b) in &edges {
            adjacency.entry(a.to_string()).or_insert_with(Vec::new).push(b.to_string());
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_graph_summary"),
                "graph.summary",
                format!("{node_count} nodes, {edge_count} edges").into_bytes(),
                "text/plain",
            )
            .with_metadata("node_count", &node_count.to_string())
            .with_metadata("edge_count", &edge_count.to_string())
            .with_metadata("density", &format!("{:.4}", density))
            .with_metadata("source_observation", id),
        );

        let nodes_json = serde_json::json!(nodes.iter().collect::<Vec<_>>()).to_string();
        results.push(
            ProcessedObservation::new(
                format!("{id}_graph_nodes"),
                "graph.nodes",
                nodes_json.into_bytes(),
                "application/json",
            )
            .with_metadata("metric", "nodes")
            .with_metadata("source_observation", id),
        );

        let edges_json = serde_json::json!(edges).to_string();
        results.push(
            ProcessedObservation::new(
                format!("{id}_graph_edges"),
                "graph.edges",
                edges_json.into_bytes(),
                "application/json",
            )
            .with_metadata("metric", "edges")
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_graph_density"),
                "graph.metrics",
                density.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "density")
            .with_metadata("value", &format!("{:.4}", density))
            .with_metadata("source_observation", id),
        );

        let mut degree_dist = HashMap::new();
        for (_, neighbors) in &adjacency {
            let d = neighbors.len();
            *degree_dist.entry(d).or_insert(0) += 1;
        }
        let avg_degree = if node_count > 0 {
            (2.0 * edge_count as f64) / node_count as f64
        } else {
            0.0
        };
        results.push(
            ProcessedObservation::new(
                format!("{id}_graph_degree"),
                "graph.metrics",
                avg_degree.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "average_degree")
            .with_metadata("value", &format!("{:.2}", avg_degree))
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_graph_size"),
                "graph.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_simple_graph() {
        let proc = GraphTheoryProcessor::new();
        let data = b"a--b\nb--c\na--c";
        let results = proc.process("g1", data, Some("application/x-graph-theory"), HashMap::new()).unwrap();
        let summaries: Vec<_> = results.iter().filter(|o| o.kind == "graph.summary").collect();
        assert_eq!(summaries.len(), 1);
        assert!(std::str::from_utf8(&summaries[0].data).unwrap().contains("3 nodes"));
    }

    #[test]
    fn detect_comma_separated_graph() {
        let proc = GraphTheoryProcessor::new();
        let data = b"x,y\nx,z\ny,z";
        let results = proc.process("g2", data, Some("application/x-graph-theory"), HashMap::new()).unwrap();
        let edges: Vec<_> = results.iter().filter(|o| o.kind == "graph.edges").collect();
        assert!(!edges.is_empty());
    }

    #[test]
    fn no_graph_returns_error() {
        let proc = GraphTheoryProcessor::new();
        let data = b"some arbitrary text without graph structure";
        let result = proc.process("g3", data, Some("application/x-graph-theory"), HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn handles_empty_input() {
        let proc = GraphTheoryProcessor::new();
        let result = proc.process("g4", b"", Some("application/x-graph-theory"), HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn produces_nodes_observation() {
        let proc = GraphTheoryProcessor::new();
        let data = b"alpha--beta\nbeta--gamma";
        let results = proc.process("g5", data, Some("application/x-graph-theory"), HashMap::new()).unwrap();
        let nodes: Vec<_> = results.iter().filter(|o| o.kind == "graph.nodes").collect();
        assert_eq!(nodes.len(), 1);
    }
}
