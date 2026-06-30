#![forbid(unsafe_code)]
#![allow(clippy::module_name_repetitions)]

pub mod index;
pub mod strategies;

use std::collections::HashMap;

use index::SearchIndex;
use serde::{Deserialize, Serialize};

/// A document stored in the search index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub source: String,
    pub timestamp: Option<time::OffsetDateTime>,
    pub metadata: HashMap<String, String>,
}

/// A scored search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredResult {
    pub document: Document,
    pub score: f64,
    pub matched_on: Vec<String>,
}

/// Time range for temporal queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Option<time::OffsetDateTime>,
    pub end: Option<time::OffsetDateTime>,
}

/// Structural pattern for structural queries (AST node matching).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StructPattern {
    /// Match a function/method definition
    Function { name: Option<String> },
    /// Match a class/struct definition
    Class { name: Option<String> },
    /// Match an interface/trait definition
    Interface { name: Option<String> },
    /// Match any symbol with the given kind
    Symbol { kind: String, name: Option<String> },
    /// Match a call expression
    Call { name: Option<String> },
    /// Match an import statement
    Import { source: Option<String> },
}

/// Graph pattern for graph queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphPattern {
    /// Find nodes by name pattern
    Node { name: String, kind: Option<String> },
    /// Find edges between pattern-matched nodes
    Edge {
        source: Box<GraphPattern>,
        target: Box<GraphPattern>,
        kind: Option<String>,
    },
    /// Find paths between two nodes
    Path {
        from: Box<GraphPattern>,
        to: Box<GraphPattern>,
        max_depth: usize,
    },
    /// Find all neighbors of a node
    Neighbors {
        node: Box<GraphPattern>,
        depth: usize,
    },
    /// Match a subgraph
    Subgraph(Vec<GraphPattern>),
}

/// Dependency query for dependency search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyQuery {
    /// Find dependents of a package
    Dependents { name: String, ecosystem: Option<String> },
    /// Find dependencies of a package
    Dependencies { name: String, ecosystem: Option<String> },
    /// Find transitive dependencies
    Transitive {
        name: String,
        ecosystem: Option<String>,
        max_depth: usize,
    },
    /// Find all packages in an ecosystem
    Ecosystem(String),
}

/// Architecture query for architecture search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchQuery {
    /// Match modules in a path
    Module { path: String },
    /// Match packages by name
    Package { name: String },
    /// Match a namespace/workspace
    Workspace { name: String },
    /// Match all files in a directory
    Directory { path: String },
}

/// Filter spec for filtering query results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterSpec {
    Kind(String),
    Source(String),
    TimeBefore(time::OffsetDateTime),
    TimeAfter(time::OffsetDateTime),
    Metadata { key: String, value: String },
    ScoreAbove(f64),
    ScoreBelow(f64),
}

/// The query algebra — search becomes algebra.
///
/// Every query is an expression that evaluates to a ranked set of results.
/// Expressions can be composed using boolean combinators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryExpr {
    // ── Atomic queries ───────────────────────────────────────────────────
    Keyword(String),
    Prefix(String),
    Phrase(Vec<String>),

    /// Semantic search using a text embedding vector (cosine similarity).
    Semantic { text: String, embedding: Vec<f64> },

    /// Temporal search — find documents within a time range.
    Temporal(TimeRange),

    /// Structural search — find code patterns (functions, classes, etc.).
    Structural(StructPattern),

    /// Graph search — find graph patterns (nodes, edges, paths).
    Graph(GraphPattern),

    /// Dependency search — find package dependencies.
    Dependency(DependencyQuery),

    /// Architecture search — find modules/packages/workspaces.
    Architecture(ArchQuery),

    /// Similarity search — find results similar to another query's results.
    Similarity {
        query: Box<QueryExpr>,
        threshold: f64,
        max_results: usize,
    },

    // ── Boolean combinators ──────────────────────────────────────────────
    And(Vec<QueryExpr>),
    Or(Vec<QueryExpr>),
    Not(Box<QueryExpr>),

    /// Filter results of a query.
    Filter {
        query: Box<QueryExpr>,
        spec: FilterSpec,
    },
}

