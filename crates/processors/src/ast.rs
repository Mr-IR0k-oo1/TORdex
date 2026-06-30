//! AST processor — detects programming languages, extracts imports, functions,
//! and structural metadata from source code.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

// Simple keyword-based language detection
const LANGUAGES: &[(&str, &[&str])] = &[
    ("rust", &["fn ", "let mut ", "use ", "impl ", "struct ", "enum ", "pub ", "crate::"]),
    ("python", &["def ", "import ", "class ", "async def", "if __name__", "self", "None"]),
    ("javascript", &["function", "const ", "let ", "var ", "import ", "export ", "=>"]),
    ("typescript", &[": string", ": number", ": boolean", "interface ", "type ", "<T>"]),
    ("go", &["func ", "package ", "import (", "defer ", "go ", "chan "]),
    ("java", &["public class", "private ", "protected ", "import java.", "@Override", "void "]),
    ("c", &["#include", "int main(", "void ", "printf(", "malloc(", "struct "]),
    ("cpp", &["#include", "int main(", "std::", "template<", "class ", "virtual "]),
    ("ruby", &["def ", "class ", "end", "require ", "attr_", "gem "]),
    ("php", &["<?php", "function ", "echo ", "$", "namespace "]),
    ("swift", &["func ", "import ", "class ", "struct ", "var ", "let "]),
    ("kotlin", &["fun ", "val ", "var ", "class ", "import ", "package "]),
    ("scala", &["def ", "val ", "var ", "object ", "class ", "import "]),
    ("haskell", &["main =", "::", "->", "where", "data ", "module "]),
    ("lua", &["function ", "local ", "end", "require ", "::"]),
    ("shell", &["#!/bin", "#!/usr", "#!/bash", "export ", "echo \"", "if [", "then", "fi", "done"]),
    ("sql", &["SELECT", "FROM ", "WHERE", "INSERT", "CREATE TABLE", "ALTER TABLE"]),
    ("yaml", &["---", ":", "  - "]),
    ("toml", &["[package]", "[dependencies]", "edition ="]),
    ("dockerfile", &["FROM ", "RUN ", "CMD ", "COPY ", "WORKDIR ", "EXPOSE "]),
];

pub struct AstProcessor;

impl AstProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn detect_language(&self, source: &str) -> Option<&'static str> {
        let mut best: Option<(&'static str, usize)> = None;
        for (lang, keywords) in LANGUAGES {
            let count = keywords.iter().filter(|k| source.contains(*k)).count();
            if count > 0 && best.map_or(true, |(_, b)| count > b) {
                best = Some((lang, count));
            }
        }
        best.map(|(l, _)| l)
    }

    fn extract_imports(&self, source: &str, lang: &str) -> Vec<String> {
        match lang {
            "rust" => {
                let mut imports = Vec::new();
                for line in source.lines() {
                    let line = line.trim();
                    if line.starts_with("use ") && line.ends_with(';') {
                        imports.push(line[4..line.len() - 1].to_string());
                    }
                }
                imports
            }
            "python" => {
                let mut imports = Vec::new();
                for line in source.lines() {
                    let line = line.trim();
                    if line.starts_with("import ") {
                        imports.push(line[7..].to_string());
                    } else if line.starts_with("from ") && line.contains(" import ") {
                        imports.push(line.to_string());
                    }
                }
                imports
            }
            "javascript" | "typescript" => {
                let mut imports = Vec::new();
                for line in source.lines() {
                    let line = line.trim();
                    if line.starts_with("import ") || line.starts_with("const ") && line.contains("require(") {
                        imports.push(line.to_string());
                    }
                }
                imports
            }
            "go" => {
                let mut imports = Vec::new();
                let mut in_block = false;
                for line in source.lines() {
                    let line = line.trim();
                    if line.starts_with("import (") {
                        in_block = true;
                    } else if in_block && line == ")" {
                        in_block = false;
                    } else if in_block {
                        imports.push(line.trim_matches('"').to_string());
                    } else if line.starts_with("import ") && !line.contains('(') {
                        imports.push(line[7..].trim_matches('"').to_string());
                    }
                }
                imports
            }
            _ => Vec::new(),
        }
    }

    fn count_functions(&self, source: &str, lang: &str) -> usize {
        match lang {
            "rust" | "swift" | "kotlin" | "scala" => {
                source.lines().filter(|l| l.trim().starts_with("fn ") || l.trim().starts_with("func ") || l.trim().starts_with("fun ")).count()
            }
            "python" | "ruby" => {
                source.lines().filter(|l| l.trim().starts_with("def ")).count()
            }
            "javascript" | "typescript" => {
                source.lines().filter(|l| l.trim().starts_with("function ")).count()
            }
            "go" => {
                source.lines().filter(|l| l.trim().starts_with("func ")).count()
            }
            "java" => {
                source.lines().filter(|l| {
                    let t = l.trim();
                    t.starts_with("public ") || t.starts_with("private ") || t.starts_with("protected ")
                }).count()
            }
            _ => 0,
        }
    }
}

impl Default for AstProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for AstProcessor {
    fn name(&self) -> &str {
        "ast"
    }

