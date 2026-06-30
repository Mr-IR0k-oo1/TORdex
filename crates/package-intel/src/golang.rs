use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for Go module manifests (go.mod).
pub struct GoParser;

impl ManifestParser for GoParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Go
    }

    fn can_parse(&self, path: &str, _content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("go.mod")
    }

    fn parse(&self, _path: &str, content: &str) -> Option<ParsedManifest> {
        let mut module_name = None;
        let mut version = None;
        let mut dependencies = Vec::new();
        let mut in_require_block = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(name) = trimmed.strip_prefix("module ") {
                module_name = Some(name.trim().to_string());
                continue;
            }
            if let Some(ver) = trimmed.strip_prefix("go ") {
                version = Some(ver.trim().to_string());
                continue;
            }

            if trimmed == "require (" {
                in_require_block = true;
                continue;
            }
            if trimmed == ")" {
                in_require_block = false;
                continue;
            }

            if in_require_block {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    dependencies.push(ParsedDependency {
                        name: parts[0].to_string(),
                        constraint: Some(parts[1].to_string()),
                        kind: DependencyKind::Runtime,
                    });
                }
            }

            // Single-line require
            if let Some(rest) = trimmed.strip_prefix("require ") {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() >= 2 {
                    dependencies.push(ParsedDependency {
                        name: parts[0].to_string(),
                        constraint: Some(parts[1].to_string()),
                        kind: DependencyKind::Runtime,
                    });
                }
            }
        }

        let module_name = module_name?;
        Some(ParsedManifest {
            ecosystem: Ecosystem::Go,
            package_name: module_name,
            version,
            dependencies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_go_mod() {
        let content = r#"module github.com/user/myapp
go 1.21

require (
    github.com/gorilla/mux v1.8.1
    golang.org/x/sync v0.6.0
)

require github.com/google/uuid v1.6.0
"#;
        let manifest = GoParser.parse("go.mod", content).unwrap();
        assert_eq!(manifest.package_name, "github.com/user/myapp");
        assert_eq!(manifest.version, Some("1.21".to_string()));
        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.dependencies[0].name, "github.com/gorilla/mux");
    }

    #[test]
    fn detect_go_mod() {
        assert!(GoParser.can_parse("go.mod", ""));
        assert!(!GoParser.can_parse("Cargo.toml", ""));
    }
}
