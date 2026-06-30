use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for OCI image manifests.
pub struct OciParser;

impl ManifestParser for OciParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Oci
    }

    fn can_parse(&self, path: &str, content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("index.json")
            || content.contains("application/vnd.oci.image.manifest.v1+json")
            || content.contains("application/vnd.oci.image.index.v1+json")
    }

    fn parse(&self, _path: &str, content: &str) -> Option<ParsedManifest> {
        let json: serde_json::Value = serde_json::from_str(content).ok()?;
        let obj = json.as_object()?;

        let media_type = obj
            .get("mediaType")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let package_name = if media_type.contains("index") {
            "oci-index".to_string()
        } else {
            "oci-manifest".to_string()
        };

        let mut dependencies = Vec::new();

        // Extract config reference
        if let Some(config) = obj.get("config").and_then(|v| v.as_object()) {
            if let Some(digest) = config.get("digest").and_then(|v| v.as_str()) {
                dependencies.push(ParsedDependency {
                    name: format!("config:{}", &digest[..std::cmp::min(16, digest.len())]),
                    constraint: Some(digest.to_string()),
                    kind: DependencyKind::Runtime,
                });
            }
        }

        // Extract layer references
        if let Some(layers) = obj.get("layers").and_then(|v| v.as_array()) {
            for layer in layers {
                if let Some(layer_obj) = layer.as_object() {
                    if let Some(digest) = layer_obj.get("digest").and_then(|v| v.as_str()) {
                        let media = layer_obj
                            .get("mediaType")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        dependencies.push(ParsedDependency {
                            name: format!("layer:{}", &digest[..std::cmp::min(16, digest.len())]),
                            constraint: Some(digest.to_string()),
                            kind: if media.contains("foreign") {
                                DependencyKind::Optional
                            } else {
                                DependencyKind::Runtime
                            },
                        });
                    }
                }
            }
        }

        // For index manifests, extract manifests
        if let Some(manifests) = obj.get("manifests").and_then(|v| v.as_array()) {
            for manifest in manifests {
                if let Some(m_obj) = manifest.as_object() {
                    if let Some(digest) = m_obj.get("digest").and_then(|v| v.as_str()) {
                        let platform = m_obj
                            .get("platform")
                            .and_then(|p| p.as_object())
                            .map(|p| {
                                format!(
                                    "{}/{}",
                                    p.get("architecture")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown"),
                                    p.get("os").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                )
                            })
                            .unwrap_or_else(|| "unknown".to_string());
                        dependencies.push(ParsedDependency {
                            name: format!("manifest:{}", platform),
                            constraint: Some(digest.to_string()),
                            kind: DependencyKind::Runtime,
                        });
                    }
                }
            }
        }

        Some(ParsedManifest {
            ecosystem: Ecosystem::Oci,
            package_name,
            version: None,
            dependencies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_oci_manifest() {
        let content = r#"{
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
                "size": 1234
            },
            "layers": [
                {
                    "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                    "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                    "size": 5678
                }
            ]
        }"#;
        let manifest = OciParser.parse("manifest.json", content).unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        assert!(manifest.dependencies[0].name.starts_with("config:sha256:"));
        assert!(manifest.dependencies[1].name.starts_with("layer:sha256:"));
    }

    #[test]
    fn detect_oci() {
        assert!(OciParser.can_parse("index.json", "{}"));
        assert!(OciParser.can_parse(
            "blob",
            r#"{"mediaType":"application/vnd.oci.image.manifest.v1+json"}"#
        ));
        assert!(!OciParser.can_parse("Cargo.toml", ""));
    }

    #[test]
    fn parse_oci_index() {
        let content = r#"{
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [
                {
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "platform": {
                        "architecture": "amd64",
                        "os": "linux"
                    }
                }
            ]
        }"#;
        let manifest = OciParser.parse("index.json", content).unwrap();
        assert_eq!(manifest.dependencies.len(), 1);
        assert_eq!(manifest.dependencies[0].name, "manifest:amd64/linux");
    }
}
