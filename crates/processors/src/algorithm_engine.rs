use std::collections::{BTreeSet, HashMap, HashSet};
use tordex_algorithms;
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct AlgorithmEngine;

impl AlgorithmEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AlgorithmEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for AlgorithmEngine {
    fn name(&self) -> &str {
        "algorithm_engine"
    }

    fn description(&self) -> &str {
        "Bridges algorithm implementations into the observation pipeline: BFS, DFS, Dijkstra, SCC, PageRank, A*, Jaccard, Cosine, MinHash, SimHash, BloomFilter, MerkleTree, HyperLogLog, Trie, FMIndex, InvertedIndex"
    }

    fn content_types(&self) -> Vec<&str> {
        vec![
            "application/x-algorithm-engine",
            "application/x-mathematics",
        ]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let algorithm = metadata.get("algorithm").map(|s| s.as_str()).unwrap_or("");
        if algorithm.is_empty() {
            return Err(ProcessorError::InvalidInput(
                "algorithm metadata field required: specify which algorithm to run".into(),
            ));
        }

        let text = std::str::from_utf8(data).map_err(|_| {
            ProcessorError::InvalidInput("data must be valid UTF-8 for algorithm processing".into())
        })?;

        match algorithm {
            "bfs" | "BFS" => self.run_bfs(id, text),
            "dfs" | "DFS" => self.run_dfs(id, text),
            "dijkstra" | "Dijkstra" => self.run_dijkstra(id, text),
            "tarjan_scc" | "tarjan" | "Tarjan" => self.run_tarjan(id, text),
            "pagerank" | "PageRank" => self.run_pagerank(id, text),
            "a_star" | "AStar" | "A*" => self.run_a_star(id, text),
            "jaccard" | "Jaccard" => self.run_jaccard(id, text),
            "cosine" | "Cosine" => self.run_cosine(id, text),
            "minhash" | "MinHash" => self.run_minhash(id, text),
            "simhash" | "SimHash" => self.run_simhash(id, text),
            "bloom" | "BloomFilter" | "bloom_filter" => self.run_bloom_filter(id, text),
            "merkle" | "MerkleTree" | "merkle_tree" => self.run_merkle_tree(id, text),
            "hyperloglog" | "HLL" | "HyperLogLog" => self.run_hyperloglog(id, text),
            "trie" | "Trie" => self.run_trie(id, text),
            "fm_index" | "FMIndex" | "FM_Index" => self.run_fm_index(id, text),
            "inverted_index" | "InvertedIndex" => self.run_inverted_index(id, text),
            _ => Err(ProcessorError::UnsupportedContent(format!(
                "unknown algorithm: {algorithm}"
            ))),
        }
    }
}

