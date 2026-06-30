#![forbid(unsafe_code)]
#![allow(clippy::module_name_repetitions)]

pub mod cargo;
pub mod npm;
pub mod pypi;
pub mod nuget;
pub mod golang;
pub mod composer;
pub mod maven;
pub mod gradle;
pub mod apt;
pub mod pacman;
pub mod docker;
pub mod oci;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Supported package ecosystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ecosystem {
    Cargo,
    Npm,
    PyPI,
    NuGet,
    Go,
    Composer,
    Maven,
    Gradle,
    Apt,
    Pacman,
    Docker,
    Oci,
}

impl Ecosystem {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Npm => "npm",
            Self::PyPI => "pypi",
            Self::NuGet => "nuget",
            Self::Go => "go",
            Self::Composer => "composer",
            Self::Maven => "maven",
            Self::Gradle => "gradle",
            Self::Apt => "apt",
            Self::Pacman => "pacman",
            Self::Docker => "docker",
            Self::Oci => "oci",
        }
    }

    /// Detect ecosystem from a filename/path.
    pub fn from_filename(path: &str) -> Option<Self> {
        let lower = path.to_lowercase();
        let name = lower.rsplit('/').next().unwrap_or(&lower);
        match name {
            "cargo.toml" => Some(Self::Cargo),
            "package.json" => Some(Self::Npm),
            "requirements.txt" => Some(Self::PyPI),
            "setup.py" => Some(Self::PyPI),
            "pyproject.toml" => Some(Self::PyPI),
            "packages.config" => Some(Self::NuGet),
            "go.mod" => Some(Self::Go),
            "composer.json" => Some(Self::Composer),
            "pom.xml" => Some(Self::Maven),
            "build.gradle" | "build.gradle.kts" => Some(Self::Gradle),
            "pkgbuild" => Some(Self::Pacman),
            "dockerfile" => Some(Self::Docker),
            _ => {
                if lower.ends_with(".csproj") {
                    Some(Self::NuGet)
                } else if lower.ends_with("/packages") || name == "packages" {
                    Some(Self::Apt)
                } else {
                    None
                }
            }
        }
    }
}

/// Kind of dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyKind {
    Runtime,
    Dev,
    Build,
    Optional,
    Peer,
}

/// A single parsed dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDependency {
    pub name: String,
    pub constraint: Option<String>,
    pub kind: DependencyKind,
}

/// Result of parsing a manifest file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedManifest {
    pub ecosystem: Ecosystem,
    pub package_name: String,
    pub version: Option<String>,
    pub dependencies: Vec<ParsedDependency>,
}

/// A node in the unified dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageNode {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub ecosystem: Ecosystem,
    pub file_path: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// A dependency edge in the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub source_id: String,
    pub target_id: String,
    pub constraint: Option<String>,
    pub kind: DependencyKind,
}

/// The unified dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: Vec<PackageNode>,
    pub edges: Vec<DependencyEdge>,
}

