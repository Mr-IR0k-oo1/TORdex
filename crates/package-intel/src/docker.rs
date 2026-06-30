use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for Dockerfiles — extracts FROM images as dependencies.
pub struct DockerParser;

impl ManifestParser for DockerParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Docker
    }

    fn can_parse(&self, path: &str, content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("dockerfile")
            || lower.ends_with("dockerfile")
            || content.contains("FROM ")
    }

    fn parse(&self, _path: &str, content: &str) -> Option<ParsedManifest> {
        let mut dependencies = Vec::new();
        let mut has_from = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            // Match: FROM image[:tag][AS name]
            if let Some(rest) = trimmed
                .strip_prefix("FROM ")
                .or_else(|| trimmed.strip_prefix("from "))
            {
                has_from = true;
                let image_spec = rest.split_whitespace().next().unwrap_or(rest).trim();
                let parts: Vec<&str> = image_spec.split(':').collect();
                let name = parts[0].to_string();
                let constraint = parts.get(1).map(|s| s.to_string());
                dependencies.push(ParsedDependency {
                    name,
                    constraint,
                    kind: DependencyKind::Runtime,
                });
            }
        }

        if !has_from {
            return None;
        }

        Some(ParsedManifest {
            ecosystem: Ecosystem::Docker,
            package_name: "Dockerfile".to_string(),
            version: None,
            dependencies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dockerfile() {
        let content = r#"
FROM node:20-alpine AS builder
WORKDIR /app
COPY package.json .
RUN npm install

FROM nginx:1.25-alpine
COPY --from=builder /app/dist /usr/share/nginx/html
"#;
        let manifest = DockerParser.parse("Dockerfile", content).unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].name, "node");
        assert_eq!(manifest.dependencies[0].constraint, Some("20-alpine".to_string()));
        assert_eq!(manifest.dependencies[1].name, "nginx");
    }

    #[test]
    fn detect_dockerfile() {
        assert!(DockerParser.can_parse("Dockerfile", "FROM ubuntu"));
        assert!(!DockerParser.can_parse("Cargo.toml", ""));
    }

    #[test]
    fn no_from_returns_none() {
        let result = DockerParser.parse("Dockerfile", "# just a comment\nRUN echo hi");
        assert!(result.is_none());
    }
}
