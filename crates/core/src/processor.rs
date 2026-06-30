//! Processor architecture.
//!
//! Processors transform collected raw data into structured observations.
//! Each processor declares what content types it can handle and produces
//! zero or more `ProcessedObservation`s that are emitted as events.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── ProcessorError ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Error)]
pub enum ProcessorError {
    #[error("unsupported content type: {0}")]
    UnsupportedContent(String),
    #[error("processing failed: {0}")]
    ProcessingFailed(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
}

// ─── ProcessedObservation ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedObservation {
    pub id: String,
    pub kind: String,
    pub data: Vec<u8>,
    pub content_type: String,
    pub metadata: HashMap<String, String>,
    pub derived: Vec<DerivedArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedArtifact {
    pub kind: String,
    pub data: Vec<u8>,
    pub content_type: String,
    pub metadata: HashMap<String, String>,
}

impl ProcessedObservation {
    #[must_use]
    pub fn new(id: String, kind: &str, data: Vec<u8>, content_type: &str) -> Self {
        Self {
            id,
            kind: kind.to_string(),
            data,
            content_type: content_type.to_string(),
            metadata: HashMap::new(),
            derived: Vec::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn add_derived(&mut self, artifact: DerivedArtifact) {
        self.derived.push(artifact);
    }
}

// ─── Processor trait ─────────────────────────────────────────────────────────

pub trait Processor: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn content_types(&self) -> Vec<&str>;
    fn process(
        &self,
        id: &str,
        data: &[u8],
        content_type: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError>;
}

// ─── ProcessorRegistry trait ─────────────────────────────────────────────────

pub trait ProcessorRegistry: Send + Sync {
    fn register(&self, processor: Box<dyn Processor>) -> Result<(), ProcessorError>;
    fn unregister(&self, name: &str) -> Result<(), ProcessorError>;
    fn get(&self, name: &str) -> Option<Arc<dyn Processor>>;
    fn processors_for(&self, content_type: &str) -> Vec<Arc<dyn Processor>>;
    fn list(&self) -> Vec<String>;
    fn process(
        &self,
        id: &str,
        data: &[u8],
        content_type: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> Vec<ProcessedObservation>;
}

// ─── InMemoryProcessorRegistry ───────────────────────────────────────────────

pub struct InMemoryProcessorRegistry {
    inner: Arc<Mutex<HashMap<String, Arc<dyn Processor>>>>,
}

impl InMemoryProcessorRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryProcessorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessorRegistry for InMemoryProcessorRegistry {
    fn register(&self, processor: Box<dyn Processor>) -> Result<(), ProcessorError> {
        let mut inner = self.inner.lock().unwrap();
        let name = processor.name().to_string();
        inner.insert(name, Arc::from(processor));
        Ok(())
    }

    fn unregister(&self, name: &str) -> Result<(), ProcessorError> {
        let mut inner = self.inner.lock().unwrap();
        inner
            .remove(name)
            .ok_or_else(|| ProcessorError::ProcessingFailed(format!("processor not found: {name}")))?;
        Ok(())
    }

    fn get(&self, name: &str) -> Option<Arc<dyn Processor>> {
        self.inner.lock().unwrap().get(name).cloned()
    }

    fn processors_for(&self, content_type: &str) -> Vec<Arc<dyn Processor>> {
        let inner = self.inner.lock().unwrap();
        inner
            .values()
            .filter(|p| p.content_types().iter().any(|ct| content_type.starts_with(ct)))
            .cloned()
            .collect()
    }

    fn list(&self) -> Vec<String> {
        self.inner.lock().unwrap().keys().cloned().collect()
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        content_type: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> Vec<ProcessedObservation> {
        let inner = self.inner.lock().unwrap();
        let mut results = Vec::new();
        for processor in inner.values() {
            if content_type.map_or(true, |ct| {
                processor.content_types().iter().any(|pct| ct.starts_with(pct))
            }) {
                if let Ok(obs) = processor.process(id, data, content_type, metadata.clone()) {
                    results.extend(obs);
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProcessor;

    impl Processor for TestProcessor {
        fn name(&self) -> &str { "test" }
        fn description(&self) -> &str { "test processor" }
        fn content_types(&self) -> Vec<&str> { vec!["text/plain"] }
        fn process(
            &self,
            id: &str,
            data: &[u8],
            _content_type: Option<&str>,
            _metadata: HashMap<String, String>,
        ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
            Ok(vec![ProcessedObservation::new(
                id.to_string(),
                "test.output",
                data.to_vec(),
                "text/plain",
            )])
        }
    }

    #[test]
    fn register_and_list() {
        let reg = InMemoryProcessorRegistry::new();
        reg.register(Box::new(TestProcessor)).unwrap();
        assert_eq!(reg.list(), vec!["test"]);
    }

    #[test]
    fn process_matching_content_type() {
        let reg = InMemoryProcessorRegistry::new();
        reg.register(Box::new(TestProcessor)).unwrap();
        let results = reg.process("obs1", b"hello world", Some("text/plain"), HashMap::new());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "test.output");
    }

    #[test]
    fn process_non_matching_content_type() {
        let reg = InMemoryProcessorRegistry::new();
        reg.register(Box::new(TestProcessor)).unwrap();
        let results = reg.process("obs2", b"data", Some("application/json"), HashMap::new());
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn process_no_content_type_matches_any() {
        let reg = InMemoryProcessorRegistry::new();
        reg.register(Box::new(TestProcessor)).unwrap();
        let results = reg.process("obs3", b"data", None, HashMap::new());
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn get_returns_processor() {
        let reg = InMemoryProcessorRegistry::new();
        reg.register(Box::new(TestProcessor)).unwrap();
        let p = reg.get("test");
        assert!(p.is_some());
        assert_eq!(p.unwrap().name(), "test");
    }

    #[test]
    fn processors_for_matches_prefix() {
        let reg = InMemoryProcessorRegistry::new();
        reg.register(Box::new(TestProcessor)).unwrap();
        let matching = reg.processors_for("text/plain; charset=utf-8");
        assert!(!matching.is_empty());
    }
}
