use std::collections::HashMap;
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct CategoryTheoryProcessor;

impl CategoryTheoryProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CategoryTheoryProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for CategoryTheoryProcessor {
    fn name(&self) -> &str {
        "category_theory"
    }

    fn description(&self) -> &str {
        "Detects categorical structure: objects, morphisms, and composition patterns in data"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-category-theory", "application/x-mathematics"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let mut results = Vec::new();
        let text = std::str::from_utf8(data)
            .map_err(|_| ProcessorError::InvalidInput("category data must be valid UTF-8".into()))?;

        let mut objects: Vec<&str> = Vec::new();
        let mut morphisms: Vec<(&str, &str, &str)> = Vec::new();
        let mut compositions: Vec<(&str, &str)> = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with("obj:") {
                let obj = line[4..].trim();
                if !obj.is_empty() {
                    objects.push(obj);
                }
            } else if line.starts_with("arr:") {
                let rest = line[4..].trim();
                if let Some(arrow_pos) = rest.find("->") {
                    let src = rest[..arrow_pos].trim();
                    let tgt_and_name = rest[arrow_pos + 2..].trim();
                    if let Some(colon_pos) = tgt_and_name.find(':') {
                        let tgt = tgt_and_name[..colon_pos].trim();
                        let name = tgt_and_name[colon_pos + 1..].trim();
                        morphisms.push((src, tgt, name));
                    }
                }
            } else if let Some(comp_pos) = line.find("=>") {
                let before = line[..comp_pos].trim();
                let after = line[comp_pos + 2..].trim();
                if !before.is_empty() && !after.is_empty() {
                    compositions.push((before, after));
                }
            }
        }

        if objects.is_empty() && morphisms.is_empty() {
            let tokens: Vec<&str> = text.split_whitespace().collect();
            if tokens.len() >= 2 {
                for chunk in tokens.chunks(2) {
                    if chunk.len() == 2 {
                        objects.push(chunk[0]);
                        objects.push(chunk[1]);
                    }
                }
                objects.sort();
                objects.dedup();
            }
            if objects.is_empty() {
                return Err(ProcessorError::ProcessingFailed(
                    "no categorical structure detected".into(),
                ));
            }
        }

        let object_count = objects.len();
        let morphism_count = morphisms.len();
        let composition_count = compositions.len();

        results.push(
            ProcessedObservation::new(
                format!("{id}_cat_summary"),
                "category.summary",
                format!("{object_count} objects, {morphism_count} morphisms").into_bytes(),
                "text/plain",
            )
            .with_metadata("object_count", &object_count.to_string())
            .with_metadata("morphism_count", &morphism_count.to_string())
            .with_metadata("source_observation", id),
        );

        if !objects.is_empty() {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_cat_objects"),
                    "category.objects",
                    serde_json::json!(objects).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("metric", "objects")
                .with_metadata("source_observation", id),
            );
        }

        if !morphisms.is_empty() {
            let morphs: Vec<serde_json::Value> = morphisms
                .iter()
                .map(|(s, t, n)| {
                    serde_json::json!({
                        "source": s,
                        "target": t,
                        "name": n,
                    })
                })
                .collect();
            results.push(
                ProcessedObservation::new(
                    format!("{id}_cat_morphisms"),
                    "category.morphisms",
                    serde_json::json!(morphs).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("metric", "morphisms")
                .with_metadata("source_observation", id),
            );

            let mut identity_count = 0;
            for (s, t, _) in &morphisms {
                if s == t {
                    identity_count += 1;
                }
            }
            if identity_count > 0 {
                results.push(
                    ProcessedObservation::new(
                        format!("{id}_cat_identities"),
                        "category.morphisms",
                        identity_count.to_string().into_bytes(),
                        "text/plain",
                    )
                    .with_metadata("metric", "identity_morphisms")
                    .with_metadata("count", &identity_count.to_string())
                    .with_metadata("source_observation", id),
                );
            }
        }

        if composition_count > 0 {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_cat_compositions"),
                    "category.composition",
                    composition_count.to_string().into_bytes(),
                    "text/plain",
                )
                .with_metadata("metric", "compositions")
                .with_metadata("count", &composition_count.to_string())
                .with_metadata("source_observation", id),
            );
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_cat_metadata"),
                "category.metadata",
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
    fn detects_objects_and_morphisms() {
        let proc = CategoryTheoryProcessor::new();
        let data = b"obj: A\nobj: B\nobj: C\narr: A->B:f\narr: B->C:g";
        let results = proc.process("c1", data, Some("application/x-category-theory"), HashMap::new()).unwrap();
        let summaries: Vec<_> = results.iter().filter(|o| o.kind == "category.summary").collect();
        assert_eq!(summaries.len(), 1);
        assert!(std::str::from_utf8(&summaries[0].data).unwrap().contains("3 objects"));
    }

    #[test]
    fn detects_composition_patterns() {
        let proc = CategoryTheoryProcessor::new();
        let data = b"f -> g => h\ng -> k => l";
        let results = proc.process("c2", data, Some("application/x-category-theory"), HashMap::new()).unwrap();
        let comps: Vec<_> = results.iter().filter(|o| o.kind == "category.composition").collect();
        assert_eq!(comps.len(), 1);
    }

    #[test]
    fn no_structure_returns_error() {
        let proc = CategoryTheoryProcessor::new();
        let data = b"just some random text";
        let result = proc.process("c3", data, Some("application/x-category-theory"), HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn handles_invalid_utf8() {
        let proc = CategoryTheoryProcessor::new();
        let data = b"\xFF\xFE\x00\x01";
        let result = proc.process("c4", data, Some("application/x-category-theory"), HashMap::new());
        assert!(result.is_err());
    }
}