impl DependencyGraph {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: PackageNode) -> String {
        let id = node.id.clone();
        if !self.nodes.iter().any(|n| n.id == id) {
            self.nodes.push(node);
        }
        id
    }

    pub fn add_edge(&mut self, edge: DependencyEdge) {
        if !self
            .edges
            .iter()
            .any(|e| e.source_id == edge.source_id && e.target_id == edge.target_id)
        {
            self.edges.push(edge);
        }
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    #[must_use]
    pub fn nodes_for_ecosystem(&self, ecosystem: Ecosystem) -> Vec<&PackageNode> {
        self.nodes
            .iter()
            .filter(|n| n.ecosystem == ecosystem)
            .collect()
    }

    #[must_use]
    pub fn dependencies_of(&self, node_id: &str) -> Vec<&PackageNode> {
        let targets: std::collections::HashSet<&str> = self
            .edges
            .iter()
            .filter(|e| e.source_id == node_id)
            .map(|e| e.target_id.as_str())
            .collect();
        self.nodes
            .iter()
            .filter(|n| targets.contains(n.id.as_str()))
            .collect()
    }

    #[must_use]
    pub fn dependents_of(&self, node_id: &str) -> Vec<&PackageNode> {
        let sources: std::collections::HashSet<&str> = self
            .edges
            .iter()
            .filter(|e| e.target_id == node_id)
            .map(|e| e.source_id.as_str())
            .collect();
        self.nodes
            .iter()
            .filter(|n| sources.contains(n.id.as_str()))
            .collect()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed manifest from a file, with optional content detection fallback.
pub trait ManifestParser {
    fn ecosystem(&self) -> Ecosystem;
    fn can_parse(&self, path: &str, content: &str) -> bool;
    fn parse(&self, path: &str, content: &str) -> Option<ParsedManifest>;
}

/// The Package Intelligence engine — parses manifests from all ecosystems
/// and builds a unified dependency graph.
pub struct PackageIntel {
    graph: DependencyGraph,
    parsers: Vec<Box<dyn ManifestParser + Send + Sync>>,
}

impl PackageIntel {
    #[must_use]
    pub fn new() -> Self {
        let parsers: Vec<Box<dyn ManifestParser + Send + Sync>> = vec![
            Box::new(cargo::CargoParser),
            Box::new(npm::NpmParser),
            Box::new(pypi::PypiParser),
            Box::new(nuget::NugetParser),
            Box::new(golang::GoParser),
            Box::new(composer::ComposerParser),
            Box::new(maven::MavenParser),
            Box::new(gradle::GradleParser),
            Box::new(apt::AptParser),
            Box::new(pacman::PacmanParser),
            Box::new(docker::DockerParser),
            Box::new(oci::OciParser),
        ];
        Self {
            graph: DependencyGraph::new(),
            parsers,
        }
    }

    /// Parse a manifest file and add it to the dependency graph.
    pub fn parse_manifest(&mut self, path: &str, content: &str) -> Option<ParsedManifest> {
        let parser = self.parsers.iter().find(|p| p.can_parse(path, content))?;
        let manifest = parser.parse(path, content)?;

        let node_id = format!(
            "{}/{}",
            manifest.ecosystem.name(),
            manifest.package_name
        );
        let mut metadata = HashMap::new();
        if let Some(ref ver) = manifest.version {
            metadata.insert("version".to_string(), ver.clone());
        }
        metadata.insert("file".to_string(), path.to_string());

        self.graph.add_node(PackageNode {
            id: node_id.clone(),
            name: manifest.package_name.clone(),
            version: manifest.version.clone(),
            ecosystem: manifest.ecosystem,
            file_path: Some(path.to_string()),
            metadata,
        });

        for dep in &manifest.dependencies {
            let dep_id = format!("{}/{}", manifest.ecosystem.name(), dep.name);
            self.graph.add_node(PackageNode {
                id: dep_id.clone(),
                name: dep.name.clone(),
                version: None,
                ecosystem: manifest.ecosystem,
                file_path: None,
                metadata: HashMap::new(),
            });
            self.graph.add_edge(DependencyEdge {
                source_id: node_id.clone(),
                target_id: dep_id,
                constraint: dep.constraint.clone(),
                kind: dep.kind,
            });
        }

        Some(manifest)
    }

    #[must_use]
    pub fn dependency_graph(&self) -> &DependencyGraph {
        &self.graph
    }

    #[must_use]
    pub fn report(&self) -> UnifiedReport {
        let mut by_eco: std::collections::HashMap<Ecosystem, usize> =
            std::collections::HashMap::new();
        for node in &self.graph.nodes {
            *by_eco.entry(node.ecosystem).or_insert(0) += 1;
        }
        UnifiedReport {
            total_packages: self.graph.node_count(),
            total_dependencies: self.graph.edge_count(),
            ecosystems: by_eco,
        }
    }
}

impl Default for PackageIntel {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary report of the unified dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedReport {
    pub total_packages: usize,
    pub total_dependencies: usize,
    pub ecosystems: std::collections::HashMap<Ecosystem, usize>,
}

/// Generate a content-based fingerprint for a manifest.
pub fn fingerprint_manifest(path: &str, content: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(path.as_bytes());
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

use sha2::Digest;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_ecosystem_from_filename() {
        assert_eq!(Ecosystem::from_filename("Cargo.toml"), Some(Ecosystem::Cargo));
        assert_eq!(Ecosystem::from_filename("package.json"), Some(Ecosystem::Npm));
        assert_eq!(Ecosystem::from_filename("requirements.txt"), Some(Ecosystem::PyPI));
        assert_eq!(Ecosystem::from_filename("setup.py"), Some(Ecosystem::PyPI));
        assert_eq!(Ecosystem::from_filename("pyproject.toml"), Some(Ecosystem::PyPI));
        assert_eq!(Ecosystem::from_filename("packages.config"), Some(Ecosystem::NuGet));
        assert_eq!(Ecosystem::from_filename("project.csproj"), Some(Ecosystem::NuGet));
        assert_eq!(Ecosystem::from_filename("go.mod"), Some(Ecosystem::Go));
        assert_eq!(Ecosystem::from_filename("composer.json"), Some(Ecosystem::Composer));
        assert_eq!(Ecosystem::from_filename("pom.xml"), Some(Ecosystem::Maven));
        assert_eq!(Ecosystem::from_filename("build.gradle"), Some(Ecosystem::Gradle));
        assert_eq!(Ecosystem::from_filename("PKGBUILD"), Some(Ecosystem::Pacman));
        assert_eq!(Ecosystem::from_filename("Dockerfile"), Some(Ecosystem::Docker));
        assert!(Ecosystem::from_filename("readme.txt").is_none());
    }

    #[test]
    fn empty_graph() {
        let intel = PackageIntel::new();
        let report = intel.report();
        assert_eq!(report.total_packages, 0);
        assert_eq!(report.total_dependencies, 0);
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let fp1 = fingerprint_manifest("Cargo.toml", "[package]\nname = \"test\"");
        let fp2 = fingerprint_manifest("Cargo.toml", "[package]\nname = \"test\"");
        assert_eq!(fp1, fp2);
        let fp3 = fingerprint_manifest("Cargo.toml", "[package]\nname = \"other\"");
        assert_ne!(fp1, fp3);
    }

    #[test]
    fn graph_dedup_nodes() {
        let mut intel = PackageIntel::new();
        intel.parse_manifest("Cargo.toml", r#"[package]
name = "my-crate"
version = "1.0.0"

[dependencies]
serde = "1.0"
"#);
        intel.parse_manifest("other/Cargo.toml", r#"[package]
name = "my-crate"
version = "2.0.0"

[dependencies]
serde = "1.0"
tokio = "1.0"
"#);
        let graph = intel.dependency_graph();
        // my-crate + serde + tokio = 3 unique nodes
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
    }
}
