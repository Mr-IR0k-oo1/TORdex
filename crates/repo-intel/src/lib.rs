#![forbid(unsafe_code)]
#![allow(clippy::module_name_repetitions)]

//! # TORdex Repository Intelligence
//!
//! Universal code understanding across 15 programming languages.
//!
//! ## Architecture
//!
//! Every repository is analyzed into a hierarchy:
//!
//! - **Workspace** — the repository root
//!   - **Package** — a language-specific package (Cargo crate, Python package, etc.)
//!     - **Module** — a single source file
//!       - **Symbols** — functions, classes, structs, interfaces, etc.
//!       - **Imports** — cross-module dependencies
//!       - **CFG** — control flow graph per function
//!       - **Call Graph** — cross-function call relationships
//!
//! ## Supported Languages
//!
//! Rust, Python, Go, Java, C, C++, C#, JavaScript, TypeScript, PHP,
//! Kotlin, Swift, Zig, Lua, WASM

pub mod analysis;
pub mod language;
pub mod parser;

pub use analysis::{BasicBlock, BlockKind, CallGraph, CallNode, ControlFlowGraph};
pub use language::Language;
pub use parser::{
    CodeLocation, CodeSymbol, Import, Module, Package, SourceAnalyzer, SymbolKind, Visibility,
    Workspace,
};

use serde::{Deserialize, Serialize};

/// Full analysis result for a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysis {
    pub module: Module,
    pub cfg: Option<ControlFlowGraph>,
}

/// Complete analysis of a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryAnalysis {
    /// The workspace-level hierarchy.
    pub workspace: Workspace,
    /// Per-function control flow graphs.
    pub cfgs: Vec<ControlFlowGraph>,
    /// The full call graph.
    pub call_graph: CallGraph,
    /// Total lines of code.
    pub total_lines: usize,
    /// Total file count.
    pub file_count: usize,
}

/// Repository intelligence engine — analyzes source code repositories
/// and produces structured knowledge.
#[derive(Debug)]
pub struct RepoIntel {
    pub(crate) workspace: Workspace,
    pub(crate) call_graph: CallGraph,
    pub(crate) cfgs: Vec<ControlFlowGraph>,
}

impl RepoIntel {
    /// Create a new empty repository intelligence engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            workspace: Workspace {
                root: String::new(),
                packages: Vec::new(),
            },
            call_graph: CallGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
            },
            cfgs: Vec::new(),
        }
    }

    /// Analyze a single file and add it to the workspace.
    pub fn analyze_file(&mut self, path: &str, content: &str) -> Option<FileAnalysis> {
        let module = SourceAnalyzer::analyze_file(path, content)?;
        let file_analysis = FileAnalysis {
            module: module.clone(),
            cfg: None,
        };

        let line_count = module.line_count;
        // Place the module in the workspace
        self.workspace.packages.push(Package {
            name: module.path.rsplit('/').next().unwrap_or(&module.path).to_string(),
            language: module.language,
            modules: vec![module.clone()],
            total_lines: line_count,
        });

        // Rebuild call graph from all modules
        let all_modules: Vec<parser::Module> = self
            .workspace
            .packages
            .iter()
            .flat_map(|p| p.modules.clone())
            .collect();
        self.call_graph = CallGraph::build(&all_modules);

        Some(file_analysis)
    }

    /// Analyze a workspace package structure from multiple files.
    pub fn analyze_package(&mut self, files: Vec<(String, String)>) {
        let mut modules = Vec::new();
        for (path, content) in &files {
            if let Some(module) = SourceAnalyzer::analyze_file(path, content) {
                modules.push(module);
            }
        }
        // Build call graph from all modules
        self.call_graph = CallGraph::build(&modules);

        // Group modules by language into packages
        let mut by_lang: std::collections::HashMap<Language, Vec<parser::Module>> =
            std::collections::HashMap::new();
        for m in modules {
            by_lang.entry(m.language).or_default().push(m);
        }

        self.workspace = Workspace {
            root: "workspace".to_string(),
            packages: by_lang
                .into_iter()
                .map(|(lang, mods)| {
                    let total_lines: usize = mods.iter().map(|m| m.line_count).sum();
                    Package {
                        name: format!("{}-package", lang.name().to_lowercase()),
                        language: lang,
                        modules: mods,
                        total_lines,
                    }
                })
                .collect(),
        };
    }

    /// Get the full repository analysis result.
    #[must_use]
    pub fn report(&self) -> RepositoryAnalysis {
        let total_lines: usize = self
            .workspace
            .packages
            .iter()
            .map(|p| p.total_lines)
            .sum();
        let file_count: usize = self
            .workspace
            .packages
            .iter()
            .map(|p| p.modules.len())
            .sum();
        RepositoryAnalysis {
            workspace: self.workspace.clone(),
            cfgs: self.cfgs.clone(),
            call_graph: self.call_graph.clone(),
            total_lines,
            file_count,
        }
    }
}

impl Default for RepoIntel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_single_file() {
        let mut intel = RepoIntel::new();
        let result = intel.analyze_file(
            "hello.rs",
            "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}\", name)\n}\n",
        );
        assert!(result.is_some());
        let fa = result.unwrap();
        assert_eq!(fa.module.language, Language::Rust);
        assert_eq!(fa.module.symbols.len(), 1);
        assert_eq!(fa.module.symbols[0].name, "greet");
    }

    #[test]
    fn analyze_package_multi_language() {
        let mut intel = RepoIntel::new();
        let files = vec![
            ("main.rs".to_string(), "fn hello() {}".to_string()),
            ("main.py".to_string(), "def hello(): pass".to_string()),
            ("main.go".to_string(), "package main\nfunc main() {}".to_string()),
        ];
        intel.analyze_package(files);
        let report = intel.report();
        assert_eq!(report.file_count, 3);
        assert!(report.total_lines > 0);
    }

    #[test]
    fn report_produces_summary() {
        let mut intel = RepoIntel::new();
        intel.analyze_file("lib.rs", "pub fn a() {}\npub fn b() {}\n").unwrap();
        intel.analyze_file("utils.rs", "pub fn helper() {}\n").unwrap();
        let report = intel.report();
        assert_eq!(report.file_count, 2);
    }

    #[test]
    fn empty_engine_produces_empty_report() {
        let intel = RepoIntel::new();
        let report = intel.report();
        assert_eq!(report.file_count, 0);
        assert_eq!(report.total_lines, 0);
    }

    #[test]
    fn unknown_file_ignored() {
        let mut intel = RepoIntel::new();
        let result = intel.analyze_file("readme.txt", "some text");
        assert!(result.is_none());
    }

    #[test]
    fn multi_language_workspace() {
        let mut intel = RepoIntel::new();
        let files = vec![
            ("mod.rs".to_string(), "pub struct Config;".to_string()),
            ("app.ts".to_string(), "interface User {\n  name: string;\n}".to_string()),
        ];
        intel.analyze_package(files);
        let report = intel.report();
        assert_eq!(report.workspace.packages.len(), 2);
    }

    #[test]
    fn call_graph_available_in_report() {
        let mut intel = RepoIntel::new();
        intel.analyze_file("main.rs", "fn main() {}\nfn helper() {}\n").unwrap();
        let report = intel.report();
        assert_eq!(report.call_graph.node_count(), 2);
    }
}
