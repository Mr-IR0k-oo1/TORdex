use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for Arch Linux PKGBUILD files.
pub struct PacmanParser;

impl ManifestParser for PacmanParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Pacman
    }

    fn can_parse(&self, path: &str, content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("pkgbuild") && content.contains("pkgname=")
    }

    fn parse(&self, _path: &str, content: &str) -> Option<ParsedManifest> {
        let mut package_name = None;
        let mut version = None;
        let mut dependencies = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }

            if let Some(name) = trimmed.strip_prefix("pkgname=") {
                package_name = Some(name.trim().trim_matches('"').trim_matches('\'').to_string());
            } else if let Some(ver) = trimmed.strip_prefix("pkgver=") {
                version = Some(ver.trim().trim_matches('"').trim_matches('\'').to_string());
            } else if let Some(deps_str) = trimmed.strip_prefix("depends=(") {
                for dep in deps_str.trim_end_matches(')').split_whitespace() {
                    let dep = dep.trim_matches('"').trim_matches('\'');
                    if !dep.is_empty() && dep != ")" {
                        let name = dep.split('>').next().unwrap_or(dep).split('<').next().unwrap_or(dep).split('=').next().unwrap_or(dep).trim();
                        dependencies.push(ParsedDependency {
                            name: name.to_string(),
                            constraint: None,
                            kind: DependencyKind::Runtime,
                        });
                    }
                }
            } else if let Some(deps_str) = trimmed.strip_prefix("makedepends=(") {
                for dep in deps_str.trim_end_matches(')').split_whitespace() {
                    let dep = dep.trim_matches('"').trim_matches('\'');
                    if !dep.is_empty() && dep != ")" {
                        let name = dep.split('>').next().unwrap_or(dep).split('<').next().unwrap_or(dep).split('=').next().unwrap_or(dep).trim();
                        dependencies.push(ParsedDependency {
                            name: name.to_string(),
                            constraint: None,
                            kind: DependencyKind::Build,
                        });
                    }
                }
            }
        }

        let package_name = package_name?;
        Some(ParsedManifest {
            ecosystem: Ecosystem::Pacman,
            package_name,
            version,
            dependencies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pkgbuild() {
        let content = r#"
pkgname=my-package
pkgver=1.0.0
depends=(glibc openssl)
makedepends=(cmake gcc)
"#;
        let manifest = PacmanParser.parse("PKGBUILD", content).unwrap();
        assert_eq!(manifest.package_name, "my-package");
        assert_eq!(manifest.version, Some("1.0.0".to_string()));
        assert_eq!(manifest.dependencies.len(), 4);
        assert_eq!(manifest.dependencies[0].name, "glibc");
        assert_eq!(manifest.dependencies[2].kind, DependencyKind::Build);
    }

    #[test]
    fn detect_pacman() {
        assert!(PacmanParser.can_parse("PKGBUILD", "pkgname=test"));
        assert!(!PacmanParser.can_parse("Cargo.toml", ""));
    }
}
