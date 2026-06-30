//! Processing Fabric — orchestrates raw data through the processor pipeline.
//!
//! The Processing Fabric is the bridge between collection (raw data) and
//! structured knowledge (processed observations). It:
//!
//! 1. Receives raw data with a content type
//! 2. Finds all matching processors from the registry
//! 3. Routes data through each matching processor
//! 4. Collects and returns `ProcessedObservation`s
//! 5. Emits events for each observation

use std::collections::HashMap;
use std::sync::Arc;

use tordex_core::processor::{InMemoryProcessorRegistry, ProcessedObservation, ProcessorError, ProcessorRegistry};

/// The Processing Fabric orchestrator.
///
/// Coordinates processor discovery, routing, and observation collection.
pub struct ProcessingFabric {
    registry: Arc<dyn ProcessorRegistry>,
    /// Domain filter — if set, only process content types starting with these prefixes.
    domain_filter: Option<Vec<String>>,
}

impl ProcessingFabric {
    /// Create a new Processing Fabric with the given registry.
    #[must_use]
    pub fn new(registry: Arc<dyn ProcessorRegistry>) -> Self {
        Self {
            registry,
            domain_filter: None,
        }
    }

    /// Create a Processing Fabric with a fresh in-memory registry.
    #[must_use]
    pub fn with_default_registry() -> Self {
        Self::new(Arc::new(InMemoryProcessorRegistry::new()))
    }

    /// Restrict processing to specific content-type domains (e.g. "text/", "image/").
    #[must_use]
    pub fn with_domain_filter(mut self, domains: Vec<String>) -> Self {
        self.domain_filter = Some(domains);
        self
    }

    /// Get a reference to the processor registry.
    #[must_use]
    pub fn registry(&self) -> &Arc<dyn ProcessorRegistry> {
        &self.registry
    }

