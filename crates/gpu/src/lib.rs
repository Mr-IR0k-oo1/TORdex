pub mod cpu;

use std::fmt::Debug;

/// Metric for similarity computation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimilarityMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

/// A graph structure for traversal operations.
#[derive(Debug, Clone)]
pub struct GraphData {
    /// All nodes in the graph.
    pub nodes: Vec<u64>,
    /// Directed edges as (source, target) pairs.
    pub edges: Vec<(u64, u64)>,
}

impl GraphData {
    pub fn new(nodes: Vec<u64>, edges: Vec<(u64, u64)>) -> Self {
        Self { nodes, edges }
    }
}

/// A vector index for nearest-neighbor search.
#[derive(Debug, Clone)]
pub struct VectorIndex {
    /// The indexed vectors (all must be same dimension).
    pub vectors: Vec<Vec<f32>>,
    /// Optional label per vector.
    pub labels: Vec<String>,
}

impl VectorIndex {
    pub fn new(vectors: Vec<Vec<f32>>, labels: Vec<String>) -> Self {
        Self { vectors, labels }
    }
}

/// GPU-accelerated knowledge operations.
///
/// This trait defines the 5 heavy operations that benefit from GPU acceleration:
/// similarity, embedding, graph traversal, ranking, and vector search.
/// The default `CpuAccelerator` provides CPU-based fallbacks.
///
/// Future backend: `WgpuAccelerator` (feature `wgpu-backend`).
pub trait GpuAccelerator: Debug + Send + Sync {
    /// Compute similarity between two vectors using the given metric.
    fn similarity(&self, a: &[f32], b: &[f32], metric: SimilarityMetric) -> f32;

    /// Generate a fixed-size embedding vector for a text string.
    fn embed(&self, text: &str) -> Vec<f32>;

    /// Perform a graph traversal from starting nodes to given depth.
    /// Returns all reachable node IDs within that depth.
    fn graph_traverse(&self, graph: &GraphData, start: &[u64], depth: u32) -> Vec<u64>;

    /// Rank items by score, returning the top-k as (label, score) sorted descending.
    fn rank(&self, scores: &[(String, f32)], top_k: usize) -> Vec<(String, f32)>;

    /// Find the top-k nearest neighbors of a query vector in the index.
    /// Returns (index_position, similarity_score) sorted by score descending.
    fn vector_search(&self, query: &[f32], index: &VectorIndex, k: usize) -> Vec<(usize, f32)>;
}

/// Unified engine that dispatches to CPU or GPU backend.
#[derive(Debug)]
pub enum GpuEngine {
    Cpu(cpu::CpuAccelerator),
}

impl GpuEngine {
    pub fn new_cpu() -> Self {
        Self::Cpu(cpu::CpuAccelerator::new())
    }
}

impl GpuAccelerator for GpuEngine {
    fn similarity(&self, a: &[f32], b: &[f32], metric: SimilarityMetric) -> f32 {
        match self {
            Self::Cpu(cpu) => cpu.similarity(a, b, metric),
        }
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        match self {
            Self::Cpu(cpu) => cpu.embed(text),
        }
    }

    fn graph_traverse(&self, graph: &GraphData, start: &[u64], depth: u32) -> Vec<u64> {
        match self {
            Self::Cpu(cpu) => cpu.graph_traverse(graph, start, depth),
        }
    }

    fn rank(&self, scores: &[(String, f32)], top_k: usize) -> Vec<(String, f32)> {
        match self {
            Self::Cpu(cpu) => cpu.rank(scores, top_k),
        }
    }

    fn vector_search(&self, query: &[f32], index: &VectorIndex, k: usize) -> Vec<(usize, f32)> {
        match self {
            Self::Cpu(cpu) => cpu.vector_search(query, index, k),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_similarity_cosine() {
        let accel = GpuEngine::new_cpu();
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = accel.similarity(&a, &b, SimilarityMetric::Cosine);
        assert!((sim - 1.0).abs() < 1e-6, "identical vectors should have cosine=1, got {}", sim);
    }

    #[test]
    fn test_similarity_orthogonal() {
        let accel = GpuEngine::new_cpu();
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = accel.similarity(&a, &b, SimilarityMetric::Cosine);
        assert!((sim - 0.0).abs() < 1e-6, "orthogonal vectors should have cosine=0, got {}", sim);
    }

    #[test]
    fn test_similarity_euclidean() {
        let accel = GpuEngine::new_cpu();
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        let sim = accel.similarity(&a, &b, SimilarityMetric::Euclidean);
        assert!((sim - 5.0).abs() < 1e-6, "euclidean distance should be 5, got {}", sim);
    }

    #[test]
    fn test_embed_produces_fixed_size() {
        let accel = GpuEngine::new_cpu();
        let e = accel.embed("hello world");
        assert_eq!(e.len(), 64, "embedding should be 64-dimensional");
    }

    #[test]
    fn test_embed_deterministic() {
        let accel = GpuEngine::new_cpu();
        let a = accel.embed("test");
        let b = accel.embed("test");
        assert_eq!(a, b, "embeddings should be deterministic");
    }

    #[test]
    fn test_graph_traverse_simple() {
        let accel = GpuEngine::new_cpu();
        let graph = GraphData::new(
            vec![0, 1, 2, 3],
            vec![(0, 1), (1, 2), (2, 3)],
        );
        let reached = accel.graph_traverse(&graph, &[0], 2);
        assert!(reached.contains(&1));
        assert!(reached.contains(&2));
        assert!(!reached.contains(&3));
    }

    #[test]
    fn test_rank_orders_by_score() {
        let accel = GpuEngine::new_cpu();
        let items = vec![
            ("a".to_string(), 0.1),
            ("b".to_string(), 0.9),
            ("c".to_string(), 0.5),
        ];
        let ranked = accel.rank(&items, 2);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].0, "b");
        assert_eq!(ranked[1].0, "c");
    }

    #[test]
    fn test_vector_search() {
        let accel = GpuEngine::new_cpu();
        let index = VectorIndex::new(
            vec![
                vec![1.0, 0.0],
                vec![0.0, 1.0],
                vec![0.9, 0.1],
            ],
            vec!["x".into(), "y".into(), "near_x".into()],
        );
        let query = vec![1.0, 0.0];
        let results = accel.vector_search(&query, &index, 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0, "most similar should be index 0");
        assert_eq!(results[1].0, 2, "second should be index 2");
    }
}
