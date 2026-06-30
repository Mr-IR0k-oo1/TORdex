use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for Maven pom.xml manifests.
pub struct MavenParser;

impl ManifestParser for MavenParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Maven
    }

    fn can_parse(&self, path: &str, _content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("pom.xml")
    }

    fn parse(&self, _path: &str, content: &str) -> Option<ParsedManifest> {
        let lines: Vec<&str> = content.lines().collect();
        let text = content;

        let group_id = extract_xml_tag(text, "groupId");
        let artifact_id = extract_xml_tag(text, "artifactId")?;
        let version = extract_xml_tag(text, "version");

        let package_name = if let Some(ref gid) = group_id {
            format!("{gid}:{artifact_id}")
        } else {
            artifact_id.clone()
        };

        let mut dependencies = Vec::new();
        let mut in_dependency = false;
        let mut dep_group = String::new();
        let mut dep_artifact = String::new();
        let mut dep_version: Option<String> = None;
        let mut dep_scope = String::new();

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.contains("<dependency>") {
                in_dependency = true;
                dep_group.clear();
                dep_artifact.clear();
                dep_version = None;
                dep_scope.clear();
                continue;
            }
            if trimmed.contains("</dependency>") && in_dependency {
                if !dep_artifact.is_empty() {
                    let dep_name = if dep_group.is_empty() {
                        dep_artifact.clone()
                    } else {
                        format!("{}:{}", dep_group, dep_artifact)
                    };
                    let kind = match dep_scope.as_str() {
                        "test" => DependencyKind::Dev,
                        "provided" => DependencyKind::Build,
                        _ => DependencyKind::Runtime,
                    };
                    dependencies.push(ParsedDependency {
                        name: dep_name,
                        constraint: dep_version.clone(),
                        kind,
                    });
                }
                in_dependency = false;
                continue;
            }
            if in_dependency {
                if let Some(val) = strip_xml_tag(trimmed, "groupId") {
                    dep_group = val;
                } else if let Some(val) = strip_xml_tag(trimmed, "artifactId") {
                    dep_artifact = val;
                } else if let Some(val) = strip_xml_tag(trimmed, "version") {
                    dep_version = Some(val);
                } else if let Some(val) = strip_xml_tag(trimmed, "scope") {
                    dep_scope = val;
                }
            }
        }

        Some(ParsedManifest {
            ecosystem: Ecosystem::Maven,
            package_name,
            version,
            dependencies,
        })
    }
}

fn extract_xml_tag(content: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let pos = content.find(&open)?;
    let after = &content[pos + open.len()..];
    let end = after.find(&close)?;
    Some(after[..end].to_string())
}

fn strip_xml_tag(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let Some(pos) = line.find(&open) {
        let after = &line[pos + open.len()..];
        if let Some(end) = after.find(&close) {
            return Some(after[..end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pom_xml() {
        let content = r#"
<project>
    <groupId>com.example</groupId>
    <artifactId>my-service</artifactId>
    <version>1.0.0</version>
    <dependencies>
        <dependency>
            <groupId>org.springframework</groupId>
            <artifactId>spring-core</artifactId>
            <version>6.0.0</version>
        </dependency>
        <dependency>
            <groupId>junit</groupId>
            <artifactId>junit</artifactId>
            <version>5.0.0</version>
            <scope>test</scope>
        </dependency>
    </dependencies>
</project>
"#;
        let manifest = MavenParser.parse("pom.xml", content).unwrap();
        assert_eq!(manifest.package_name, "com.example:my-service");
        assert_eq!(manifest.version, Some("1.0.0".to_string()));
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].name, "org.springframework:spring-core");
        assert_eq!(manifest.dependencies[1].kind, DependencyKind::Dev);
    }

    #[test]
    fn detect_maven() {
        assert!(MavenParser.can_parse("pom.xml", ""));
        assert!(!MavenParser.can_parse("Cargo.toml", ""));
    }
}
