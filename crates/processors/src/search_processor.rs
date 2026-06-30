use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};
use tordex_search::{document_from_observation, Document, QueryExpr, SearchEngine};

/// A processor that wraps the Intelligence Search Engine into the observation pipeline.
///
/// Accepts `application/x-search` content type and provides full-text,
/// semantic, structural, graph, dependency, and architecture search over
/// indexed observations.
///
/// Use `action` metadata to select the operation:
/// - `"index"` — index an observation's data as a search document
/// - `"search"` — keyword search (uses `query` metadata field)
/// - `"query"` — execute a serialized `QueryExpr` (from JSON body)
/// - `"clear"` — remove all indexed documents
/// - `"count"` — get the number of indexed documents
pub struct SearchProcessor {
    engine: std::sync::Mutex<SearchEngine>,
}

impl SearchProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: std::sync::Mutex::new(SearchEngine::new()),
        }
    }
}

impl Default for SearchProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for SearchProcessor {
    fn name(&self) -> &str {
        "SearchProcessor"
    }

    fn description(&self) -> &str {
        "Intelligence Search Engine — indexes observations and supports keyword, phrase, prefix, semantic, temporal, structural, graph, dependency, architecture, and similarity queries via QueryExpr algebra"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-search"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let action = metadata
            .get("action")
            .map(|s| s.as_str())
            .unwrap_or("search");

        let mut engine = self.engine.lock().map_err(|e| {
            ProcessorError::ProcessingFailed(format!("lock error: {e}"))
        })?;

        match action {
            "index" => {
                let json_value: serde_json::Value =
                    serde_json::from_slice(data).map_err(|e| {
                        ProcessorError::InvalidInput(format!("invalid JSON: {e}"))
                    })?;

                // Accept either a Document or a ProcessedObservation
                let doc = if let Some(title) = json_value["title"].as_str() {
                    // Direct Document
                    let doc_id = json_value["id"]
                        .as_str()
                        .unwrap_or(id);
                    Document {
                        id: doc_id.to_string(),
                        title: title.to_string(),
                        body: json_value["body"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        kind: json_value["kind"]
                            .as_str()
                            .unwrap_or("unknown")
                            .to_string(),
                        source: json_value["source"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        timestamp: None,
                        metadata: json_value["metadata"]
                            .as_object()
                            .map(|obj| {
                                obj.iter()
                                    .map(|(k, v)| {
                                        (k.clone(), v.as_str().unwrap_or("").to_string())
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    }
                } else {
                    // Try to parse as ProcessedObservation
                    let obs_id = json_value["id"]
                        .as_str()
                        .unwrap_or(id);
                    let kind = json_value["kind"]
                        .as_str()
                        .unwrap_or("search.document");
                    let data_str = json_value["data"]
                        .as_str()
                        .unwrap_or("");
                    let ct = json_value["content_type"]
                        .as_str()
                        .unwrap_or("text/plain");
                    let obs_metadata: HashMap<String, String> = json_value["metadata"]
                        .as_object()
                        .map(|obj| {
                            obj.iter()
                                .map(|(k, v)| {
                                    (k.clone(), v.as_str().unwrap_or("").to_string())
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let obs = ProcessedObservation::new(
                        obs_id.to_string(),
                        kind,
                        data_str.as_bytes().to_vec(),
                        ct,
                    )
                    .with_metadata_from(obs_metadata);
                    document_from_observation(&obs)
                };

                engine.index_document(doc);
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "search.indexed",
                    serde_json::to_vec(&serde_json::json!({"indexed": true}))
                        .unwrap_or_default(),
                    "application/x-search+result",
                )
                .with_metadata(
                    "document_count",
                    &engine.document_count().to_string(),
                )])
            }

            "query" => {
                let query: QueryExpr =
                    serde_json::from_slice(data).map_err(|e| {
                        ProcessorError::InvalidInput(format!(
                            "invalid QueryExpr JSON: {e}"
                        ))
                    })?;
                let max_results: usize = metadata
                    .get("max_results")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10);
                let result = engine.search(&query, max_results);
                let output = serde_json::to_value(&result).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "search.results",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-search+results",
                )
                .with_metadata("total", &result.total_count.to_string())
                .with_metadata("took_ns", &result.took_ns.to_string())])
            }

            "search" => {
                let query_text = metadata.get("query").ok_or_else(|| {
                    ProcessorError::InvalidInput(
                        "missing 'query' metadata for search action".to_string(),
                    )
                })?;
                let max_results: usize = metadata
                    .get("max_results")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10);
                let query = QueryExpr::Keyword(query_text.clone());
                let result = engine.search(&query, max_results);
                let output = serde_json::to_value(&result).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "search.results",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-search+results",
                )
                .with_metadata("total", &result.total_count.to_string())
                .with_metadata("query", query_text)
                .with_metadata("took_ns", &result.took_ns.to_string())])
            }

            "clear" => {
                engine.clear();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "search.cleared",
                    serde_json::to_vec(&serde_json::json!({"cleared": true}))
                        .unwrap_or_default(),
                    "application/x-search+result",
                )])
            }

            "count" => {
                let count = engine.document_count();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "search.count",
                    serde_json::to_vec(&serde_json::json!({"count": count}))
                        .unwrap_or_default(),
                    "application/x-search+result",
                )
                .with_metadata("count", &count.to_string())])
            }

            _ => Err(ProcessorError::InvalidInput(format!(
                "unknown action: {action}"
            ))),
        }
    }
}

