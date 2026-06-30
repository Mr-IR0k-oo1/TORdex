use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};
use tordex_repo_intel::RepoIntel;

/// A processor that bridges Repository Intelligence into the observation pipeline.
///
/// Accepts `application/x-repo-intel` content type and performs:
/// - Single-file analysis
/// - Multi-file package analysis
/// - Call graph construction
/// - Control flow graph generation
/// - Full repository report generation
///
/// Use `action` metadata to select the operation:
/// - `"analyze_file"` — analyze a single source file
/// - `"analyze_package"` — analyze multiple files as a package
/// - `"report"` — get the full repository analysis report
/// - `"call_graph"` — get the call graph
/// - `"cfg"` — get control flow graphs
pub struct RepoIntelProcessor {
    intel: std::sync::Mutex<RepoIntel>,
}

impl RepoIntelProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            intel: std::sync::Mutex::new(RepoIntel::new()),
        }
    }
}

impl Default for RepoIntelProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for RepoIntelProcessor {
    fn name(&self) -> &str {
        "RepoIntelProcessor"
    }

    fn description(&self) -> &str {
        "Universal code understanding across 15 languages — symbol extraction, imports, call graphs, control flow graphs, and full repository analysis"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-repo-intel"]
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
            .unwrap_or("analyze_file");

        let json_value: serde_json::Value =
            serde_json::from_slice(data).map_err(|e| {
                ProcessorError::InvalidInput(format!("invalid JSON: {e}"))
            })?;

        let mut intel = self.intel.lock().map_err(|e| {
            ProcessorError::ProcessingFailed(format!("lock error: {e}"))
        })?;

        match action {
            "analyze_file" => {
                let path = json_value["path"]
                    .as_str()
                    .ok_or_else(|| {
                        ProcessorError::InvalidInput(
                            "missing 'path' field".to_string(),
                        )
                    })?;
                let content = json_value["content"]
                    .as_str()
                    .ok_or_else(|| {
                        ProcessorError::InvalidInput(
                            "missing 'content' field".to_string(),
                        )
                    })?;
                let result = intel.analyze_file(path, content);
                match result {
                    Some(fa) => {
                        let output = serde_json::to_value(&fa).unwrap_or_default();
                        Ok(vec![ProcessedObservation::new(
                            id.to_string(),
                            "repo_intel.file_analyzed",
                            serde_json::to_vec(&output).unwrap_or_default(),
                            "application/x-repo-intel+file",
                        )
                        .with_metadata("file", path)
                        .with_metadata(
                            "language",
                            fa.module.language.name(),
                        )
                        .with_metadata(
                            "symbol_count",
                            &fa.module.symbols.len().to_string(),
                        )
                        .with_metadata(
                            "import_count",
                            &fa.module.imports.len().to_string(),
                        )])
                    }
                    None => Err(ProcessorError::InvalidInput(format!(
                        "unable to analyze file: {path} (unsupported language or extension)"
                    ))),
                }
            }
            "analyze_package" => {
                let files = json_value["files"]
                    .as_array()
                    .ok_or_else(|| {
                        ProcessorError::InvalidInput(
                            "missing 'files' array field".to_string(),
                        )
                    })?;
                let file_pairs: Vec<(String, String)> = files
                    .iter()
                    .filter_map(|f| {
                        Some((
                            f["path"].as_str()?.to_string(),
                            f["content"].as_str()?.to_string(),
                        ))
                    })
                    .collect();
                if file_pairs.is_empty() {
                    return Err(ProcessorError::InvalidInput(
                        "'files' array is empty or contains invalid entries".to_string(),
                    ));
                }
                intel.analyze_package(file_pairs);
                let report = intel.report();
                let output = serde_json::to_value(&report).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "repo_intel.package_analyzed",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-repo-intel+package",
                )
                .with_metadata("file_count", &report.file_count.to_string())
                .with_metadata(
                    "total_lines",
                    &report.total_lines.to_string(),
                )
                .with_metadata(
                    "package_count",
                    &report.workspace.packages.len().to_string(),
                )])
            }
            "report" => {
                let report = intel.report();
                let output = serde_json::to_value(&report).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "repo_intel.report",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-repo-intel+report",
                )
                .with_metadata("file_count", &report.file_count.to_string())
                .with_metadata(
                    "total_lines",
                    &report.total_lines.to_string(),
                )
                .with_metadata(
                    "package_count",
                    &report.workspace.packages.len().to_string(),
                )])
            }
            "call_graph" => {
                let report = intel.report();
                let cg = &report.call_graph;
                let output = serde_json::to_value(cg).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "repo_intel.call_graph",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-repo-intel+callgraph",
                )
                .with_metadata("node_count", &cg.node_count().to_string())
                .with_metadata(
                    "edge_count",
                    &cg.edge_count().to_string(),
                )])
            }
            "cfg" => {
                let report = intel.report();
                let output = serde_json::to_value(&report.cfgs).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "repo_intel.cfgs",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-repo-intel+cfg",
                )
                .with_metadata(
                    "cfg_count",
                    &report.cfgs.len().to_string(),
                )])
            }
            _ => Err(ProcessorError::InvalidInput(format!(
                "unknown action: {action}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_processor() -> RepoIntelProcessor {
        RepoIntelProcessor::new()
    }

    #[test]
    fn name_and_content_types() {
        let p = make_processor();
        assert_eq!(p.name(), "RepoIntelProcessor");
        assert!(p.content_types().contains(&"application/x-repo-intel"));
    }

    #[test]
    fn analyze_single_file() {
        let p = make_processor();
        let data = serde_json::json!({
            "path": "hello.rs",
            "content": "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}\", name)\n}\n",
        });
        let results = p
            .process(
                "obs_001",
                &serde_json::to_vec(&data).unwrap(),
                Some("application/x-repo-intel"),
                HashMap::from([("action".into(), "analyze_file".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "repo_intel.file_analyzed");
        assert!(results[0].metadata.contains_key("language"));
        assert_eq!(results[0].metadata.get("language").unwrap(), "Rust");
    }

    #[test]
    fn analyze_package_multi_file() {
        let p = make_processor();
        let data = serde_json::json!({
            "files": [
                {"path": "main.rs", "content": "fn hello() {}"},
                {"path": "main.py", "content": "def hello(): pass"},
            ],
        });
        let results = p
            .process(
                "obs_001",
                &serde_json::to_vec(&data).unwrap(),
                Some("application/x-repo-intel"),
                HashMap::from([("action".into(), "analyze_package".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "repo_intel.package_analyzed");
        assert_eq!(results[0].metadata.get("file_count").unwrap(), "2");
    }

    #[test]
    fn report_after_analysis() {
        let p = make_processor();
        let data = serde_json::json!({
            "path": "lib.rs",
            "content": "pub fn a() {}\npub fn b() {}\n",
        });
        p.process(
            "obs_001",
            &serde_json::to_vec(&data).unwrap(),
            Some("application/x-repo-intel"),
            HashMap::from([("action".into(), "analyze_file".into())]),
        )
        .unwrap();
        let results = p
            .process(
                "obs_002",
                b"{}",
                Some("application/x-repo-intel"),
                HashMap::from([("action".into(), "report".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "repo_intel.report");
        assert_eq!(results[0].metadata.get("file_count").unwrap(), "1");
    }

    #[test]
    fn call_graph_action() {
        let p = make_processor();
        let data = serde_json::json!({
            "path": "main.rs",
            "content": "fn main() {}\nfn helper() {}\n",
        });
        p.process(
            "obs_001",
            &serde_json::to_vec(&data).unwrap(),
            Some("application/x-repo-intel"),
            HashMap::from([("action".into(), "analyze_file".into())]),
        )
        .unwrap();
        let results = p
            .process(
                "obs_002",
                b"{}",
                Some("application/x-repo-intel"),
                HashMap::from([("action".into(), "call_graph".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "repo_intel.call_graph");
    }

    #[test]
    fn missing_path_returns_error() {
        let p = make_processor();
        let data = serde_json::json!({
            "content": "fn main() {}",
        });
        let result = p.process(
            "obs_001",
            &serde_json::to_vec(&data).unwrap(),
            Some("application/x-repo-intel"),
            HashMap::from([("action".into(), "analyze_file".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn unknown_action_returns_error() {
        let p = make_processor();
        let result = p.process(
            "obs_001",
            b"{}",
            Some("application/x-repo-intel"),
            HashMap::from([("action".into(), "bogus".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn invalid_json_returns_error() {
        let p = make_processor();
        let result = p.process(
            "obs_001",
            b"not valid json",
            Some("application/x-repo-intel"),
            HashMap::from([("action".into(), "analyze_file".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn empty_files_array_returns_error() {
        let p = make_processor();
        let data = serde_json::json!({"files": []});
        let result = p.process(
            "obs_001",
            &serde_json::to_vec(&data).unwrap(),
            Some("application/x-repo-intel"),
            HashMap::from([("action".into(), "analyze_package".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn cfg_action_returns_cfgs() {
        let p = make_processor();
        let data = serde_json::json!({
            "path": "main.rs",
            "content": "fn main() { if x {} }",
        });
        p.process(
            "obs_001",
            &serde_json::to_vec(&data).unwrap(),
            Some("application/x-repo-intel"),
            HashMap::from([("action".into(), "analyze_file".into())]),
        )
        .unwrap();
        let results = p
            .process(
                "obs_002",
                b"{}",
                Some("application/x-repo-intel"),
                HashMap::from([("action".into(), "cfg".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "repo_intel.cfgs");
    }
}
