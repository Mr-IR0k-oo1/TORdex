use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for npm package.json manifests.
pub struct NpmParser;

impl ManifestParser for NpmParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Npm
    }

    fn can_parse(&self, path: &str, content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("package.json") && content.contains("\"dependencies\"")
    }

    fn parse(&self, _path: &str, content: &str) -> Option<ParsedManifest> {
        let json: serde_json::Value = serde_json::from_str(content).ok()?;
        let obj = json.as_object()?;

        let package_name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)?;
        let version = obj.get("version").and_then(|v| v.as_str()).map(String::from);

        let mut dependencies = Vec::new();
        if let Some(deps) = obj.get("dependencies").and_then(|v| v.as_object()) {
            for (name, constraint) in deps {
                dependencies.push(ParsedDependency {
                    name: name.clone(),
                    constraint: constraint.as_str().map(String::from),
                    kind: DependencyKind::Runtime,
                });
            }
        }
        if let Some(deps) = obj
            .get("devDependencies")
            .and_then(|v| v.as_object())
        {
            for (name, constraint) in deps {
                dependencies.push(ParsedDependency {
                    name: name.clone(),
                    constraint: constraint.as_str().map(String::from),
                    kind: DependencyKind::Dev,
                });
            }
        }
        if let Some(deps) = obj
            .get("peerDependencies")
            .and_then(|v| v.as_object())
        {
            for (name, constraint) in deps {
                dependencies.push(ParsedDependency {
                    name: name.clone(),
                    constraint: constraint.as_str().map(String::from),
                    kind: DependencyKind::Peer,
                });
            }
        }
        if let Some(deps) = obj
            .get("optionalDependencies")
            .and_then(|v| v.as_object())
        {
            for (name, constraint) in deps {
                dependencies.push(ParsedDependency {
                    name: name.clone(),
                    constraint: constraint.as_str().map(String::from),
                    kind: DependencyKind::Optional,
                });
            }
        }

        Some(ParsedManifest {
            ecosystem: Ecosystem::Npm,
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
    fn parse_basic_package_json() {
        let content = r#"{
            "name": "my-app",
            "version": "1.0.0",
            "dependencies": {
                "express": "^4.18.0",
                "lodash": "~4.17.21"
            },
            "devDependencies": {
                "jest": "^29.0.0"
            }
        }"#;
        let manifest = NpmParser.parse("package.json", content).unwrap();
        assert_eq!(manifest.package_name, "my-app");
        assert_eq!(manifest.version, Some("1.0.0".to_string()));
        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.dependencies[0].name, "express");
        assert_eq!(manifest.dependencies[2].kind, DependencyKind::Dev);
    }

    #[test]
    fn detect_package_json() {
        assert!(NpmParser.can_parse("package.json", r#"{"dependencies":{}}"#));
        assert!(!NpmParser.can_parse("composer.json", "{}"));
    }
}
