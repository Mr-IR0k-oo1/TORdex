use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for Python package manifests (requirements.txt, setup.py, pyproject.toml).
pub struct PypiParser;

impl ManifestParser for PypiParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::PyPI
    }

    fn can_parse(&self, path: &str, _content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("requirements.txt")
            || lower.ends_with("setup.py")
            || lower.ends_with("pyproject.toml")
    }

    fn parse(&self, path: &str, content: &str) -> Option<ParsedManifest> {
        let lower = path.to_lowercase();
        if lower.ends_with("requirements.txt") {
            parse_requirements(content)
        } else if lower.ends_with("setup.py") {
            parse_setup_py(content)
        } else if lower.ends_with("pyproject.toml") {
            parse_pyproject(content)
        } else {
            None
        }
    }
}

fn parse_requirements(content: &str) -> Option<ParsedManifest> {
    let mut dependencies = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("-r") {
            continue;
        }
        // Strip environment markers after ";"
        let dep_part = trimmed.split(';').next().unwrap_or(trimmed).trim();
        let parts: Vec<&str> = dep_part.splitn(2, &['=', '~', '>', '<', '!'][..]).collect();
        let name = parts[0].trim().to_string();
        let constraint = if parts.len() > 1 {
            Some(dep_part.trim_start_matches(parts[0]).trim().to_string())
        } else {
            None
        };
        if !name.is_empty() {
            dependencies.push(ParsedDependency {
                name,
                constraint,
                kind: DependencyKind::Runtime,
            });
        }
    }
    if dependencies.is_empty() {
        return None;
    }
    Some(ParsedManifest {
        ecosystem: Ecosystem::PyPI,
        package_name: "requirements".to_string(),
        version: None,
        dependencies,
    })
}

fn parse_setup_py(content: &str) -> Option<ParsedManifest> {
    let mut package_name = None;
    let mut version = None;
    let mut dependencies = Vec::new();
    let mut in_install_requires = false;
    let mut bracket_depth = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(name) = trimmed.strip_prefix("name=") {
            package_name = Some(name.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(v) = trimmed.strip_prefix("version=") {
            version = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        }

        if trimmed.contains("install_requires") {
            in_install_requires = true;
            bracket_depth = 0;
        }
        if in_install_requires {
            for ch in trimmed.chars() {
                match ch {
                    '[' | '(' => bracket_depth += 1,
                    ']' | ')' => {
                        bracket_depth -= 1;
                        if bracket_depth <= 0 {
                            in_install_requires = false;
                        }
                    }
                    _ => {}
                }
            }
            // Extract dep names from strings
            if let Some(dep_str) = trimmed.strip_prefix('"').and_then(|s| s.split('"').next()) {
                let dep_name = dep_str
                    .split(&['=', '>', '<', '~', '!', '['][..])
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !dep_name.is_empty() && !dep_name.starts_with('#') {
                    dependencies.push(ParsedDependency {
                        name: dep_name,
                        constraint: None,
                        kind: DependencyKind::Runtime,
                    });
                }
            }
        }
    }

    let package_name = package_name?;
    Some(ParsedManifest {
        ecosystem: Ecosystem::PyPI,
        package_name,
        version,
        dependencies,
    })
}

fn parse_pyproject(content: &str) -> Option<ParsedManifest> {
    let mut package_name = None;
    let mut version = None;
    let mut dependencies = Vec::new();
    let mut in_project = false;
    let mut in_deps = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[project]") {
            in_project = true;
            in_deps = false;
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_project = false;
            in_deps = false;
            continue;
        }
        if !in_project {
            continue;
        }
        if let Some(val) = trimmed.strip_prefix("name = ") {
            package_name = Some(val.trim_matches('"').to_string());
        } else if let Some(val) = trimmed.strip_prefix("version = ") {
            version = Some(val.trim_matches('"').to_string());
        }
        if trimmed == "dependencies = [" || trimmed.starts_with("dependencies=[") {
            in_deps = true;
            continue;
        }
        if in_deps {
            if trimmed == "]" || trimmed == "]," {
                in_deps = false;
                continue;
            }
            let dep_str = trimmed.trim().trim_matches(',');
            let dep_name = dep_str
                .trim_matches('"')
                .split(&['=', '>', '<', '~', '!', ';', '['][..])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !dep_name.is_empty() {
                dependencies.push(ParsedDependency {
                    name: dep_name,
                    constraint: None,
                    kind: DependencyKind::Runtime,
                });
            }
        }
    }

    let package_name = package_name?;
    Some(ParsedManifest {
        ecosystem: Ecosystem::PyPI,
        package_name,
        version,
        dependencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_requirements_txt() {
        let content = "requests==2.31.0\nflask>=2.0\nnumpy\n# comment";
        let manifest = PypiParser.parse("requirements.txt", content).unwrap();
        assert_eq!(manifest.package_name, "requirements");
        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.dependencies[0].name, "requests");
        assert_eq!(manifest.dependencies[0].constraint, Some("==2.31.0".to_string()));
    }

    #[test]
    fn detect_pypi() {
        assert!(PypiParser.can_parse("requirements.txt", ""));
        assert!(PypiParser.can_parse("setup.py", ""));
        assert!(PypiParser.can_parse("pyproject.toml", ""));
        assert!(!PypiParser.can_parse("Cargo.toml", ""));
    }

    #[test]
    fn parse_pyproject_toml() {
        let content = r#"[project]
name = "my-package"
version = "0.1.0"
dependencies = [
    "requests>=2.0",
    "click",
]
"#;
        let manifest = PypiParser.parse("pyproject.toml", content).unwrap();
        assert_eq!(manifest.package_name, "my-package");
        assert_eq!(manifest.version, Some("0.1.0".to_string()));
        assert_eq!(manifest.dependencies.len(), 2);
    }
}
