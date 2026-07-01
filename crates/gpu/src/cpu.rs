use std::collections::{HashMap, HashSet, VecDeque};

use crate::{GraphData, GpuAccelerator, SimilarityMetric, VectorIndex};

/// CPU-based implementation of GPU-accelerated operations.
///
/// Uses the CPU for all computations. The same trait interface
/// means a `WgpuAccelerator` (feature `wgpu-backend`) can be
/// swapped in transparently when GPU hardware is available.
#[derive(Debug, Clone)]
pub struct CpuAccelerator;

impl CpuAccelerator {
    pub fn new() -> Self {
        Self
    }
}

impl GpuAccelerator for CpuAccelerator {
    fn similarity(&self, a: &[f32], b: &[f32], metric: SimilarityMetric) -> f32 {
        match metric {
            SimilarityMetric::Cosine => cosine_similarity(a, b),
            SimilarityMetric::Euclidean => euclidean_distance(a, b),
            SimilarityMetric::DotProduct => dot_product(a, b),
        }
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        embed_text(text)
    }

    fn graph_traverse(&self, graph: &GraphData, start: &[u64], depth: u32) -> Vec<u64> {
        traverse_bfs(graph, start, depth)
    }

    fn rank(&self, scores: &[(String, f32)], top_k: usize) -> Vec<(String, f32)> {
        rank_items(scores, top_k)
    }

    fn vector_search(&self, query: &[f32], index: &VectorIndex, k: usize) -> Vec<(usize, f32)> {
        search_vectors(query, index, k)
    }
}

// ─── Similarity ──────────────────────────────────────────────────────────

fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn magnitude(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mag_a = magnitude(a);
    let mag_b = magnitude(b);
    if mag_a < 1e-10 || mag_b < 1e-10 {
        return 0.0;
    }
    dot_product(a, b) / (mag_a * mag_b)
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f32>()
        .sqrt()
}

// ─── Embedding ───────────────────────────────────────────────────────────

/// Deterministic hash-based embedding (CPU fallback).
///
/// Produces a 64-dimensional vector by hashing n-grams of the input text.
/// This is NOT a neural embedding — it's a fast locality-sensitive hash
/// for CPU fallback. GPU backend would use a real embedding model.
fn embed_text(text: &str) -> Vec<f32> {
    let dim = 64;
    let mut vec = vec![0.0f32; dim];
    let lower = text.to_lowercase();

    // Sum character-level n-gram hashes
    for ngram_size in 1..=4 {
        for window in lower.as_bytes().windows(ngram_size) {
            let hash = fnv_hash(window);
            let idx = (hash as usize) % dim;
            vec[idx] += 1.0;
        }
    }

    // Normalize
    let mag = magnitude(&vec);
    if mag > 1e-10 {
        for v in &mut vec {
            *v /= mag;
        }
    }

    vec
}

fn fnv_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ─── Graph Traversal ─────────────────────────────────────────────────────

fn traverse_bfs(graph: &GraphData, start: &[u64], depth: u32) -> Vec<u64> {
    // Build adjacency list
    let mut adj: HashMap<u64, Vec<u64>> = HashMap::new();
    for &node in &graph.nodes {
        adj.entry(node).or_default();
    }
    for &(src, tgt) in &graph.edges {
        adj.entry(src).or_default().push(tgt);
        adj.entry(tgt).or_default().push(src); // undirected
    }

    let mut visited: HashSet<u64> = HashSet::new();
    let mut result: Vec<u64> = Vec::new();
    let mut queue: VecDeque<(u64, u32)> = VecDeque::new();

    for &s in start {
        visited.insert(s);
        queue.push_back((s, 0));
    }

    while let Some((node, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        if let Some(neighbors) = adj.get(&node) {
            for &next in neighbors {
                if visited.insert(next) {
                    result.push(next);
                    queue.push_back((next, d + 1));
                }
            }
        }
    }

    result
}

// ─── Ranking ─────────────────────────────────────────────────────────────

fn rank_items(scores: &[(String, f32)], top_k: usize) -> Vec<(String, f32)> {
    let mut sorted: Vec<(String, f32)> = scores.to_vec();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(top_k);
    sorted
}

// ─── Vector Search ───────────────────────────────────────────────────────

fn search_vectors(query: &[f32], index: &VectorIndex, k: usize) -> Vec<(usize, f32)> {
    let mut scores: Vec<(usize, f32)> = index
        .vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i, cosine_similarity(query, v)))
        .collect();

    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores.truncate(k);
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product_identity() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(dot_product(&a, &b), 14.0);
    }

    #[test]
    fn test_cosine_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_zero() {
        let a = vec![5.0, 5.0];
        let b = vec![5.0, 5.0];
        assert!((euclidean_distance(&a, &b) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_embed_similar_texts() {
        let a = embed_text("hello world");
        let b = embed_text("hello world!");
        let sim = cosine_similarity(&a, &b);
        assert!(sim > 0.8, "similar texts should have high cosine, got {}", sim);
    }

    #[test]
    fn test_embed_different_texts() {
        let a = embed_text("network security threat intelligence");
        let b = embed_text("baking banana bread recipe");
        let sim = cosine_similarity(&a, &b);
        assert!(sim < 0.8, "unrelated texts should have lower cosine, got {}", sim);
    }

    #[test]
    fn test_bfs_chain() {
        let graph = GraphData::new(
            vec![0, 1, 2, 3, 4],
            vec![(0, 1), (1, 2), (2, 3), (3, 4)],
        );
        let reached = traverse_bfs(&graph, &[0], 3);
        assert!(reached.contains(&1));
        assert!(reached.contains(&2));
        assert!(reached.contains(&3));
        assert!(!reached.contains(&4));
    }

    #[test]
    fn test_bfs_star_graph() {
        let graph = GraphData::new(
            vec![0, 1, 2, 3],
            vec![(0, 1), (0, 2), (0, 3)],
        );
        let reached = traverse_bfs(&graph, &[0], 1);
        assert_eq!(reached.len(), 3);
        assert!(reached.contains(&1));
        assert!(reached.contains(&2));
        assert!(reached.contains(&3));
    }

    #[test]
    fn test_bfs_disconnected() {
        let graph = GraphData::new(
            vec![0, 1, 2],
            vec![(0, 1)],
        );
        let reached = traverse_bfs(&graph, &[2], 5);
        assert!(reached.is_empty(), "node 2 is isolated");
    }

    #[test]
    fn test_rank_top_k() {
        let items = vec![
            ("a".into(), 0.1),
            ("b".into(), 0.9),
            ("c".into(), 0.5),
            ("d".into(), 0.8),
        ];
        let ranked = rank_items(&items, 2);
        assert_eq!(ranked[0].0, "b");
        assert_eq!(ranked[1].0, "d");
    }

    #[test]
    fn test_vector_search_exact_match() {
        let index = VectorIndex::new(
            vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            vec!["x".into(), "y".into()],
        );
        let results = search_vectors(&[1.0, 0.0], &index, 1);
        assert_eq!(results[0].0, 0);
        assert!((results[0].1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_vector_search_k_larger_than_index() {
        let index = VectorIndex::new(
            vec![vec![1.0, 0.0]],
            vec!["only".into()],
        );
        let results = search_vectors(&[1.0, 0.0], &index, 10);
        assert_eq!(results.len(), 1);
    }
}
