use std::collections::{BTreeSet, HashMap};
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct SetTheoryProcessor;

impl SetTheoryProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SetTheoryProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for SetTheoryProcessor {
    fn name(&self) -> &str {
        "set_theory"
    }

    fn description(&self) -> &str {
        "Detects set structures in data: unique elements, cardinality, set operations"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-set-theory", "application/x-mathematics"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let mut results = Vec::new();

        if data.is_empty() {
            return Err(ProcessorError::InvalidInput(
                "cannot perform set analysis on empty data".into(),
            ));
        }

        let mut sets: Vec<BTreeSet<String>> = Vec::new();
        let text = std::str::from_utf8(data);

        if let Ok(s) = text {
            let mut current_set = BTreeSet::new();
            for line in s.lines() {
                let line = line.trim();
                if line.starts_with('{') && line.ends_with('}') {
                    let mut set = BTreeSet::new();
                    for elem in line[1..line.len() - 1].split(',') {
                        let e = elem.trim().trim_matches('"');
                        if !e.is_empty() {
                            set.insert(e.to_string());
                        }
                    }
                    if !set.is_empty() {
                        sets.push(set);
                    }
                } else if line == "---" {
                    if !current_set.is_empty() {
                        sets.push(current_set.clone());
                        current_set.clear();
                    }
                } else if !line.is_empty() && !line.starts_with('#') {
                    current_set.insert(line.to_string());
                }
            }
            if !current_set.is_empty() {
                sets.push(current_set);
            }
        }

        if sets.is_empty() {
            let byte_set: BTreeSet<String> = data.iter().map(|b| format!("0x{b:02x}")).collect();
            sets.push(byte_set);
        }

        for (i, set) in sets.iter().enumerate() {
            let cardinality = set.len();

            results.push(
                ProcessedObservation::new(
                    format!("{id}_set_{i}_cardinality"),
                    "set.cardinality",
                    cardinality.to_string().into_bytes(),
                    "text/plain",
                )
                .with_metadata("metric", "cardinality")
                .with_metadata("set_index", &i.to_string())
                .with_metadata("value", &cardinality.to_string())
                .with_metadata("source_observation", id),
            );

            let elements_json: Vec<&str> = set.iter().map(|s| s.as_str()).collect();
            results.push(
                ProcessedObservation::new(
                    format!("{id}_set_{i}_elements"),
                    "set.elements",
                    serde_json::json!(elements_json).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("metric", "elements")
                .with_metadata("set_index", &i.to_string())
                .with_metadata("cardinality", &cardinality.to_string())
                .with_metadata("source_observation", id),
            );
        }

        if sets.len() >= 2 {
            let intersection: BTreeSet<String> = sets[0].intersection(&sets[1]).cloned().collect();
            results.push(
                ProcessedObservation::new(
                    format!("{id}_set_intersection"),
                    "set.operation",
                    serde_json::json!(intersection.iter().collect::<Vec<_>>()).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("operation", "intersection")
                .with_metadata("cardinality", &intersection.len().to_string())
                .with_metadata("source_observation", id),
            );

            let union: BTreeSet<String> = sets[0].union(&sets[1]).cloned().collect();
            results.push(
                ProcessedObservation::new(
                    format!("{id}_set_union"),
                    "set.operation",
                    serde_json::json!(union.iter().collect::<Vec<_>>()).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("operation", "union")
                .with_metadata("cardinality", &union.len().to_string())
                .with_metadata("source_observation", id),
            );

            let difference: BTreeSet<String> = sets[0].difference(&sets[1]).cloned().collect();
            if !difference.is_empty() {
                results.push(
                    ProcessedObservation::new(
                        format!("{id}_set_difference"),
                        "set.operation",
                        serde_json::json!(difference.iter().collect::<Vec<_>>()).to_string().into_bytes(),
                        "application/json",
                    )
                    .with_metadata("operation", "set_difference")
                    .with_metadata("cardinality", &difference.len().to_string())
                    .with_metadata("source_observation", id),
                );
            }
        }

        let total_sets = sets.len();
        results.push(
            ProcessedObservation::new(
                format!("{id}_set_metadata"),
                "set.metadata",
                format!("{total_sets} set(s) detected").into_bytes(),
                "text/plain",
            )
            .with_metadata("total_sets", &total_sets.to_string())
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
    fn detects_curly_brace_set() {
        let proc = SetTheoryProcessor::new();
        let data = b"{a,b,c}";
        let results = proc.process("s1", data, Some("application/x-set-theory"), HashMap::new()).unwrap();
        let cards: Vec<_> = results.iter().filter(|o| o.kind == "set.cardinality").collect();
        assert!(!cards.is_empty());
        assert_eq!(std::str::from_utf8(&cards[0].data).unwrap(), "3");
    }

    #[test]
    fn detects_line_separated_sets() {
        let proc = SetTheoryProcessor::new();
        let data = b"apple\nbanana\ncherry\n---\ndates\nelderberry";
        let results = proc.process("s2", data, Some("application/x-set-theory"), HashMap::new()).unwrap();
        let ops: Vec<_> = results.iter().filter(|o| o.kind == "set.operation").collect();
        assert!(!ops.is_empty());
    }

    #[test]
    fn raw_bytes_produce_single_set() {
        let proc = SetTheoryProcessor::new();
        let data = b"\x00\x01\x02\x03";
        let results = proc.process("s3", data, Some("application/x-set-theory"), HashMap::new()).unwrap();
        let cards: Vec<_> = results.iter().filter(|o| o.kind == "set.cardinality").collect();
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn empty_data_returns_error() {
        let proc = SetTheoryProcessor::new();
        let result = proc.process("s4", b"", Some("application/x-set-theory"), HashMap::new());
        assert!(result.is_err());
    }
}
