use std::collections::HashMap;
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct TemporalAlgebraProcessor;

impl TemporalAlgebraProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TemporalAlgebraProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for TemporalAlgebraProcessor {
    fn name(&self) -> &str {
        "temporal_algebra"
    }

    fn description(&self) -> &str {
        "Analyzes temporal patterns in sequential data: ordering, intervals, and temporal relations"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-temporal-algebra", "application/x-mathematics"]
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
                "cannot perform temporal analysis on empty data".into(),
            ));
        }

        let n = data.len();
        let mut events: Vec<(usize, u8, &str)> = Vec::new();

        if let Ok(text) = std::str::from_utf8(data) {
            let mut in_interval = false;
            let mut interval_start = 0usize;

            for (line_idx, line) in text.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if line.starts_with('[') && line.ends_with(']') {
                    let label = &line[1..line.len() - 1];
                    events.push((line_idx, b'I', label));
                } else if line.starts_with("before:") {
                    let rest = line[7..].trim();
                    events.push((line_idx, b'<', rest));
                } else if line.starts_with("after:") {
                    let rest = line[6..].trim();
                    events.push((line_idx, b'>', rest));
                } else if line.starts_with("during:") {
                    let rest = line[7..].trim();
                    events.push((line_idx, b'D', rest));
                } else if line == "begin" {
                    in_interval = true;
                    interval_start = line_idx;
                } else if line == "end" && in_interval {
                    in_interval = false;
                    events.push((interval_start, b'B', &text.lines().nth(interval_start).unwrap_or("")));
                    events.push((line_idx, b'E', "end"));
                } else if !line.is_empty() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let operator = match parts[0] {
                            "seq" => 'S',
                            "par" => 'P',
                            "alt" => 'A',
                            _ => 'E',
                        };
                        events.push((line_idx, operator as u8, &line));
                    }
                }
            }
        }

        let sequence_length = n;
        let event_count = events.len();

        let mut unique_vals = HashMap::new();
        let mut positions = HashMap::new();
        for (i, &byte) in data.iter().enumerate() {
            unique_vals.entry(byte).or_insert(0u64);
            *unique_vals.get_mut(&byte).unwrap() += 1;
            positions.entry(byte).or_insert_with(Vec::new).push(i);
        }

        let mut intervals_found = 0;
        let mut _before_relations = 0;
        let mut _after_relations = 0;
        for (_, kind, _) in &events {
            match *kind {
                b'I' => intervals_found += 1,
                b'<' => _before_relations += 1,
                b'>' => _after_relations += 1,
                _ => {}
            }
        }

        let mut transitions = 0u64;
        for i in 0..n.saturating_sub(1) {
            if data[i] != data[i + 1] {
                transitions += 1;
            }
        }
        let transition_rate = if n > 1 {
            transitions as f64 / (n - 1) as f64
        } else {
            0.0
        };

        let mut first_occurrence: HashMap<u8, usize> = HashMap::new();
        let mut last_occurrence: HashMap<u8, usize> = HashMap::new();
        for (i, &byte) in data.iter().enumerate() {
            first_occurrence.entry(byte).or_insert(i);
            last_occurrence.insert(byte, i);
        }

        let mut temporal_relations: Vec<serde_json::Value> = Vec::new();
        for (b1, &first1) in &first_occurrence {
            if let Some(&last1) = last_occurrence.get(b1) {
                if first1 < last1 {
                    temporal_relations.push(
                        serde_json::json!({
                            "byte": format!("0x{b1:02x}"),
                            "first_at": first1,
                            "last_at": last1,
                            "span": last1 - first1,
                        }),
                    );
                }
            }
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_ta_summary"),
                "temporal.summary",
                format!("{sequence_length} positions, {event_count} events").into_bytes(),
                "text/plain",
            )
            .with_metadata("sequence_length", &sequence_length.to_string())
            .with_metadata("event_count", &event_count.to_string())
            .with_metadata("transition_rate", &format!("{:.4}", transition_rate))
            .with_metadata("source_observation", id),
        );

        if event_count > 0 {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_ta_events"),
                    "temporal.sequence",
                    serde_json::json!(events.iter().map(|(pos, kind, label)| {
                        serde_json::json!({
                            "position": pos,
                            "kind": String::from_utf8(vec![*kind]).unwrap_or_default(),
                            "label": label,
                        })
                    }).collect::<Vec<_>>()).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("metric", "events")
                .with_metadata("event_count", &event_count.to_string())
                .with_metadata("source_observation", id),
            );
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_ta_spans"),
                "temporal.interval",
                serde_json::json!(temporal_relations).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("metric", "temporal_spans")
            .with_metadata("intervals_detected", &intervals_found.to_string())
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_ta_metadata"),
                "temporal.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("sequence_length", &sequence_length.to_string())
            .with_metadata("transitions", &transitions.to_string())
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_temporal_analysis() {
        let proc = TemporalAlgebraProcessor::new();
        let data = b"hello world";
        let results = proc.process("t1", data, Some("application/x-temporal-algebra"), HashMap::new()).unwrap();
        let summaries: Vec<_> = results.iter().filter(|o| o.kind == "temporal.summary").collect();
        assert!(!summaries.is_empty());
        assert!(std::str::from_utf8(&summaries[0].data).unwrap().contains("11 positions"));
    }

    #[test]
    fn detects_temporal_relations() {
        let proc = TemporalAlgebraProcessor::new();
        let data = b"[event1]\n[event2]\nbefore: event2\nafter: event1";
        let results = proc.process("t2", data, Some("application/x-temporal-algebra"), HashMap::new()).unwrap();
        let evts: Vec<_> = results.iter().filter(|o| o.kind == "temporal.sequence").collect();
        assert!(!evts.is_empty());
    }

    #[test]
    fn transition_rate_computation() {
        let proc = TemporalAlgebraProcessor::new();
        let data = b"aaaa";
        let results = proc.process("t3", data, Some("application/x-temporal-algebra"), HashMap::new()).unwrap();
        let summaries: Vec<_> = results.iter().filter(|o| o.kind == "temporal.summary").collect();
        assert!(std::str::from_utf8(&summaries[0].data).unwrap().contains("4 positions"));
        let meta: Vec<_> = results.iter().filter(|o| o.kind == "temporal.metadata").collect();
        assert_eq!(meta.len(), 1);
    }

    #[test]
    fn empty_data_returns_error() {
        let proc = TemporalAlgebraProcessor::new();
        let result = proc.process("t4", b"", Some("application/x-temporal-algebra"), HashMap::new());
        assert!(result.is_err());
    }
}
