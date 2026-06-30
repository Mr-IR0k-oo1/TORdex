use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

/// A processor that bridges the Knowledge Core into the observation pipeline.
///
/// Accepts `application/x-knowledge` content type and performs:
/// - Canonicalization of structured data
/// - Fingerprinting (SHA-256, SimHash)
/// - Identity resolution against known entities
/// - Deduplication checking
/// - Version chain tracking
/// - Confidence scoring
///
/// Use `action` metadata to select the operation:
/// - `"canonicalize"` — normalize JSON content to canonical form
/// - `"fingerprint"` — compute content fingerprint
/// - `"ingest"` — full knowledge ingestion pipeline
/// - `"dedup"` — check if content is a duplicate
pub struct KnowledgeProcessor {
    core: std::sync::Mutex<tordex_knowledge::KnowledgeCore>,
}

impl KnowledgeProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: std::sync::Mutex::new(tordex_knowledge::KnowledgeCore::new()),
        }
    }
}

impl Default for KnowledgeProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for KnowledgeProcessor {
    fn name(&self) -> &str {
        "KnowledgeProcessor"
    }

    fn description(&self) -> &str {
        "Bridges the Knowledge Core into the processing pipeline — canonicalization, fingerprinting, identity resolution, deduplication, versioning, provenance, and confidence scoring"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-knowledge"]
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
            .unwrap_or("ingest");

        let json_value: serde_json::Value =
            serde_json::from_slice(data).map_err(|e| {
                ProcessorError::InvalidInput(format!("invalid JSON: {e}"))
            })?;

        let mut core = self.core.lock().map_err(|e| {
            ProcessorError::ProcessingFailed(format!("lock error: {e}"))
        })?;

        match action {
            "canonicalize" => {
                let form = core.canonicalizer.canonicalize(&json_value);
                let output = serde_json::json!({
                    "canonical": form.normalized,
                    "schema": form.schema,
                });
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "knowledge.canonical",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-knowledge+canonical",
                )])
            }
            "fingerprint" => {
                let fp = tordex_knowledge::Fingerprint::sha256(&json_value);
                let simhash_fp = if let Some(tokens) = extract_tokens(&json_value) {
                    Some(tordex_knowledge::Fingerprint::simhash(tokens.into_iter()))
                } else {
                    None
                };
                let mut obs = ProcessedObservation::new(
                    id.to_string(),
                    "knowledge.fingerprint",
                    fp.hex().into_bytes(),
                    "application/x-knowledge+fingerprint",
                )
                .with_metadata("fingerprint_hex", &fp.hex())
                .with_metadata("fingerprint_type", "sha256");
                if let Some(sim) = simhash_fp {
                    obs.metadata
                        .insert("simhash_hex".to_string(), sim.hex());
                }
                Ok(vec![obs])
            }
            "ingest" => {
                let knowledge = tordex_types::Knowledge {
                    id: tordex_core::KnowledgeId::from_str(id).unwrap_or_else(tordex_core::KnowledgeId::generate),
                    kind: metadata
                        .get("kind")
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string()),
                    content: json_value,
                    confidence: metadata
                        .get("confidence")
                        .and_then(|c| c.parse::<f64>().ok())
                        .unwrap_or(0.5),
                    source_ids: metadata
                        .get("source_ids")
                        .map(|s| s.split(',').map(String::from).collect())
                        .unwrap_or_default(),
                    created_at: tordex_core::now(),
                    metadata: HashMap::new(),
                };
                let record = core.ingest(&knowledge);
                let output = serde_json::to_value(&record).unwrap_or_default();
                let obs = ProcessedObservation::new(
                    id.to_string(),
                    "knowledge.ingested",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-knowledge+record",
                )
                .with_metadata("knowledge_id", &record.id)
                .with_metadata("fingerprint_hex", &record.fingerprint.hex())
                .with_metadata(
                    "confidence",
                    &format!("{:.4}", record.confidence.raw()),
                )
                .with_metadata("version", &record.version.version.to_string())
                .with_metadata("producer", &record.provenance.producer);
                Ok(vec![obs])
            }
            "dedup" => {
                let result = core.check_duplicate(&json_value);
                let output = serde_json::json!({
                    "is_duplicate": result.as_ref().map_or(false, |r| r.is_duplicate),
                    "existing_id": result.as_ref().and_then(|r| r.existing_id.clone()),
                    "similarity": result.as_ref().map_or(1.0, |r| r.similarity),
                });
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "knowledge.dedup",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-knowledge+dedup",
                )])
            }
            _ => Err(ProcessorError::InvalidInput(format!(
                "unknown action: {action}"
            ))),
        }
    }
}

