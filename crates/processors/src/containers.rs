//! Container processor — analyzes Dockerfiles, OCI images, and container configs.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct ContainerProcessor;

impl ContainerProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn analyze_dockerfile(&self, source: &str) -> Vec<ProcessedObservation> {
        let mut results = Vec::new();
        let mut stages: Vec<String> = Vec::new();
        let mut base_images: Vec<String> = Vec::new();
        let mut instructions: Vec<String> = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("FROM ") {
                let image = trimmed[5..].trim().to_string();
                base_images.push(image.clone());
                stages.push(image);
            } else if trimmed.starts_with("RUN ") {
                instructions.push(format!("RUN: {}", &trimmed[4..].trim()));
            } else if trimmed.starts_with("COPY ") || trimmed.starts_with("ADD ") {
                instructions.push(trimmed.to_string());
            } else if trimmed.starts_with("EXPOSE ") {
                instructions.push(trimmed.to_string());
            } else if trimmed.starts_with("ENV ") {
                instructions.push(trimmed.to_string());
            } else if trimmed.starts_with("CMD ") || trimmed.starts_with("ENTRYPOINT ") {
                instructions.push(trimmed.to_string());
            }
        }

        if !base_images.is_empty() {
            let json = serde_json::to_string(&base_images).unwrap_or_default();
            results.push(
                ProcessedObservation::new(
                    format!("{}{}", "_docker_base_images", base_images.len()),
                    "container.base_images",
                    json.into_bytes(),
                    "application/json",
                )
                .with_metadata("count", &base_images.len().to_string()),
            );
        }

        if !stages.is_empty() {
            results.push(
                ProcessedObservation::new(
                    format!("{}{}", "_docker_stages", stages.len()),
                    "container.stages",
                    stages.join(", ").into_bytes(),
                    "text/plain",
                )
                .with_metadata("stage_count", &stages.len().to_string()),
            );
        }

        if !instructions.is_empty() {
            let json = serde_json::to_string(&instructions).unwrap_or_default();
            results.push(
                ProcessedObservation::new(
                    format!("{}{}", "_docker_instructions", instructions.len()),
                    "container.instructions",
                    json.into_bytes(),
                    "application/json",
                )
                .with_metadata("instruction_count", &instructions.len().to_string()),
            );
        }

        results
    }

    fn analyze_oci_manifest(&self, json_str: &str) -> Vec<ProcessedObservation> {
        let mut results = Vec::new();

        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(layers) = parsed.get("layers").and_then(|v| v.as_array()) {
                let layer_count = layers.len();
                let total_size: u64 = layers
                    .iter()
                    .filter_map(|l| l.get("size").and_then(|s| s.as_u64()))
                    .sum();

                results.push(
                        ProcessedObservation::new(
                            "_oci_layers".to_string(),
                        "container.oci_layers",
                        layer_count.to_string().into_bytes(),
                        "text/plain",
                    )
                    .with_metadata("layer_count", &layer_count.to_string())
                    .with_metadata("total_size_bytes", &total_size.to_string()),
                );
            }

            if let Some(config) = parsed.get("config") {
                if let Some(digest) = config.get("digest").and_then(|v| v.as_str()) {
                    results.push(
                        ProcessedObservation::new(
                            "_oci_config".to_string(),
                            "container.oci_config",
                            digest.as_bytes().to_vec(),
                            "text/plain",
                        )
                        .with_metadata("digest", digest),
                    );
                }
            }

            if let Some(media_type) = parsed.get("mediaType").and_then(|v| v.as_str()) {
                results.push(
                        ProcessedObservation::new(
                            "_oci_media_type".to_string(),
                        "container.media_type",
                        media_type.as_bytes().to_vec(),
                        "text/plain",
                    )
                    .with_metadata("media_type", media_type),
                );
            }
        }

        results
    }

    fn detect_container_format(&self, data: &[u8]) -> Option<&'static str> {
        if data.len() < 4 {
            return None;
        }
        match &data[..4] {
            [b'F', b'R', b'O', b'M'] => Some("Dockerfile"),
            [0x1F, 0x8B, 0x08, ..] => Some("Compressed layer (gzip)"),
            _ => {
                if let Ok(s) = std::str::from_utf8(data) {
                    let s = s.trim();
                    if s.starts_with('{') && s.contains("\"mediaType\"") {
                        return Some("OCI Image Manifest");
                    }
                    if s.starts_with('{') && s.contains("\"config\"") {
                        return Some("OCI Image Index");
                    }
                }
                None
            }
        }
    }
}

