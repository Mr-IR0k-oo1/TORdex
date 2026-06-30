use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};
use tordex_package_intel::PackageIntel;

/// A processor that bridges Universal Package Intelligence into the observation pipeline.
///
/// Accepts `application/x-package-intel` content type and understands
/// Cargo, npm, PyPI, NuGet, Go, Composer, Maven, Gradle, APT, Pacman,
/// Docker, and OCI manifests — producing a unified dependency graph.
///
/// Use `action` metadata to select the operation:
/// - `"parse"` — parse a single manifest file and add to dependency graph
/// - `"graph"` — get the full unified dependency graph
/// - `"report"` — get summary report
/// - `"dependents"` — get dependents of a package (needs `name` metadata)
/// - `"dependencies"` — get dependencies of a package (needs `name` metadata)
pub struct PackageIntelProcessor {
    intel: std::sync::Mutex<PackageIntel>,
}

impl PackageIntelProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            intel: std::sync::Mutex::new(PackageIntel::new()),
        }
    }
}

impl Default for PackageIntelProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for PackageIntelProcessor {
    fn name(&self) -> &str {
        "PackageIntelProcessor"
    }

    fn description(&self) -> &str {
        "Universal Package Intelligence — parses manifests from 12 ecosystems (Cargo, npm, PyPI, NuGet, Go, Composer, Maven, Gradle, APT, Pacman, Docker, OCI) into a unified dependency graph"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-package-intel"]
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
            .unwrap_or("parse");

        let json_value: serde_json::Value =
            serde_json::from_slice(data).map_err(|e| {
                ProcessorError::InvalidInput(format!("invalid JSON: {e}"))
            })?;

        let mut intel = self.intel.lock().map_err(|e| {
            ProcessorError::ProcessingFailed(format!("lock error: {e}"))
        })?;

        match action {
            "parse" => {
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
                let result = intel.parse_manifest(path, content);
                match result {
                    Some(manifest) => {
                        let output = serde_json::to_value(&manifest).unwrap_or_default();
                        Ok(vec![ProcessedObservation::new(
                            id.to_string(),
                            "package_intel.parsed",
                            serde_json::to_vec(&output).unwrap_or_default(),
                            "application/x-package-intel+manifest",
                        )
                        .with_metadata("file", path)
                        .with_metadata(
                            "ecosystem",
                            manifest.ecosystem.name(),
                        )
                        .with_metadata(
                            "package_name",
                            &manifest.package_name,
                        )
                        .with_metadata(
                            "dependency_count",
                            &manifest.dependencies.len().to_string(),
                        )])
                    }
                    None => Err(ProcessorError::InvalidInput(format!(
                        "unable to parse manifest: {path} (unsupported format)"
                    ))),
                }
            }
            "graph" => {
                let graph = intel.dependency_graph();
                let output = serde_json::to_value(graph).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "package_intel.graph",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-package-intel+graph",
                )
                .with_metadata("node_count", &graph.node_count().to_string())
                .with_metadata(
                    "edge_count",
                    &graph.edge_count().to_string(),
                )])
            }
            "report" => {
                let report = intel.report();
                let output = serde_json::to_value(&report).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "package_intel.report",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-package-intel+report",
                )
                .with_metadata(
                    "total_packages",
                    &report.total_packages.to_string(),
                )
                .with_metadata(
                    "total_dependencies",
                    &report.total_dependencies.to_string(),
                )])
            }
            "dependents" => {
                let name = metadata.get("name").ok_or_else(|| {
                    ProcessorError::InvalidInput(
                        "missing 'name' metadata for dependents query".to_string(),
                    )
                })?;
                let graph = intel.dependency_graph();
                let ecosystem = metadata.get("ecosystem").map(|s| s.as_str()).unwrap_or("cargo");
                let node_id = format!("{ecosystem}/{name}");
                let deps = graph.dependents_of(&node_id);
                let output: Vec<serde_json::Value> = deps
                    .iter()
                    .map(|n| {
                        serde_json::json!({
                            "id": n.id,
                            "name": n.name,
                            "ecosystem": n.ecosystem.name(),
                        })
                    })
                    .collect();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "package_intel.dependents",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-package-intel+dependents",
                )
                .with_metadata("name", name)
                .with_metadata("count", &deps.len().to_string())])
            }
            "dependencies" => {
                let name = metadata.get("name").ok_or_else(|| {
                    ProcessorError::InvalidInput(
                        "missing 'name' metadata for dependencies query".to_string(),
                    )
                })?;
                let graph = intel.dependency_graph();
                let ecosystem = metadata.get("ecosystem").map(|s| s.as_str()).unwrap_or("cargo");
                let node_id = format!("{ecosystem}/{name}");
                let deps = graph.dependencies_of(&node_id);
                let output: Vec<serde_json::Value> = deps
                    .iter()
                    .map(|n| {
                        serde_json::json!({
                            "id": n.id,
                            "name": n.name,
                            "ecosystem": n.ecosystem.name(),
                        })
                    })
                    .collect();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "package_intel.dependencies",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-package-intel+dependencies",
                )
                .with_metadata("name", name)
                .with_metadata("count", &deps.len().to_string())])
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

    fn make_processor() -> PackageIntelProcessor {
        PackageIntelProcessor::new()
    }

    #[test]
    fn name_and_content_types() {
        let p = make_processor();
        assert_eq!(p.name(), "PackageIntelProcessor");
        assert!(p.content_types().contains(&"application/x-package-intel"));
    }

    #[test]
    fn parse_cargo_manifest() {
        let p = make_processor();
        let data = serde_json::json!({
            "path": "Cargo.toml",
            "content": r#"[package]
name = "my-crate"
version = "1.0.0"

[dependencies]
serde = "1.0"
tokio = { version = "1.35", features = ["full"] }
"#,
        });
        let results = p
            .process(
                "obs_001",
                &serde_json::to_vec(&data).unwrap(),
                Some("application/x-package-intel"),
                HashMap::from([("action".into(), "parse".into())]),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "package_intel.parsed");
        assert_eq!(results[0].metadata.get("ecosystem").unwrap(), "cargo");
        assert_eq!(results[0].metadata.get("package_name").unwrap(), "my-crate");
        assert_eq!(results[0].metadata.get("dependency_count").unwrap(), "2");
    }

    #[test]
    fn parse_npm_manifest() {
        let p = make_processor();
        let data = serde_json::json!({
            "path": "package.json",
            "content": r#"{
                "name": "my-app",
                "version": "1.0.0",
                "dependencies": {
                    "express": "^4.18.0"
                }
            }"#,
        });
        let results = p
            .process(
                "obs_001",
                &serde_json::to_vec(&data).unwrap(),
                Some("application/x-package-intel"),
                HashMap::from([("action".into(), "parse".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "package_intel.parsed");
        assert_eq!(results[0].metadata.get("ecosystem").unwrap(), "npm");
    }

    #[test]
    fn graph_action_after_parses() {
        let p = make_processor();
        // Parse two manifests
        let cargo = serde_json::json!({
            "path": "Cargo.toml",
            "content": "[package]\nname = \"app\"\n\n[dependencies]\nserde = \"1.0\"\n",
        });
        p.process(
            "obs_001",
            &serde_json::to_vec(&cargo).unwrap(),
            Some("application/x-package-intel"),
            HashMap::from([("action".into(), "parse".into())]),
        )
        .unwrap();

        let npm = serde_json::json!({
            "path": "package.json",
            "content": r#"{"name":"web","dependencies":{"react":"^18.0"}}"#,
        });
        p.process(
            "obs_002",
            &serde_json::to_vec(&npm).unwrap(),
            Some("application/x-package-intel"),
            HashMap::from([("action".into(), "parse".into())]),
        )
        .unwrap();

        let results = p
            .process(
                "obs_003",
                b"{}",
                Some("application/x-package-intel"),
                HashMap::from([("action".into(), "graph".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "package_intel.graph");
        let graph_data: serde_json::Value =
            serde_json::from_slice(&results[0].data).unwrap();
        assert_eq!(graph_data["nodes"].as_array().unwrap().len(), 4); // app + serde + web + react
        assert_eq!(graph_data["edges"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn report_action() {
        let p = make_processor();
        let results = p
            .process(
                "obs_001",
                b"{}",
                Some("application/x-package-intel"),
                HashMap::from([("action".into(), "report".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "package_intel.report");
        assert_eq!(results[0].metadata.get("total_packages").unwrap(), "0");
    }

    #[test]
    fn missing_path_returns_error() {
        let p = make_processor();
        let data = serde_json::json!({"content": "irrelevant"});
        let result = p.process(
            "obs_001",
            &serde_json::to_vec(&data).unwrap(),
            Some("application/x-package-intel"),
            HashMap::from([("action".into(), "parse".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn unknown_action_returns_error() {
        let p = make_processor();
        let result = p.process(
            "obs_001",
            b"{}",
            Some("application/x-package-intel"),
            HashMap::from([("action".into(), "bogus".into())]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn dependents_and_dependencies_queries() {
        let p = make_processor();
        let data = serde_json::json!({
            "path": "Cargo.toml",
            "content": "[package]\nname = \"app\"\n\n[dependencies]\nserde = \"1.0\"\n",
        });
        p.process(
            "obs_001",
            &serde_json::to_vec(&data).unwrap(),
            Some("application/x-package-intel"),
            HashMap::from([("action".into(), "parse".into())]),
        )
        .unwrap();

        let deps = p
            .process(
                "obs_002",
                b"{}",
                Some("application/x-package-intel"),
                HashMap::from([
                    ("action".into(), "dependencies".into()),
                    ("name".into(), "app".into()),
                ]),
            )
            .unwrap();
        assert_eq!(deps[0].kind, "package_intel.dependencies");
        assert_eq!(deps[0].metadata.get("count").unwrap(), "1");

        let dependents = p
            .process(
                "obs_003",
                b"{}",
                Some("application/x-package-intel"),
                HashMap::from([
                    ("action".into(), "dependents".into()),
                    ("name".into(), "serde".into()),
                ]),
            )
            .unwrap();
        assert_eq!(dependents[0].kind, "package_intel.dependents");
        assert_eq!(dependents[0].metadata.get("count").unwrap(), "1");
    }
}