    fn description(&self) -> &str {
        "Detects programming languages, extracts imports, symbols, and structural metadata"
    }

    fn content_types(&self) -> Vec<&str> {
        vec![
            "text/x-rust", "text/x-python", "text/javascript", "text/typescript",
            "text/x-go", "text/x-java", "text/x-c", "text/x-cpp", "text/x-ruby",
            "text/x-php", "text/x-swift", "text/x-kotlin", "text/x-scala",
            "text/x-haskell", "text/x-lua", "text/x-sh", "text/x-sql",
            "text/x-yaml", "text/x-toml", "text/x-dockerfile",
            "application/x-rust", "application/x-python",
            "text/plain",
        ]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let source =
            std::str::from_utf8(data).map_err(|e| ProcessorError::InvalidInput(e.to_string()))?;

        let lang = self.detect_language(source);
        let lang_name = lang.unwrap_or("unknown");
        let mut results = Vec::new();

        // Language detection
        results.push(
            ProcessedObservation::new(
                format!("{id}_language"),
                "ast.language",
                lang_name.as_bytes().to_vec(),
                "text/plain",
            )
            .with_metadata("language", lang_name)
            .with_metadata("source_observation", id),
        );

        // Line count
        let line_count = source.lines().count();
        results.push(
            ProcessedObservation::new(
                format!("{id}_lines"),
                "ast.metadata",
                line_count.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "line_count")
            .with_metadata("value", &line_count.to_string())
            .with_metadata("source_observation", id),
        );

        // Byte size
        let byte_size = data.len();
        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "ast.metadata",
                byte_size.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &byte_size.to_string())
            .with_metadata("source_observation", id),
        );

        // Imports
        if let Some(lang_name) = lang {
            let imports = self.extract_imports(source, lang_name);
            if !imports.is_empty() {
                let imports_json = serde_json::to_string(&imports).unwrap_or_default();
                results.push(
                    ProcessedObservation::new(
                        format!("{id}_imports"),
                        "ast.imports",
                        imports_json.into_bytes(),
                        "application/json",
                    )
                    .with_metadata("language", lang_name)
                    .with_metadata("import_count", &imports.len().to_string())
                    .with_metadata("source_observation", id),
                );
            }

            // Function count
            let func_count = self.count_functions(source, lang_name);
            if func_count > 0 {
                results.push(
                    ProcessedObservation::new(
                        format!("{id}_functions"),
                        "ast.metadata",
                        func_count.to_string().into_bytes(),
                        "text/plain",
                    )
                    .with_metadata("metric", "function_count")
                    .with_metadata("value", &func_count.to_string())
                    .with_metadata("language", lang_name)
                    .with_metadata("source_observation", id),
                );
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_rust() {
        let proc = AstProcessor::new();
        let source = b"fn main() {\n    let x = 1;\n    println!(\"hello\");\n}";
        let results = proc.process("a1", source, Some("text/x-rust"), HashMap::new()).unwrap();
        let langs: Vec<_> = results.iter().filter(|o| o.kind == "ast.language").collect();
        assert!(!langs.is_empty());
        assert_eq!(std::str::from_utf8(&langs[0].data).unwrap(), "rust");
    }

    #[test]
    fn detect_python() {
        let proc = AstProcessor::new();
        let source = b"def hello():\n    print('world')\n\nif __name__ == '__main__':\n    hello()";
        let results = proc.process("a2", source, Some("text/x-python"), HashMap::new()).unwrap();
        let langs: Vec<_> = results.iter().filter(|o| o.kind == "ast.language").collect();
        assert!(!langs.is_empty());
        assert_eq!(std::str::from_utf8(&langs[0].data).unwrap(), "python");
    }

    #[test]
    fn extract_rust_imports() {
        let proc = AstProcessor::new();
        let source = b"use std::collections::HashMap;\nuse serde::Serialize;\n\nfn main() {}";
        let results = proc.process("a3", source, Some("text/x-rust"), HashMap::new()).unwrap();
        let imports: Vec<_> = results.iter().filter(|o| o.kind == "ast.imports").collect();
        assert_eq!(imports.len(), 1);
        let json_str = std::str::from_utf8(&imports[0].data).unwrap();
        assert!(json_str.contains("std::collections::HashMap"));
    }

    #[test]
    fn extract_python_imports() {
        let proc = AstProcessor::new();
        let source = b"import os\nimport sys\nfrom datetime import datetime\n\ndef main(): pass";
        let results = proc.process("a4", source, Some("text/x-python"), HashMap::new()).unwrap();
        let imports: Vec<_> = results.iter().filter(|o| o.kind == "ast.imports").collect();
        assert_eq!(imports.len(), 1);
    }

    #[test]
    fn metadata_always_present() {
        let proc = AstProcessor::new();
        let source = b"some text content\nwith multiple lines\n";
        let results = proc.process("a5", source, Some("text/plain"), HashMap::new()).unwrap();
        let metadatas: Vec<_> = results.iter().filter(|o| o.kind == "ast.metadata").collect();
        assert!(metadatas.len() >= 2); // line count + byte size
    }
}
