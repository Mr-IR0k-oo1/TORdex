use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for Cargo.toml manifests.
pub struct CargoParser;

impl ManifestParser for CargoParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Cargo
    }

    fn can_parse(&self, path: &str, _content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("cargo.toml")
    }

    fn parse(&self, _path: &str, content: &str) -> Option<ParsedManifest> {
        let lines: Vec<&str> = content.lines().collect();
        let mut package_name = String::new();
        let mut version = None;
        let mut dependencies = Vec::new();
        let mut in_section = String::new();

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_section = trimmed[1..trimmed.len() - 1].to_string();
                continue;
            }
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            match in_section.as_str() {
                "package" => {
                    if let Some(val) = trimmed.strip_prefix("name = ") {
                        package_name = val.trim_matches('"').to_string();
                    } else if let Some(val) = trimmed.strip_prefix("version = ") {
                        version = Some(val.trim_matches('"').to_string());
                    }
                }
                "dependencies" => {
                    if let Some(dep) = parse_toml_dep(trimmed) {
                        dependencies.push(dep);
                    }
                }
                "dev-dependencies" => {
                    if let Some(mut dep) = parse_toml_dep(trimmed) {
                        dep.kind = DependencyKind::Dev;
                        dependencies.push(dep);
                    }
                }
                "build-dependencies" => {
                    if let Some(mut dep) = parse_toml_dep(trimmed) {
                        dep.kind = DependencyKind::Build;
                        dependencies.push(dep);
                    }
                }
                _ => {}
            }
        }

        if package_name.is_empty() {
            return None;
        }

        Some(ParsedManifest {
            ecosystem: Ecosystem::Cargo,
            package_name,
            version,
            dependencies,
        })
    }
}

fn parse_toml_dep(line: &str) -> Option<ParsedDependency> {
    // Simple parsing: `name = "version"` or `name = { version = "...", ... }`
    if let Some(eq_pos) = line.find(" = ") {
        let name = line[..eq_pos].trim().to_string();
        let value = line[eq_pos + 3..].trim();
        if value.starts_with('"') {
            let constraint = value.trim_matches('"').to_string();
            return Some(ParsedDependency {
                name,
                constraint: Some(constraint),
                kind: DependencyKind::Runtime,
            });
        }
        if value.starts_with('{') {
            // Inline table — try to find version
                    if let Some(ver_pos) = value.find("version = \"") {
                                let after_ver = &value[ver_pos + 11..];
                if let Some(end) = after_ver.find('"') {
                    let constraint = after_ver[..end].to_string();
                    return Some(ParsedDependency {
                        name,
                        constraint: Some(constraint),
                        kind: DependencyKind::Runtime,
                    });
                }
            }
            return Some(ParsedDependency {
                name,
                constraint: None,
                kind: DependencyKind::Runtime,
            });
        }
        // Path or git dependency
        if value.starts_with("path=") || value.starts_with("git=") {
            return Some(ParsedDependency {
                name,
                constraint: None,
                kind: DependencyKind::Runtime,
            });
        }
        // Table reference `name = { ... }` with no version
        Some(ParsedDependency {
            name,
            constraint: None,
            kind: DependencyKind::Runtime,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_cargo_toml() {
        let content = r#"[package]
name = "my-crate"
version = "1.0.0"

[dependencies]
serde = "1.0"
tokio = { version = "1.35", features = ["full"] }

[dev-dependencies]
criterion = "0.5"
"#;
        let manifest = CargoParser.parse("Cargo.toml", content).unwrap();
        assert_eq!(manifest.package_name, "my-crate");
        assert_eq!(manifest.version, Some("1.0.0".to_string()));
        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.dependencies[0].name, "serde");
        assert_eq!(manifest.dependencies[0].constraint, Some("1.0".to_string()));
        assert_eq!(manifest.dependencies[1].name, "tokio");
        assert_eq!(manifest.dependencies[1].constraint, Some("1.35".to_string()));
        assert_eq!(manifest.dependencies[2].name, "criterion");
        assert_eq!(manifest.dependencies[2].kind, DependencyKind::Dev);
    }

    #[test]
    fn detect_cargo_toml() {
        assert!(CargoParser.can_parse("Cargo.toml", ""));
        assert!(CargoParser.can_parse("subdir/Cargo.toml", ""));
        assert!(!CargoParser.can_parse("package.json", ""));
    }
}
