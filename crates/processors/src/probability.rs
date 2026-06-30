use std::collections::HashMap;
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct ProbabilityProcessor;

impl ProbabilityProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProbabilityProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for ProbabilityProcessor {
    fn name(&self) -> &str {
        "probability"
    }

    fn description(&self) -> &str {
        "Computes byte-frequency distributions, Shannon entropy, and statistical moments from data"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-probability", "application/x-mathematics"]
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
                "cannot compute probability distribution on empty data".into(),
            ));
        }

        let n = data.len() as f64;
        let mut freq = [0u64; 256];
        for &byte in data {
            freq[byte as usize] += 1;
        }

        let mut entropy = 0.0f64;
        let mut mode_byte = 0u8;
        let mut max_freq = 0u64;
        let mut unique_count = 0;
        let mut sum = 0u64;
        for (i, &count) in freq.iter().enumerate() {
            if count > 0 {
                unique_count += 1;
                let p = count as f64 / n;
                entropy -= p * p.log2();
                sum += (i as u64) * count;
                if count > max_freq {
                    max_freq = count;
                    mode_byte = i as u8;
                }
            }
        }

        let mean = sum as f64 / n;
        let mut variance = 0.0f64;
        for &byte in data {
            let diff = byte as f64 - mean;
            variance += diff * diff;
        }
        variance /= n;
        let std_dev = variance.sqrt();

        let dist_json = {
            let mut pairs: Vec<_> = (0..=255)
                .filter(|i| freq[*i] > 0)
                .map(|i| (i, freq[i]))
                .collect();
            pairs.sort_by(|a, b| b.1.cmp(&a.1));
            let top: Vec<serde_json::Value> = pairs
                .iter()
                .take(16)
                .map(|(byte, count)| {
                    serde_json::json!({
                        "byte": byte,
                        "count": count,
                        "probability": format!("{:.4}", *count as f64 / n)
                    })
                })
                .collect();
            serde_json::json!(top).to_string()
        };

        results.push(
            ProcessedObservation::new(
                format!("{id}_prob_distribution"),
                "probability.distribution",
                dist_json.into_bytes(),
                "application/json",
            )
            .with_metadata("metric", "frequency_distribution")
            .with_metadata("unique_values", &unique_count.to_string())
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_prob_entropy"),
                "probability.entropy",
                entropy.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "shannon_entropy")
            .with_metadata("value", &format!("{:.4}", entropy))
            .with_metadata("max_entropy", &format!("{:.4}", 8.0f64))
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_prob_moments"),
                "probability.statistics",
                format!("mean={mean:.4}, std_dev={std_dev:.4}, mode=0x{mode_byte:02x}").into_bytes(),
                "text/plain",
            )
            .with_metadata("mean", &format!("{:.4}", mean))
            .with_metadata("std_deviation", &format!("{:.4}", std_dev))
            .with_metadata("mode_byte", &format!("0x{mode_byte:02x}"))
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_prob_metadata"),
                "probability.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("sample_count", &data.len().to_string())
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_entropy_on_known_data() {
        let proc = ProbabilityProcessor::new();
        let data = b"\x00\x00\x00\x00\x01\x01\x01\x01";
        let results = proc.process("p1", data, Some("application/x-probability"), HashMap::new()).unwrap();
        let entropies: Vec<_> = results.iter().filter(|o| o.kind == "probability.entropy").collect();
        assert_eq!(entropies.len(), 1);
        let e: f64 = std::str::from_utf8(&entropies[0].data).unwrap().parse().unwrap();
        assert!((e - 1.0).abs() < 0.001);
    }

    #[test]
    fn uniform_distribution_has_max_entropy() {
        let proc = ProbabilityProcessor::new();
        let data: Vec<u8> = (0..=255).cycle().take(2560).collect();
        let results = proc.process("p2", &data, Some("application/x-probability"), HashMap::new()).unwrap();
        let entropies: Vec<_> = results.iter().filter(|o| o.kind == "probability.entropy").collect();
        let e: f64 = std::str::from_utf8(&entropies[0].data).unwrap().parse().unwrap();
        assert!((e - 8.0).abs() < 0.1);
    }

    #[test]
    fn empty_data_returns_error() {
        let proc = ProbabilityProcessor::new();
        let result = proc.process("p3", b"", Some("application/x-probability"), HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn produces_distribution_observation() {
        let proc = ProbabilityProcessor::new();
        let results = proc.process("p4", b"hello world", Some("application/x-probability"), HashMap::new()).unwrap();
        let dists: Vec<_> = results.iter().filter(|o| o.kind == "probability.distribution").collect();
        assert_eq!(dists.len(), 1);
    }
}