fn extract_tokens(value: &serde_json::Value) -> Option<Vec<&[u8]>> {
    match value {
        serde_json::Value::String(s) => {
            Some(s.split_whitespace().map(|t| t.as_bytes()).collect())
        }
        serde_json::Value::Object(map) => {
            let tokens: Vec<&[u8]> = map
                .values()
                .filter_map(|v| {
                    if let serde_json::Value::String(s) = v {
                        Some(s.as_bytes())
                    } else {
                        None
                    }
                })
                .collect();
            if tokens.is_empty() {
                None
            } else {
                Some(tokens)
            }
        }
        serde_json::Value::Array(arr) => {
            let tokens: Vec<&[u8]> = arr
                .iter()
                .filter_map(|v| {
                    if let serde_json::Value::String(s) = v {
                        Some(s.as_bytes())
                    } else {
                        None
                    }
                })
                .collect();
            if tokens.is_empty() {
                None
            } else {
                Some(tokens)
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_processor() -> KnowledgeProcessor {
        KnowledgeProcessor::new()
    }

    #[test]
    fn name_and_content_types() {
        let p = make_processor();
        assert_eq!(p.name(), "KnowledgeProcessor");
        assert!(p.content_types().contains(&"application/x-knowledge"));
    }

    #[test]
    fn canonicalize_action() {
        let p = make_processor();
        let data = serde_json::json!({"z": 1, "a": 2});
        let results = p
            .process(
                "obs_001",
                &serde_json::to_vec(&data).unwrap(),
                Some("application/x-knowledge"),
                HashMap::from([("action".into(), "canonicalize".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "knowledge.canonical");
    }

    #[test]
    fn fingerprint_action() {
        let p = make_processor();
        let data = serde_json::json!({"key": "value"});
        let results = p
            .process(
                "obs_001",
                &serde_json::to_vec(&data).unwrap(),
                Some("application/x-knowledge"),
                HashMap::from([("action".into(), "fingerprint".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "knowledge.fingerprint");
        assert!(results[0].metadata.contains_key("fingerprint_hex"));
    }

    #[test]
    fn ingest_action_creates_record() {
        let p = make_processor();
        let data = serde_json::json!({"ip": "8.8.8.8", "asn": "15169"});
        let results = p
            .process(
                "obs_001",
                &serde_json::to_vec(&data).unwrap(),
                Some("application/x-knowledge"),
                HashMap::from([
                    ("action".into(), "ingest".into()),
                    ("kind".into(), "dns_enrichment".into()),
                    ("confidence".into(), "0.95".into()),
                ]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "knowledge.ingested");
        assert!(results[0].metadata.contains_key("knowledge_id"));
        assert!(results[0].metadata.contains_key("fingerprint_hex"));
    }

    #[test]
    fn dedup_action_detects_duplicate() {
        let p = make_processor();
        let data = serde_json::json!({"key": "value"});
        let data_bytes = serde_json::to_vec(&data).unwrap();
        // First ingest
        p.process(
            "obs_001",
            &data_bytes,
            Some("application/x-knowledge"),
            HashMap::from([("action".into(), "ingest".into())]),
        )
        .unwrap();
        // Then check dedup
        let results = p
            .process(
                "obs_002",
                &data_bytes,
                Some("application/x-knowledge"),
                HashMap::from([("action".into(), "dedup".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "knowledge.dedup");
        let output: serde_json::Value =
            serde_json::from_slice(&results[0].data).unwrap();
        assert_eq!(output["is_duplicate"], true);
    }

    #[test]
    fn invalid_json_returns_error() {
        let p = make_processor();
        let result = p.process(
            "obs_001",
            b"not valid json",
            Some("application/x-knowledge"),
            HashMap::from([("action".into(), "ingest".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn unknown_action_returns_error() {
        let p = make_processor();
        let data = serde_json::json!({"key": "value"});
        let result = p.process(
            "obs_001",
            &serde_json::to_vec(&data).unwrap(),
            Some("application/x-knowledge"),
            HashMap::from([("action".into(), "bogus".into())]),
        );
        assert!(result.is_err());
    }
}