impl Default for ContainerProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for ContainerProcessor {
    fn name(&self) -> &str {
        "containers"
    }

    fn description(&self) -> &str {
        "Analyzes Dockerfiles, OCI image manifests, and container configurations"
    }

    fn content_types(&self) -> Vec<&str> {
        vec![
            "application/x-container",
            "application/vnd.docker.image",
            "application/vnd.docker.dockerfile",
            "application/vnd.oci.image.manifest.v1+json",
            "application/vnd.oci.image.index.v1+json",
            "text/x-dockerfile",
        ]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let mut results = Vec::new();
        let fmt = self.detect_container_format(data).unwrap_or("unknown");

        results.push(
            ProcessedObservation::new(
                format!("{id}_format"),
                "container.format",
                fmt.as_bytes().to_vec(),
                "text/plain",
            )
            .with_metadata("source_observation", id),
        );

        match fmt {
            "Dockerfile" => {
                let source = std::str::from_utf8(data)
                    .map_err(|e| ProcessorError::InvalidInput(e.to_string()))?;
                results.extend(self.analyze_dockerfile(source));
            }
            "OCI Image Manifest" => {
                let source = std::str::from_utf8(data)
                    .map_err(|e| ProcessorError::InvalidInput(e.to_string()))?;
                results.extend(self.analyze_oci_manifest(source));
            }
            _ => {}
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "container.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_dockerfile() {
        let proc = ContainerProcessor::new();
        let df = b"FROM ubuntu:22.04\nRUN apt update\nCOPY . /app\nCMD [\"bash\"]";
        let results = proc.process("c1", df, Some("text/x-dockerfile"), HashMap::new()).unwrap();
        let formats: Vec<_> = results.iter().filter(|o| o.kind == "container.format").collect();
        assert_eq!(std::str::from_utf8(&formats[0].data).unwrap(), "Dockerfile");
    }

    #[test]
    fn analyze_dockerfile_base_images() {
        let proc = ContainerProcessor::new();
        let df = b"FROM node:18 AS builder\nFROM nginx:alpine";
        let results = proc.process("c2", df, Some("text/x-dockerfile"), HashMap::new()).unwrap();
        let base: Vec<_> = results.iter().filter(|o| o.kind == "container.base_images").collect();
        assert!(!base.is_empty());
        let json_str = std::str::from_utf8(&base[0].data).unwrap();
        assert!(json_str.contains("node:18"));
        assert!(json_str.contains("nginx:alpine"));
    }

    #[test]
    fn analyze_oci_manifest() {
        let proc = ContainerProcessor::new();
        let manifest = r#"{
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": {"digest": "sha256:abc123", "size": 1000},
            "layers": [
                {"digest": "sha256:layer1", "size": 500},
                {"digest": "sha256:layer2", "size": 1500}
            ]
        }"#;
        let results = proc.process("c3", manifest.as_bytes(), Some("application/vnd.oci.image.manifest.v1+json"), HashMap::new()).unwrap();
        let layers: Vec<_> = results.iter().filter(|o| o.kind == "container.oci_layers").collect();
        assert!(!layers.is_empty());
    }

    #[test]
    fn error_on_empty() {
        let proc = ContainerProcessor::new();
        let results = proc.process("c4", b"", Some("application/x-container"), HashMap::new()).unwrap();
        assert!(!results.is_empty());
    }
}