/// Helper to build metadata from a HashMap for ProcessedObservation.
trait WithMetadataFrom {
    fn with_metadata_from(self, metadata: HashMap<String, String>) -> Self;
}

impl WithMetadataFrom for ProcessedObservation {
    fn with_metadata_from(self, metadata: HashMap<String, String>) -> Self {
        let mut obs = self;
        for (k, v) in metadata {
            obs = obs.with_metadata(&k, &v);
        }
        obs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_processor() -> SearchProcessor {
        SearchProcessor::new()
    }

    #[test]
    fn name_and_content_types() {
        let p = make_processor();
        assert_eq!(p.name(), "SearchProcessor");
        assert!(p.content_types().contains(&"application/x-search"));
    }

    #[test]
    fn index_and_count() {
        let p = make_processor();
        let doc = serde_json::json!({
            "title": "test doc",
            "body": "hello world",
            "kind": "test",
            "source": "test",
        });
        let results = p
            .process(
                "obs_001",
                &serde_json::to_vec(&doc).unwrap(),
                Some("application/x-search"),
                HashMap::from([("action".into(), "index".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "search.indexed");
        assert_eq!(results[0].metadata.get("document_count").unwrap(), "1");

        // count
        let count_result = p
            .process(
                "obs_002",
                b"{}",
                Some("application/x-search"),
                HashMap::from([("action".into(), "count".into())]),
            )
            .unwrap();
        assert_eq!(count_result[0].kind, "search.count");
        assert_eq!(count_result[0].metadata.get("count").unwrap(), "1");
    }

    #[test]
    fn keyword_search_after_index() {
        let p = make_processor();
        let doc = serde_json::json!({
            "title": "Rust ownership",
            "body": "Ownership is Rust's memory management system.",
            "kind": "article",
            "source": "docs",
        });
        p.process(
            "obs_001",
            &serde_json::to_vec(&doc).unwrap(),
            Some("application/x-search"),
            HashMap::from([("action".into(), "index".into())]),
        )
        .unwrap();

        let results = p
            .process(
                "obs_002",
                b"{}",
                Some("application/x-search"),
                HashMap::from([
                    ("action".into(), "search".into()),
                    ("query".into(), "Rust".into()),
                ]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "search.results");
        assert_eq!(results[0].metadata.get("total").unwrap(), "1");
        assert_eq!(results[0].metadata.get("query").unwrap(), "Rust");
    }

    #[test]
    fn query_with_queryexpr() {
        let p = make_processor();
        // Index some docs
        let doc1 = serde_json::json!({
            "id": "d1",
            "title": "Rust book",
            "body": "The Rust programming language",
            "kind": "article",
            "source": "docs",
        });
        let doc2 = serde_json::json!({
            "id": "d2",
            "title": "Python book",
            "body": "The Python programming language",
            "kind": "article",
            "source": "docs",
        });
        p.process(
            "obs_001",
            &serde_json::to_vec(&doc1).unwrap(),
            Some("application/x-search"),
            HashMap::from([("action".into(), "index".into())]),
        )
        .unwrap();
        p.process(
            "obs_002",
            &serde_json::to_vec(&doc2).unwrap(),
            Some("application/x-search"),
            HashMap::from([("action".into(), "index".into())]),
        )
        .unwrap();

        // Query using QueryExpr
        let query = serde_json::json!({"Keyword": "Rust"});
        let results = p
            .process(
                "obs_003",
                &serde_json::to_vec(&query).unwrap(),
                Some("application/x-search"),
                HashMap::from([("action".into(), "query".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "search.results");
        let total: usize = results[0]
            .metadata
            .get("total")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(total, 1);
    }

    #[test]
    fn clear_engine() {
        let p = make_processor();
        // Index one doc
        let doc = serde_json::json!({
            "title": "test",
            "body": "content",
            "kind": "test",
            "source": "test",
        });
        p.process(
            "obs_001",
            &serde_json::to_vec(&doc).unwrap(),
            Some("application/x-search"),
            HashMap::from([("action".into(), "index".into())]),
        )
        .unwrap();

        // Clear
        let clear_result = p
            .process(
                "obs_002",
                b"{}",
                Some("application/x-search"),
                HashMap::from([("action".into(), "clear".into())]),
            )
            .unwrap();
        assert_eq!(clear_result[0].kind, "search.cleared");

        // Count should be 0
        let count_result = p
            .process(
                "obs_003",
                b"{}",
                Some("application/x-search"),
                HashMap::from([("action".into(), "count".into())]),
            )
            .unwrap();
        assert_eq!(count_result[0].metadata.get("count").unwrap(), "0");
    }

    #[test]
    fn unknown_action_returns_error() {
        let p = make_processor();
        let result = p.process(
            "obs_001",
            b"{}",
            Some("application/x-search"),
            HashMap::from([("action".into(), "bogus".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn search_missing_query_returns_error() {
        let p = make_processor();
        let result = p.process(
            "obs_001",
            b"{}",
            Some("application/x-search"),
            HashMap::from([("action".into(), "search".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn and_or_not_combinator_via_query() {
        let p = make_processor();
        let docs = vec![
            serde_json::json!({"id": "a", "title": "Rust A", "body": "Rust is safe", "kind": "doc", "source": "s"}),
            serde_json::json!({"id": "b", "title": "Rust B", "body": "Rust is fast", "kind": "doc", "source": "s"}),
            serde_json::json!({"id": "c", "title": "Python C", "body": "Python is easy", "kind": "doc", "source": "s"}),
        ];
        for (i, doc) in docs.iter().enumerate() {
            p.process(
                &format!("obs_{i}"),
                &serde_json::to_vec(doc).unwrap(),
                Some("application/x-search"),
                HashMap::from([("action".into(), "index".into())]),
            )
            .unwrap();
        }

        // AND query: Rust AND Python — should return none
        let and_query = serde_json::json!({
            "And": [
                {"Keyword": "Rust"},
                {"Keyword": "Python"}
            ]
        });
        let results = p
            .process(
                "obs_q1",
                &serde_json::to_vec(&and_query).unwrap(),
                Some("application/x-search"),
                HashMap::from([("action".into(), "query".into())]),
            )
            .unwrap();
        let total: usize = results[0]
            .metadata
            .get("total")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(total, 0);

        // OR query: Rust OR Python — should return 3
        let or_query = serde_json::json!({
            "Or": [
                {"Keyword": "Rust"},
                {"Keyword": "Python"}
            ]
        });
        let results = p
            .process(
                "obs_q2",
                &serde_json::to_vec(&or_query).unwrap(),
                Some("application/x-search"),
                HashMap::from([("action".into(), "query".into())]),
            )
            .unwrap();
        let total: usize = results[0]
            .metadata
            .get("total")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(total, 3);
    }

    #[test]
    fn no_match_returns_empty() {
        let p = make_processor();
        let results = p
            .process(
                "obs_001",
                b"{}",
                Some("application/x-search"),
                HashMap::from([
                    ("action".into(), "search".into()),
                    ("query".into(), "nonexistent".into()),
                ]),
            )
            .unwrap();
        let total: usize = results[0]
            .metadata
            .get("total")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(total, 0);
    }
}