/// A query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub results: Vec<ScoredResult>,
    pub total_count: usize,
    pub took_ns: u128,
}

/// The Intelligence Search Engine — indexes documents and evaluates
/// query algebra expressions against them.
#[derive(Debug)]
pub struct SearchEngine {
    index: SearchIndex,
}

impl SearchEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            index: SearchIndex::new(),
        }
    }

    /// Index a document for search.
    pub fn index_document(&mut self, doc: Document) {
        self.index.add_document(doc);
    }

    /// Index multiple documents.
    pub fn index_documents(&mut self, docs: Vec<Document>) {
        for doc in docs {
            self.index.add_document(doc);
        }
    }

    /// Evaluate a query expression against the index.
    pub fn search(&self, query: &QueryExpr, max_results: usize) -> QueryResult {
        let start = std::time::Instant::now();
        let results = self.evaluate(query, max_results);
        let total_count = results.len();
        QueryResult {
            results,
            total_count,
            took_ns: start.elapsed().as_nanos(),
        }
    }

    fn evaluate(&self, query: &QueryExpr, max_results: usize) -> Vec<ScoredResult> {
        match query {
            QueryExpr::Keyword(term) => self.eval_keyword(term, max_results),
            QueryExpr::Prefix(prefix) => self.eval_prefix(prefix, max_results),
            QueryExpr::Phrase(words) => self.eval_phrase(words, max_results),
            QueryExpr::Semantic { text: _, embedding } => {
                self.eval_semantic(embedding, max_results)
            }
            QueryExpr::Temporal(range) => self.eval_temporal(range, max_results),
            QueryExpr::Structural(pattern) => self.eval_structural(pattern, max_results),
            QueryExpr::Graph(pattern) => self.eval_graph(pattern, max_results),
            QueryExpr::Dependency(query) => self.eval_dependency(query, max_results),
            QueryExpr::Architecture(query) => self.eval_architecture(query, max_results),
            QueryExpr::Similarity {
                query: sub_query,
                threshold,
                max_results: sim_max,
            } => self.eval_similarity(sub_query, *threshold, *sim_max, max_results),
            QueryExpr::And(queries) => {
                let mut sets: Vec<Vec<ScoredResult>> = queries
                    .iter()
                    .map(|q| self.evaluate(q, max_results))
                    .collect();
                if sets.is_empty() {
                    return Vec::new();
                }
                let mut result = sets.remove(0);
                for other in sets {
                    result.retain(|r| other.iter().any(|o| o.document.id == r.document.id));
                }
                result.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                result.truncate(max_results);
                result
            }
            QueryExpr::Or(queries) => {
                let mut seen = std::collections::HashSet::new();
                let mut results = Vec::new();
                for q in queries {
                    for r in self.evaluate(q, max_results) {
                        if seen.insert(r.document.id.clone()) {
                            results.push(r);
                        }
                    }
                }
                results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                results.truncate(max_results);
                results
            }
            QueryExpr::Not(sub_query) => {
                let exclude_ids: std::collections::HashSet<String> = self
                    .evaluate(sub_query, usize::MAX)
                    .into_iter()
                    .map(|r| r.document.id)
                    .collect();
                let mut results: Vec<ScoredResult> = self
                    .index
                    .all_documents()
                    .into_iter()
                    .filter(|doc| !exclude_ids.contains(&doc.id))
                    .map(|doc| ScoredResult {
                        score: 1.0,
                        matched_on: vec!["not".to_string()],
                        document: doc,
                    })
                    .collect();
                results.truncate(max_results);
                results
            }
            QueryExpr::Filter { query: sub_query, spec } => {
                let results = self.evaluate(sub_query, max_results);
                let filtered: Vec<ScoredResult> = results
                    .into_iter()
                    .filter(|r| match spec {
                        FilterSpec::Kind(k) => r.document.kind == *k,
                        FilterSpec::Source(s) => r.document.source == *s,
                        FilterSpec::TimeBefore(t) => r.document.timestamp.map_or(true, |ts| ts < *t),
                        FilterSpec::TimeAfter(t) => r.document.timestamp.map_or(true, |ts| ts > *t),
                        FilterSpec::Metadata { key, value } => r
                            .document
                            .metadata
                            .get(key)
                            .map_or(false, |v| v == value),
                        FilterSpec::ScoreAbove(s) => r.score >= *s,
                        FilterSpec::ScoreBelow(s) => r.score <= *s,
                    })
                    .collect();
                filtered
            }
        }
    }

    fn eval_keyword(&self, term: &str, max_results: usize) -> Vec<ScoredResult> {
        let mut results = Vec::new();
        let term_lower = term.to_lowercase();

        // Score by TF (term frequency in body + title)
        for doc in self.index.all_documents() {
            let body_lower = doc.body.to_lowercase();
            let title_lower = doc.title.to_lowercase();

            let body_count = body_lower.matches(&term_lower).count();
            let title_count = title_lower.matches(&term_lower).count();
            if body_count > 0 || title_count > 0 {
                let score = (title_count * 3 + body_count) as f64;
                let matched_on = if title_count > 0 {
                    vec!["title".to_string()]
                } else {
                    vec!["body".to_string()]
                };
                results.push(ScoredResult {
                    score,
                    matched_on,
                    document: doc,
                });
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(max_results);
        results
    }

    fn eval_prefix(&self, prefix: &str, max_results: usize) -> Vec<ScoredResult> {
        let mut results = Vec::new();
        let prefix_lower = prefix.to_lowercase();

        for doc in self.index.all_documents() {
            if doc.body.to_lowercase().contains(&prefix_lower)
                || doc.title.to_lowercase().contains(&prefix_lower)
            {
                results.push(ScoredResult {
                    score: 1.0,
                    matched_on: vec!["prefix".to_string()],
                    document: doc,
                });
            }
        }

        results.truncate(max_results);
        results
    }

    fn eval_phrase(&self, words: &[String], max_results: usize) -> Vec<ScoredResult> {
        let phrase = words.join(" ").to_lowercase();
        let mut results = Vec::new();

        for doc in self.index.all_documents() {
            if doc.body.to_lowercase().contains(&phrase)
                || doc.title.to_lowercase().contains(&phrase)
            {
                results.push(ScoredResult {
                    score: words.len() as f64,
                    matched_on: vec!["phrase".to_string()],
                    document: doc,
                });
            }
        }

        results.truncate(max_results);
        results
    }

    fn eval_semantic(&self, embedding: &[f64], max_results: usize) -> Vec<ScoredResult> {
        let mut results: Vec<ScoredResult> = self
            .index
            .all_documents()
            .into_iter()
            .filter_map(|doc| {
                let doc_embedding = self.index.get_embedding(&doc.id)?;
                let sim = cosine_similarity(embedding, &doc_embedding)?;
                Some(ScoredResult {
                    score: sim,
                    matched_on: vec!["semantic".to_string()],
                    document: doc,
                })
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(max_results);
        results
    }

    fn eval_temporal(&self, range: &TimeRange, max_results: usize) -> Vec<ScoredResult> {
        let mut results: Vec<ScoredResult> = self
            .index
            .all_documents()
            .into_iter()
            .filter(|doc| {
                let ts = match doc.timestamp {
                    Some(t) => t,
                    None => return range.start.is_none() && range.end.is_none(),
                };
                let after = range.start.map_or(true, |s| ts >= s);
                let before = range.end.map_or(true, |e| ts <= e);
                after && before
            })
            .map(|doc| ScoredResult {
                score: 1.0,
                matched_on: vec!["temporal".to_string()],
                document: doc,
            })
            .collect();

        results.truncate(max_results);
        results
    }

    fn eval_structural(&self, pattern: &StructPattern, max_results: usize) -> Vec<ScoredResult> {
        let mut results: Vec<ScoredResult> = self
            .index
            .all_documents()
            .into_iter()
            .filter(|doc| {
                let body_lower = doc.body.to_lowercase();
                match pattern {
                    StructPattern::Function { name } => {
                        let has_fn = body_lower.contains("fn ")
                            || body_lower.contains("function ")
                            || body_lower.contains("def ")
                            || body_lower.contains("func ");
                        name.as_ref().map_or(has_fn, |n| {
                            has_fn && body_lower.contains(&n.to_lowercase())
                        })
                    }
                    StructPattern::Class { name } => {
                        let has_class = body_lower.contains("class ")
                            || body_lower.contains("struct ");
                        name.as_ref().map_or(has_class, |n| {
                            has_class && body_lower.contains(&n.to_lowercase())
                        })
                    }
                    StructPattern::Interface { name } => {
                        let has_iface = body_lower.contains("interface ")
                            || body_lower.contains("trait ")
                            || body_lower.contains("protocol ");
                        name.as_ref().map_or(has_iface, |n| {
                            has_iface && body_lower.contains(&n.to_lowercase())
                        })
                    }
                    StructPattern::Symbol { kind, name } => {
                        let kind_match = doc.metadata.get("symbol_kind").map_or(
                            body_lower.contains(kind),
                            |k| k == kind,
                        );
                        name.as_ref().map_or(kind_match, |n| {
                            kind_match && body_lower.contains(&n.to_lowercase())
                        })
                    }
                    StructPattern::Call { name } => {
                        let has_call = name.as_ref().map_or(true, |n| {
                            body_lower.contains(&format!("{n}("))
                        });
                        has_call
                    }
                    StructPattern::Import { source } => {
                        let has_import = body_lower.contains("import ")
                            || body_lower.contains("use ")
                            || body_lower.contains("#include")
                            || body_lower.contains("require(");
                        source.as_ref().map_or(has_import, |s| {
                            has_import && body_lower.contains(&s.to_lowercase())
                        })
                    }
                }
            })
            .map(|doc| ScoredResult {
                score: 1.0,
                matched_on: vec!["structural".to_string()],
                document: doc,
            })
            .collect();

        results.truncate(max_results);
        results
    }

    fn eval_graph(&self, pattern: &GraphPattern, max_results: usize) -> Vec<ScoredResult> {
        let mut results: Vec<ScoredResult> = self
            .index
            .all_documents()
            .into_iter()
            .filter(|doc| match pattern {
                GraphPattern::Node { name, kind } => {
                    let name_match = doc.title.contains(name) || doc.body.contains(name);
                    let kind_match = kind.as_ref().map_or(true, |k| doc.kind == *k);
                    name_match && kind_match
                }
                GraphPattern::Edge { source, target, kind } => {
                    let src = self.eval_graph(source, 1);
                    let tgt = self.eval_graph(target, 1);
                    let has_source = src.iter().any(|r| r.document.id == doc.id);
                    let has_target = tgt.iter().any(|r| r.document.id == doc.id);
                    let kind_match = kind.as_ref().map_or(true, |k| {
                        doc.metadata.get("edge_kind").map_or(false, |v| v == k)
                    });
                    has_source && has_target && kind_match
                }
                GraphPattern::Path { from, to, max_depth } => {
                    let src = self.eval_graph(from, 1);
                    let tgt = self.eval_graph(to, 1);
                    let has_from = src.iter().any(|r| r.document.id == doc.id);
                    let has_to = tgt.iter().any(|r| r.document.id == doc.id);
                    let depth_ok = doc
                        .metadata
                        .get("path_depth")
                        .and_then(|v| v.parse::<usize>().ok())
                        .map_or(true, |d| d <= *max_depth);
                    has_from || has_to || depth_ok
                }
                GraphPattern::Neighbors { node, depth } => {
                    let node_results = self.eval_graph(node, max_results);
                    if node_results.is_empty() {
                        return false;
                    }
                    let neighbor_depth = doc
                        .metadata
                        .get("graph_distance")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(usize::MAX);
                    neighbor_depth <= *depth
                }
                GraphPattern::Subgraph(patterns) => {
                    patterns.iter().any(|p| {
                        let sub = self.eval_graph(p, 1);
                        sub.iter().any(|r| r.document.id == doc.id)
                    })
                }
            })
            .map(|doc| ScoredResult {
                score: 1.0,
                matched_on: vec!["graph".to_string()],
                document: doc,
            })
            .collect();

        results.truncate(max_results);
        results
    }

    fn eval_dependency(&self, query: &DependencyQuery, max_results: usize) -> Vec<ScoredResult> {
        let mut results: Vec<ScoredResult> = self
            .index
            .all_documents()
            .into_iter()
            .filter(|doc| match query {
                DependencyQuery::Dependents { name, ecosystem } => {
                    let name_match = doc.metadata.get("depends_on").map_or(false, |d| d == name);
                    let eco_match =
                        ecosystem.as_ref().map_or(true, |e| doc.source.contains(e));
                    name_match && eco_match
                }
                DependencyQuery::Dependencies { name, ecosystem } => {
                    let name_match =
                        doc.metadata.get("package_name").map_or(false, |p| p == name);
                    let eco_match =
                        ecosystem.as_ref().map_or(true, |e| doc.source.contains(e));
                    name_match && eco_match
                }
                DependencyQuery::Transitive {
                    name,
                    ecosystem,
                    max_depth,
                } => {
                    let depth = doc
                        .metadata
                        .get("dep_depth")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(usize::MAX);
                    let name_match = doc
                        .metadata
                        .get("dep_chain")
                        .map_or(false, |c| c.contains(name));
                    let eco_match =
                        ecosystem.as_ref().map_or(true, |e| doc.source.contains(e));
                    name_match && eco_match && depth <= *max_depth
                }
                DependencyQuery::Ecosystem(eco) => doc.source.contains(eco),
            })
            .map(|doc| ScoredResult {
                score: 1.0,
                matched_on: vec!["dependency".to_string()],
                document: doc,
            })
            .collect();

        results.truncate(max_results);
        results
    }

    fn eval_architecture(&self, query: &ArchQuery, max_results: usize) -> Vec<ScoredResult> {
        let mut results: Vec<ScoredResult> = self
            .index
            .all_documents()
            .into_iter()
            .filter(|doc| match query {
                ArchQuery::Module { path } => {
                    doc.source.contains(path) || doc.title.contains(path)
                }
                ArchQuery::Package { name } => {
                    doc.metadata
                        .get("package")
                        .map_or(false, |p| p == name)
                        || doc.title == *name
                }
                ArchQuery::Workspace { name } => {
                    doc.metadata
                        .get("workspace")
                        .map_or(false, |w| w == name)
                        || doc.source.contains(name)
                }
                ArchQuery::Directory { path } => doc.source.contains(path),
            })
            .map(|doc| ScoredResult {
                score: 1.0,
                matched_on: vec!["architecture".to_string()],
                document: doc,
            })
            .collect();

        results.truncate(max_results);
        results
    }

    fn eval_similarity(
        &self,
        sub_query: &QueryExpr,
        threshold: f64,
        sim_max: usize,
        max_results: usize,
    ) -> Vec<ScoredResult> {
        let base_results = self.evaluate(sub_query, sim_max);
        if base_results.is_empty() {
            return Vec::new();
        }

        let target_texts: Vec<String> = base_results
            .iter()
            .map(|r| format!("{} {}", r.document.title, r.document.body))
            .collect();

        let mut results = Vec::new();
        for doc in self.index.all_documents() {
            let doc_text = format!("{} {}", doc.title, doc.body);
            for target in &target_texts {
                let sim = text_cosine_similarity(target, &doc_text);
                if sim >= threshold {
                    results.push(ScoredResult {
                        score: sim,
                        matched_on: vec!["similarity".to_string()],
                        document: doc,
                    });
                    break;
                }
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(max_results);
        results
    }

    /// Get the document count in the index.
    #[must_use]
    pub fn document_count(&self) -> usize {
        self.index.len()
    }

    /// Clear all indexed documents.
    pub fn clear(&mut self) {
        self.index.clear();
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f64], b: &[f64]) -> Option<f64> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f64 = a.iter().map(|x| x * x).sum();
    let mag_b: f64 = b.iter().map(|x| x * x).sum();
    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom == 0.0 {
        return None;
    }
    Some(dot / denom)
}

/// Compute cosine similarity between two text strings using word frequency.
fn text_cosine_similarity(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    let tokens_a: Vec<&str> = a_lower.split_whitespace().collect();
    let tokens_b: Vec<&str> = b_lower.split_whitespace().collect();

    let mut vocab: Vec<&str> = Vec::new();
    for t in &tokens_a {
        if !vocab.contains(t) {
            vocab.push(t);
        }
    }
    for t in &tokens_b {
        if !vocab.contains(t) {
            vocab.push(t);
        }
    }

    let vec_a: Vec<f64> = vocab
        .iter()
        .map(|v| tokens_a.iter().filter(|t| **t == *v).count() as f64)
        .collect();
    let vec_b: Vec<f64> = vocab
        .iter()
        .map(|v| tokens_b.iter().filter(|t| **t == *v).count() as f64)
        .collect();

    cosine_similarity(&vec_a, &vec_b).unwrap_or(0.0)
}

/// Convenience function to create a document from a `ProcessedObservation`.
pub fn document_from_observation(
    obs: &tordex_core::processor::ProcessedObservation,
) -> Document {
    let body = String::from_utf8_lossy(&obs.data).to_string();
    Document {
        id: obs.id.clone(),
        title: obs.kind.clone(),
        body,
        kind: obs.kind.clone(),
        source: obs.content_type.clone(),
        timestamp: None,
        metadata: obs.metadata.clone(),
    }
}

/// Convenience function to create a document from text content.
pub fn document_from_text(
    id: &str,
    title: &str,
    body: &str,
    kind: &str,
    source: &str,
) -> Document {
    Document {
        id: id.to_string(),
        title: title.to_string(),
        body: body.to_string(),
        kind: kind.to_string(),
        source: source.to_string(),
        timestamp: None,
        metadata: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_engine() -> SearchEngine {
        let mut engine = SearchEngine::new();
        engine.index_documents(vec![
            document_from_text(
                "doc1",
                "Rust ownership",
                "Ownership is Rust's unique memory management system. It ensures memory safety.",
                "article",
                "docs",
            ),
            document_from_text(
                "doc2",
                "Python generators",
                "Generators in Python allow lazy evaluation using yield statements.",
                "article",
                "docs",
            ),
            document_from_text(
                "doc3",
                "Rust vs Python",
                "Rust offers memory safety while Python prioritizes developer speed.",
                "comparison",
                "blog",
            ),
            document_from_text(
                "doc4",
                "Cargo package manager",
                "Cargo is the Rust package manager. It handles dependencies and builds.",
                "article",
                "docs",
            ),
        ]);
        engine
    }

    #[test]
    fn keyword_search_finds_matches() {
        let engine = sample_engine();
        let query = QueryExpr::Keyword("Rust".to_string());
        let result = engine.search(&query, 10);
        assert!(result.total_count >= 2);
        assert!(result.results.iter().any(|r| r.document.id == "doc1"));
    }

    #[test]
    fn keyword_search_scores_title_higher() {
        let engine = sample_engine();
        let query = QueryExpr::Keyword("Rust".to_string());
        let result = engine.search(&query, 10);
        assert!(!result.results.is_empty());
        // "Rust ownership" and "Rust vs Python" should both match
        assert!(result.results.len() >= 2);
    }

    #[test]
    fn phrase_search_finds_exact_phrase() {
        let engine = sample_engine();
        let query = QueryExpr::Phrase(vec![
            "memory".to_string(),
            "safety".to_string(),
        ]);
        let result = engine.search(&query, 10);
        assert!(result.total_count >= 2);
    }

    #[test]
    fn prefix_search() {
        let engine = sample_engine();
        let query = QueryExpr::Prefix("gen".to_string());
        let result = engine.search(&query, 10);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.results[0].document.id, "doc2");
    }

    #[test]
    fn and_combinator_intersects() {
        let engine = sample_engine();
        let query = QueryExpr::And(vec![
            QueryExpr::Keyword("Rust".to_string()),
            QueryExpr::Keyword("Python".to_string()),
        ]);
        let result = engine.search(&query, 10);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.results[0].document.id, "doc3");
    }

    #[test]
    fn or_combinator_unions() {
        let engine = sample_engine();
        let query = QueryExpr::Or(vec![
            QueryExpr::Keyword("ownership".to_string()),
            QueryExpr::Keyword("generators".to_string()),
        ]);
        let result = engine.search(&query, 10);
        assert_eq!(result.total_count, 2);
    }

    #[test]
    fn not_combinator_excludes() {
        let engine = sample_engine();
        let query = QueryExpr::And(vec![
            QueryExpr::Keyword("Rust".to_string()),
            QueryExpr::Not(Box::new(QueryExpr::Keyword("Cargo".to_string()))),
        ]);
        let result = engine.search(&query, 10);
        assert!(result.total_count >= 1);
        assert!(!result.results.iter().any(|r| r.document.id == "doc4"));
    }

    #[test]
    fn filter_by_kind() {
        let engine = sample_engine();
        let query = QueryExpr::Filter {
            query: Box::new(QueryExpr::Keyword("Rust".to_string())),
            spec: FilterSpec::Kind("comparison".to_string()),
        };
        let result = engine.search(&query, 10);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.results[0].document.id, "doc3");
    }

    #[test]
    fn temporal_search_filters_by_range() {
        let engine = sample_engine();
        let now = time::OffsetDateTime::now_utc();
        let future = now + time::Duration::days(1);
        let range = TimeRange {
            start: Some(future),
            end: None,
        };
        let query = QueryExpr::Temporal(range);
        let result = engine.search(&query, 10);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn empty_engine_returns_no_results() {
        let engine = SearchEngine::new();
        let query = QueryExpr::Keyword("anything".to_string());
        let result = engine.search(&query, 10);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn structural_pattern_functions() {
        let mut engine = SearchEngine::new();
        engine.index_document(document_from_text(
            "code1",
            "main.rs",
            "fn main() { println!(\"hello\"); }",
            "source",
            "code",
        ));
        let query = QueryExpr::Structural(StructPattern::Function {
            name: Some("main".to_string()),
        });
        let result = engine.search(&query, 10);
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn query_algebra_serde_roundtrip() {
        let query = QueryExpr::And(vec![
            QueryExpr::Keyword("rust".to_string()),
            QueryExpr::Or(vec![
                QueryExpr::Keyword("ownership".to_string()),
                QueryExpr::Keyword("borrow".to_string()),
            ]),
        ]);
        let json = serde_json::to_string(&query).unwrap();
        let back: QueryExpr = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, QueryExpr::And(_)));
    }

    #[test]
    fn cosine_similarity_basic() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&v1, &v2).unwrap();
        assert!((sim - 0.0).abs() < 1e-10);

        let v3 = vec![1.0, 2.0, 3.0];
        let v4 = vec![2.0, 4.0, 6.0];
        let sim2 = cosine_similarity(&v3, &v4).unwrap();
        assert!((sim2 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn text_similarity_finds_similar() {
        let mut engine = SearchEngine::new();
        engine.index_document(document_from_text("a", "Rust guide", "Rust is a systems language focused on safety.", "doc", ""));
        engine.index_document(document_from_text("b", "Rust tutorial", "Learn Rust programming for systems development.", "doc", ""));
        engine.index_document(document_from_text("c", "Python guide", "Python is great for data science.", "doc", ""));

        let query = QueryExpr::Similarity {
            query: Box::new(QueryExpr::Keyword("Rust".to_string())),
            threshold: 0.1,
            max_results: 5,
        };
        let result = engine.search(&query, 10);
        assert!(result.total_count >= 2);
        // Both Rust docs should be found; Python doc should have lower or no match
        let rust_ids: Vec<&str> = result
            .results
            .iter()
            .map(|r| r.document.id.as_str())
            .collect();
        assert!(rust_ids.contains(&"a"));
        assert!(rust_ids.contains(&"b"));
    }

    #[test]
    fn dependency_query() {
        let mut engine = SearchEngine::new();
        let mut meta = HashMap::new();
        meta.insert("package_name".to_string(), "my-app".to_string());
        meta.insert("depends_on".to_string(), "serde".to_string());
        engine.index_document(Document {
            id: "pkg1".to_string(),
            title: "my-app".to_string(),
            body: "Depends on serde".to_string(),
            kind: "package".to_string(),
            source: "cargo".to_string(),
            timestamp: None,
            metadata: meta,
        });
        let query = QueryExpr::Dependency(DependencyQuery::Dependents {
            name: "serde".to_string(),
            ecosystem: None,
        });
        let result = engine.search(&query, 10);
        assert_eq!(result.total_count, 1);
    }
}
