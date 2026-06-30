use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for Composer PHP manifests (composer.json).
pub struct ComposerParser;

impl ManifestParser for ComposerParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Composer
    }

    fn can_parse(&self, path: &str, content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("composer.json") && content.contains("\"require\"")
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
        if let Some(deps) = obj.get("require").and_then(|v| v.as_object()) {
            for (name, constraint) in deps {
                dependencies.push(ParsedDependency {
                    name: name.clone(),
                    constraint: constraint.as_str().map(String::from),
                    kind: DependencyKind::Runtime,
                });
            }
        }
        if let Some(deps) = obj.get("require-dev").and_then(|v| v.as_object()) {
            for (name, constraint) in deps {
                dependencies.push(ParsedDependency {
                    name: name.clone(),
                    constraint: constraint.as_str().map(String::from),
                    kind: DependencyKind::Dev,
                });
            }
        }

        Some(ParsedManifest {
            ecosystem: Ecosystem::Composer,
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
    fn parse_composer_json() {
        let content = r#"{
            "name": "vendor/my-app",
            "require": {
                "php": ">=8.0",
                "monolog/monolog": "^3.0"
            },
            "require-dev": {
                "phpunit/phpunit": "^10.0"
            }
        }"#;
        let manifest = ComposerParser.parse("composer.json", content).unwrap();
        assert_eq!(manifest.package_name, "vendor/my-app");
        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.dependencies[0].name, "monolog/monolog");
        assert_eq!(manifest.dependencies[2].kind, DependencyKind::Dev);
    }

    #[test]
    fn detect_composer() {
        assert!(ComposerParser.can_parse("composer.json", r#"{"require":{}}"#));
        assert!(!ComposerParser.can_parse("package.json", "{}"));
    }
}