    /// Process raw data through all matching processors.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the source data
    /// * `data` - Raw bytes to process
    /// * `content_type` - MIME type for routing (e.g. "text/html")
    /// * `metadata` - Key-value metadata passed to each processor
    ///
    /// # Returns
    /// A list of `ProcessedObservation`s from all matching processors.
    pub fn process(
        &self,
        id: &str,
        data: &[u8],
        content_type: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> Vec<ProcessedObservation> {
        let processors = match content_type {
            Some(ct) => {
                // Apply domain filter if set
                if let Some(ref domains) = self.domain_filter {
                    if !domains.iter().any(|d| ct.starts_with(d)) {
                        return Vec::new();
                    }
                }
                self.registry.processors_for(ct)
            }
            None => {
                // No content type — try all processors
                let names = self.registry.list();
                let mut all = Vec::new();
                for name in names {
                    if let Some(p) = self.registry.get(&name) {
                        all.push(p);
                    }
                }
                all
            }
        };

        let mut all_observations = Vec::new();
        for processor in &processors {
            match processor.process(id, data, content_type, metadata.clone()) {
                Ok(observations) => {
                    all_observations.extend(observations);
                }
                Err(e) => {
                    // Emit a processing error observation
                    all_observations.push(
                        ProcessedObservation::new(
                            format!("{id}_error_{}", processor.name()),
                            "processing.error",
                            e.to_string().into_bytes(),
                            "text/plain",
                        )
                        .with_metadata("processor", processor.name())
                        .with_metadata("source_observation", id),
                    );
                }
            }
        }

        all_observations
    }

    /// Process data through a specific named processor.
    ///
    /// Returns `None` if the processor is not registered.
    pub fn process_with(
        &self,
        processor_name: &str,
        id: &str,
        data: &[u8],
        content_type: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> Option<Result<Vec<ProcessedObservation>, ProcessorError>> {
        self.registry
            .get(processor_name)
            .map(|p| p.process(id, data, content_type, metadata))
    }

    /// List all registered processors.
    #[must_use]
    pub fn list_processors(&self) -> Vec<String> {
        self.registry.list()
    }

    /// Get the count of observations produced if data were processed through
    /// matching processors (without actually running them).
    #[must_use]
    pub fn estimated_yield(&self, content_type: Option<&str>) -> usize {
        match content_type {
            Some(ct) => self.registry.processors_for(ct).len(),
            None => self.registry.list().len(),
        }
    }
}

impl Default for ProcessingFabric {
    fn default() -> Self {
        Self::with_default_registry()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTextProcessor;

    impl tordex_core::processor::Processor for MockTextProcessor {
        fn name(&self) -> &str {
            "mock_text"
        }
        fn description(&self) -> &str {
            "mock text processor"
        }
        fn content_types(&self) -> Vec<&str> {
            vec!["text/plain"]
        }
        fn process(
            &self,
            id: &str,
            data: &[u8],
            _ct: Option<&str>,
            _meta: HashMap<String, String>,
        ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
            Ok(vec![ProcessedObservation::new(
                format!("{id}_mock"),
                "mock.output",
                data.to_vec(),
                "text/plain",
            )])
        }
    }

    struct FailingProcessor;

    impl tordex_core::processor::Processor for FailingProcessor {
        fn name(&self) -> &str {
            "failing"
        }
        fn description(&self) -> &str {
            "always fails"
        }
        fn content_types(&self) -> Vec<&str> {
            vec!["text/plain"]
        }
        fn process(
            &self,
            _id: &str,
            _data: &[u8],
            _ct: Option<&str>,
            _meta: HashMap<String, String>,
        ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
            Err(ProcessorError::ProcessingFailed("intentional failure".into()))
        }
    }

    fn test_fabric() -> ProcessingFabric {
        let registry = Arc::new(InMemoryProcessorRegistry::new());
        registry.register(Box::new(MockTextProcessor)).unwrap();
        registry.register(Box::new(FailingProcessor)).unwrap();
        ProcessingFabric::new(registry)
    }

    #[test]
    fn process_routes_to_matching_processors() {
        let fabric = test_fabric();
        let results = fabric.process(
            "obs1",
            b"hello world",
            Some("text/plain"),
            HashMap::new(),
        );
        // MockTextProcessor succeeds, FailingProcessor returns error observation
        let outputs: Vec<_> = results.iter().filter(|o| o.kind == "mock.output").collect();
        assert_eq!(outputs.len(), 1);
        let errors: Vec<_> = results.iter().filter(|o| o.kind == "processing.error").collect();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn process_nonexistent_content_type() {
        let fabric = test_fabric();
        let results = fabric.process(
            "obs2",
            b"data",
            Some("application/json"),
            HashMap::new(),
        );
        assert!(results.is_empty());
    }

    #[test]
    fn process_without_content_type_uses_all() {
        let fabric = test_fabric();
        let results = fabric.process("obs3", b"data", None, HashMap::new());
        assert!(!results.is_empty());
    }

    #[test]
    fn process_with_specific_processor() {
        let fabric = test_fabric();
        let result = fabric.process_with(
            "mock_text",
            "obs4",
            b"test",
            Some("text/plain"),
            HashMap::new(),
        );
        assert!(result.is_some());
        let observations = result.unwrap().unwrap();
        assert_eq!(observations.len(), 1);
    }

    #[test]
    fn process_with_missing_processor_returns_none() {
        let fabric = test_fabric();
        let result = fabric.process_with(
            "nonexistent",
            "obs5",
            b"data",
            None,
            HashMap::new(),
        );
        assert!(result.is_none());
    }

    #[test]
    fn list_registered_processors() {
        let fabric = test_fabric();
        let names = fabric.list_processors();
        assert!(names.contains(&"mock_text".to_string()));
        assert!(names.contains(&"failing".to_string()));
    }

    #[test]
    fn domain_filter_restricts_processing() {
        let registry = Arc::new(InMemoryProcessorRegistry::new());
        registry.register(Box::new(MockTextProcessor)).unwrap();
        let fabric = ProcessingFabric::new(registry)
            .with_domain_filter(vec!["image/".to_string()]);

        // text/plain should be filtered out
        let results = fabric.process("obs6", b"data", Some("text/plain"), HashMap::new());
        assert!(results.is_empty());
    }
}