impl AlgorithmEngine {
    fn edge_list_to_graph(&self, text: &str) -> tordex_algorithms::graphs::Graph {
        let mut graph = tordex_algorithms::graphs::Graph::new(true);
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split(|c: char| c == ',' || c == ' ' || c == '\t')
                .filter(|s| !s.is_empty())
                .collect();
            if parts.len() >= 2 {
                let from: usize = parts[0].parse().unwrap_or(0);
                let to: usize = parts[1].parse().unwrap_or(0);
                let weight: f64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
                graph.add_edge(from, to, weight);
            }
        }
        graph
    }

    #[allow(dead_code)]
    fn graph_to_json(&self, graph: &tordex_algorithms::graphs::Graph) -> String {
        let mut edges = Vec::new();
        for node in graph.nodes() {
            for (neighbor, weight) in graph.neighbors(node) {
                edges.push(serde_json::json!({
                    "from": node,
                    "to": neighbor,
                    "weight": weight,
                }));
            }
        }
        serde_json::json!({
            "directed": graph.is_directed(),
            "node_count": graph.node_count(),
            "edge_count": graph.edge_count(),
            "edges": edges,
        })
        .to_string()
    }

    fn run_bfs(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let graph = self.edge_list_to_graph(text);
        if graph.node_count() == 0 {
            return Err(ProcessorError::ProcessingFailed("no graph nodes found".into()));
        }
        let start = 0;
        let order = tordex_algorithms::graphs::bfs(&graph, start);
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_bfs"),
                "algorithm.bfs",
                serde_json::json!(order).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("algorithm", "BFS")
            .with_metadata("start_node", &start.to_string())
            .with_metadata("nodes_visited", &order.len().to_string())
            .with_metadata("source_observation", id),
        ])
    }

    fn run_dfs(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let graph = self.edge_list_to_graph(text);
        if graph.node_count() == 0 {
            return Err(ProcessorError::ProcessingFailed("no graph nodes found".into()));
        }
        let start = 0;
        let order = tordex_algorithms::graphs::dfs(&graph, start);
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_dfs"),
                "algorithm.dfs",
                serde_json::json!(order).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("algorithm", "DFS")
            .with_metadata("start_node", &start.to_string())
            .with_metadata("nodes_visited", &order.len().to_string())
            .with_metadata("source_observation", id),
        ])
    }

    fn run_dijkstra(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let graph = self.edge_list_to_graph(text);
        if graph.node_count() == 0 {
            return Err(ProcessorError::ProcessingFailed("no graph nodes found".into()));
        }
        let distances = tordex_algorithms::graphs::dijkstra(&graph, 0);
        let dist_json: Vec<serde_json::Value> = distances
            .iter()
            .map(|(&node, &(dist, _))| {
                serde_json::json!({
                    "node": node,
                    "distance": dist,
                })
            })
            .collect();
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_dijkstra"),
                "algorithm.dijkstra",
                serde_json::json!(dist_json).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("algorithm", "Dijkstra")
            .with_metadata("source_observation", id),
        ])
    }

    fn run_tarjan(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let graph = self.edge_list_to_graph(text);
        if graph.node_count() == 0 {
            return Err(ProcessorError::ProcessingFailed("no graph nodes found".into()));
        }
        let sccs = tordex_algorithms::graphs::tarjan_scc(&graph);
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_tarjan"),
                "algorithm.tarjan_scc",
                serde_json::json!(sccs).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("algorithm", "TarjanSCC")
            .with_metadata("component_count", &sccs.len().to_string())
            .with_metadata("source_observation", id),
        ])
    }

    fn run_pagerank(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let graph = self.edge_list_to_graph(text);
        if graph.node_count() == 0 {
            return Err(ProcessorError::ProcessingFailed("no graph nodes found".into()));
        }
        let ranks = tordex_algorithms::graphs::pagerank(&graph, 0.85, 20);
        let rank_json: Vec<serde_json::Value> = ranks
            .iter()
            .map(|(&node, &rank)| {
                serde_json::json!({
                    "node": node,
                    "rank": format!("{:.6}", rank),
                })
            })
            .collect();
        let _summary = format!("{} nodes ranked", ranks.len());
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_pagerank"),
                "algorithm.pagerank",
                serde_json::json!(rank_json).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("algorithm", "PageRank")
            .with_metadata("node_count", &ranks.len().to_string())
            .with_metadata("source_observation", id),
        ])
    }

    fn run_a_star(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let graph = self.edge_list_to_graph(text);
        if graph.node_count() == 0 {
            return Err(ProcessorError::ProcessingFailed("no graph nodes found".into()));
        }
        let heuristic = |a: usize, b: usize| (a as isize - b as isize).abs() as f64;
        let goal = graph.node_count() - 1;
        let result = tordex_algorithms::graphs::a_star(&graph, 0, goal, heuristic);
        match result {
            Some((dist, path)) => Ok(vec![
                ProcessedObservation::new(
                    format!("{id}_astar"),
                    "algorithm.a_star",
                    serde_json::json!({
                        "distance": dist,
                        "path": path,
                    }).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("algorithm", "A*")
                .with_metadata("distance", &format!("{:.4}", dist))
                .with_metadata("path_length", &path.len().to_string())
                .with_metadata("source_observation", id),
            ]),
            None => Err(ProcessorError::ProcessingFailed("no path found".into())),
        }
    }

    fn run_jaccard(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        if lines.len() < 2 {
            return Err(ProcessorError::ProcessingFailed(
                "need at least 2 lines for Jaccard comparison".into(),
            ));
        }
        let set_a: HashSet<&str> = lines[0].split_whitespace().collect();
        let set_b: HashSet<&str> = lines[1].split_whitespace().collect();
        let similarity = tordex_algorithms::similarity::jaccard_similarity(&set_a, &set_b);
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_jaccard"),
                "algorithm.jaccard",
                similarity.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("algorithm", "Jaccard")
            .with_metadata("similarity", &format!("{:.6}", similarity))
            .with_metadata("set_a_size", &set_a.len().to_string())
            .with_metadata("set_b_size", &set_b.len().to_string())
            .with_metadata("source_observation", id),
        ])
    }

    fn run_cosine(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        if lines.len() < 2 {
            return Err(ProcessorError::ProcessingFailed(
                "need at least 2 lines for Cosine comparison".into(),
            ));
        }
        let similarity = tordex_algorithms::similarity::cosine_similarity_text(lines[0], lines[1]);
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_cosine"),
                "algorithm.cosine",
                similarity.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("algorithm", "Cosine")
            .with_metadata("similarity", &format!("{:.6}", similarity))
            .with_metadata("source_observation", id),
        ])
    }

    fn run_minhash(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        if lines.len() < 2 {
            return Err(ProcessorError::ProcessingFailed(
                "need at least 2 lines for MinHash comparison".into(),
            ));
        }
        let mut mh = tordex_algorithms::similarity::MinHash::new(128);
        let set_a: BTreeSet<String> = lines[0].split_whitespace().map(|s| s.to_string()).collect();
        let set_b: BTreeSet<String> = lines[1].split_whitespace().map(|s| s.to_string()).collect();
        let i = mh.add_set(set_a);
        let j = mh.add_set(set_b);
        let estimate = mh.estimate_similarity(i, j);
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_minhash"),
                "algorithm.minhash",
                estimate.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("algorithm", "MinHash")
            .with_metadata("estimated_similarity", &format!("{:.6}", estimate))
            .with_metadata("hash_count", "128")
            .with_metadata("source_observation", id),
        ])
    }

    fn run_simhash(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        if lines.len() < 2 {
            return Err(ProcessorError::ProcessingFailed(
                "need at least 2 lines for SimHash comparison".into(),
            ));
        }
        let h1 = tordex_algorithms::similarity::SimHash::hash(lines[0]);
        let h2 = tordex_algorithms::similarity::SimHash::hash(lines[1]);
        let similarity = tordex_algorithms::similarity::SimHash::similarity(h1, h2);
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_simhash"),
                "algorithm.simhash",
                serde_json::json!({
                    "fingerprint_1": format!("0x{h1:016x}"),
                    "fingerprint_2": format!("0x{h2:016x}"),
                    "similarity": similarity,
                }).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("algorithm", "SimHash")
            .with_metadata("similarity", &format!("{:.6}", similarity))
            .with_metadata("source_observation", id),
        ])
    }

    fn run_bloom_filter(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return Err(ProcessorError::ProcessingFailed("no words to insert into bloom filter".into()));
        }
        let mut bf = tordex_algorithms::storage::BloomFilter::with_false_positive_rate(words.len(), 0.01);
        for &word in &words {
            bf.insert_str(word);
        }
        let fp_rate = bf.false_positive_rate();
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_bloom"),
                "algorithm.bloom_filter",
                format!("{} items inserted, estimated false positive rate: {:.4}", words.len(), fp_rate).into_bytes(),
                "text/plain",
            )
            .with_metadata("algorithm", "BloomFilter")
            .with_metadata("items_inserted", &words.len().to_string())
            .with_metadata("estimated_fp_rate", &format!("{:.6}", fp_rate))
            .with_metadata("source_observation", id),
        ])
    }

    fn run_merkle_tree(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let lines: Vec<Vec<u8>> = text.lines().filter(|l| !l.is_empty()).map(|l| l.as_bytes().to_vec()).collect();
        if lines.is_empty() {
            return Err(ProcessorError::ProcessingFailed("no data for Merkle tree".into()));
        }
        let data_refs: Vec<&[u8]> = lines.iter().map(|v| v.as_slice()).collect();
        let tree = tordex_algorithms::storage::MerkleTree::from_data(&data_refs);
        let root_hex = tree.root_hex().unwrap_or_default();
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_merkle"),
                "algorithm.merkle_tree",
                root_hex.clone().into_bytes(),
                "text/plain",
            )
            .with_metadata("algorithm", "MerkleTree")
            .with_metadata("root_hash", &root_hex)
            .with_metadata("leaf_count", &tree.leaf_count().to_string())
            .with_metadata("source_observation", id),
        ])
    }

    fn run_hyperloglog(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let items: Vec<&str> = text.split_whitespace().collect();
        if items.is_empty() {
            return Err(ProcessorError::ProcessingFailed("no items for HyperLogLog".into()));
        }
        let mut hll = tordex_algorithms::storage::HyperLogLog::new(12);
        let mut actual = HashSet::new();
        for &item in &items {
            hll.insert_str(item);
            actual.insert(item.to_string());
        }
        let estimate = hll.estimate();
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_hll"),
                "algorithm.hyperloglog",
                format!("estimated: {:.0}, actual: {}", estimate, actual.len()).into_bytes(),
                "text/plain",
            )
            .with_metadata("algorithm", "HyperLogLog")
            .with_metadata("estimated", &format!("{:.0}", estimate))
            .with_metadata("actual", &actual.len().to_string())
            .with_metadata("precision", "12")
            .with_metadata("source_observation", id),
        ])
    }

    fn run_trie(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return Err(ProcessorError::ProcessingFailed("no words for Trie".into()));
        }
        let mut trie = tordex_algorithms::searching::Trie::new();
        for word in &words {
            trie.insert(word, None);
        }
        let prefixes: Vec<String> = words
            .iter()
            .filter(|w| w.len() >= 2)
            .map(|w| {
                let prefix = &w[..w.len() - 1];
                let count = trie.keys_with_prefix(prefix).len();
                format!("{prefix}: {count}")
            })
            .collect();
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_trie"),
                "algorithm.trie",
                serde_json::json!(prefixes).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("algorithm", "Trie")
            .with_metadata("words_inserted", &words.len().to_string())
            .with_metadata("source_observation", id),
        ])
    }

    fn run_fm_index(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let parts: Vec<&str> = text.splitn(2, '\n').collect();
        if parts.len() < 2 || parts[1].trim().is_empty() {
            return Err(ProcessorError::ProcessingFailed(
                "need text on line 1 and pattern on line 2 for FM Index".into(),
            ));
        }
        let corpus = parts[0].trim();
        let pattern = parts[1].trim();
        let fm = tordex_algorithms::searching::FMIndex::new(corpus);
        let count = fm.count_occurrences(pattern);
        let positions = fm.search(pattern);
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_fm_index"),
                "algorithm.fm_index",
                serde_json::json!({
                    "pattern": pattern,
                    "occurrences": count,
                    "positions": positions,
                }).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("algorithm", "FMIndex")
            .with_metadata("occurrences", &count.to_string())
            .with_metadata("source_observation", id),
        ])
    }

    fn run_inverted_index(&self, id: &str, text: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        if lines.len() < 2 {
            return Err(ProcessorError::ProcessingFailed(
                "need at least 2 lines for Inverted Index (doc text + query)".into(),
            ));
        }
        let mut idx = tordex_algorithms::searching::InvertedIndex::new();
        for (i, line) in lines[..lines.len() - 1].iter().enumerate() {
            idx.add_document(&format!("doc{i}"), line);
        }
        let query = lines[lines.len() - 1];
        let results = idx.search(query);
        Ok(vec![
            ProcessedObservation::new(
                format!("{id}_inverted_index"),
                "algorithm.inverted_index",
                serde_json::json!({
                    "query": query,
                    "results": results.iter().map(|(doc, positions)| {
                        serde_json::json!({
                            "document": doc,
                            "positions": positions,
                            "frequency": positions.len(),
                        })
                    }).collect::<Vec<_>>(),
                }).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("algorithm", "InvertedIndex")
            .with_metadata("documents_searched", &lines.len().saturating_sub(1).to_string())
            .with_metadata("query", query)
            .with_metadata("source_observation", id),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_algorithm_returns_error() {
        let engine = AlgorithmEngine::new();
        let result = engine.process("e1", b"data", Some("application/x-algorithm-engine"), HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn bfs_runs_on_graph_data() {
        let engine = AlgorithmEngine::new();
        let data = b"0 1\n1 2\n2 3\n3 0";
        let mut meta = HashMap::new();
        meta.insert("algorithm".to_string(), "BFS".to_string());
        let results = engine.process("e2", data, Some("application/x-algorithm-engine"), meta).unwrap();
        let bfs_obs: Vec<_> = results.iter().filter(|o| o.kind == "algorithm.bfs").collect();
        assert_eq!(bfs_obs.len(), 1);
    }

    #[test]
    fn dijkstra_runs_on_weighted_graph() {
        let engine = AlgorithmEngine::new();
        let data = b"0 1 4\n0 2 2\n1 3 1\n2 3 5";
        let mut meta = HashMap::new();
        meta.insert("algorithm".to_string(), "Dijkstra".to_string());
        let results = engine.process("e3", data, Some("application/x-algorithm-engine"), meta).unwrap();
        let dijkstra_obs: Vec<_> = results.iter().filter(|o| o.kind == "algorithm.dijkstra").collect();
        assert_eq!(dijkstra_obs.len(), 1);
    }

    #[test]
    fn jaccard_runs_on_two_sets() {
        let engine = AlgorithmEngine::new();
        let data = b"apple banana cherry\ndanana apple fig";
        let mut meta = HashMap::new();
        meta.insert("algorithm".to_string(), "Jaccard".to_string());
        let results = engine.process("e4", data, Some("application/x-algorithm-engine"), meta).unwrap();
        let jaccard_obs: Vec<_> = results.iter().filter(|o| o.kind == "algorithm.jaccard").collect();
        assert_eq!(jaccard_obs.len(), 1);
    }

    #[test]
    fn cosine_runs_on_two_texts() {
        let engine = AlgorithmEngine::new();
        let data = b"hello world\nhello world";
        let mut meta = HashMap::new();
        meta.insert("algorithm".to_string(), "Cosine".to_string());
        let results = engine.process("e5", data, Some("application/x-algorithm-engine"), meta).unwrap();
        let cosine_obs: Vec<_> = results.iter().filter(|o| o.kind == "algorithm.cosine").collect();
        assert_eq!(cosine_obs.len(), 1);
    }

    #[test]
    fn bloom_filter_runs() {
        let engine = AlgorithmEngine::new();
        let data = b"hello world foo bar baz";
        let mut meta = HashMap::new();
        meta.insert("algorithm".to_string(), "BloomFilter".to_string());
        let results = engine.process("e6", data, Some("application/x-algorithm-engine"), meta).unwrap();
        let bloom_obs: Vec<_> = results.iter().filter(|o| o.kind == "algorithm.bloom_filter").collect();
        assert_eq!(bloom_obs.len(), 1);
    }

    #[test]
    fn merkle_tree_runs() {
        let engine = AlgorithmEngine::new();
        let data = b"line1\nline2\nline3";
        let mut meta = HashMap::new();
        meta.insert("algorithm".to_string(), "MerkleTree".to_string());
        let results = engine.process("e7", data, Some("application/x-algorithm-engine"), meta).unwrap();
        let merkle_obs: Vec<_> = results.iter().filter(|o| o.kind == "algorithm.merkle_tree").collect();
        assert_eq!(merkle_obs.len(), 1);
    }

    #[test]
    fn unsupported_algorithm_returns_error() {
        let engine = AlgorithmEngine::new();
        let mut meta = HashMap::new();
        meta.insert("algorithm".to_string(), "Nonsense".to_string());
        let result = engine.process("e8", b"data", Some("application/x-algorithm-engine"), meta);
        assert!(result.is_err());
    }

    #[test]
    fn fm_index_runs() {
        let engine = AlgorithmEngine::new();
        let data = b"banana\nana";
        let mut meta = HashMap::new();
        meta.insert("algorithm".to_string(), "FMIndex".to_string());
        let results = engine.process("e9", data, Some("application/x-algorithm-engine"), meta).unwrap();
        let fm_obs: Vec<_> = results.iter().filter(|o| o.kind == "algorithm.fm_index").collect();
        assert_eq!(fm_obs.len(), 1);
    }

    #[test]
    fn pagerank_runs() {
        let engine = AlgorithmEngine::new();
        let data = b"0 1\n0 2\n1 2\n2 0";
        let mut meta = HashMap::new();
        meta.insert("algorithm".to_string(), "PageRank".to_string());
        let results = engine.process("e10", data, Some("application/x-algorithm-engine"), meta).unwrap();
        let pr_obs: Vec<_> = results.iter().filter(|o| o.kind == "algorithm.pagerank").collect();
        assert_eq!(pr_obs.len(), 1);
    }
}
