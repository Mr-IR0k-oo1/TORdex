use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for Debian/APT Packages files.
pub struct AptParser;

impl ManifestParser for AptParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Apt
    }

    fn can_parse(&self, path: &str, content: &str) -> bool {
        let lower = path.to_lowercase();
        (lower.ends_with("/packages") || lower.ends_with("packages"))
            && content.contains("Package:")
            && content.contains("Version:")
    }

    fn parse(&self, _path: &str, content: &str) -> Option<ParsedManifest> {
        let mut dependencies = Vec::new();
        let mut current_package = String::new();
        let mut in_entry = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // End of a package entry — add it
                if !current_package.is_empty() {
                    current_package.clear();
                }
                in_entry = false;
                continue;
            }

            if let Some(name) = trimmed.strip_prefix("Package: ") {
                current_package = name.to_string();
                in_entry = true;
                continue;
            }
            if in_entry {
                if let Some(deps) = trimmed.strip_prefix("Depends: ") {
                    for dep_name in deps.split(',') {
                        let dep_name = dep_name.trim();
                        // Split off version constraint at first paren
                        let name = dep_name.split('(').next().unwrap_or(dep_name).trim();
                        if !name.is_empty() {
                            dependencies.push(ParsedDependency {
                                name: name.to_string(),
                                constraint: None,
                                kind: DependencyKind::Runtime,
                            });
                        }
                    }
                }
            }
        }

        if dependencies.is_empty() {
            return None;
        }

        Some(ParsedManifest {
            ecosystem: Ecosystem::Apt,
            package_name: "apt-packages".to_string(),
            version: None,
            dependencies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_apt_packages() {
        let content = r#"Package: curl
Version: 8.0.0
Depends: libc6 (>= 2.34), libssl3 (>= 3.0)

Package: git
Version: 2.40.0
Depends: libc6, zlib1g
"#;
        let manifest = AptParser.parse("Packages", content).unwrap();
        assert_eq!(manifest.dependencies.len(), 4);
        assert_eq!(manifest.dependencies[0].name, "libc6");
    }

    #[test]
    fn detect_apt() {
        assert!(AptParser.can_parse("/var/lib/apt/lists/Packages", "Package: foo\nVersion: 1.0"));
        assert!(!AptParser.can_parse("Cargo.toml", ""));
    }
}
