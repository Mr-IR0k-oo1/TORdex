use std::collections::HashMap;
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct InformationTheoryProcessor;

impl InformationTheoryProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InformationTheoryProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for InformationTheoryProcessor {
    fn name(&self) -> &str {
        "information_theory"
    }

    fn description(&self) -> &str {
        "Computes Shannon entropy, mutual information, redundancy, and compression estimates from data"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-information-theory", "application/x-mathematics"]
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
                "cannot compute information metrics on empty data".into(),
            ));
        }

        let n = data.len();
        let nf = n as f64;

        let mut freq = [0u64; 256];
        for &byte in data {
            freq[byte as usize] += 1;
        }

        let mut entropy = 0.0f64;
        let mut max_freq = 0u64;
        let mut _max_freq_byte = 0usize;
        for (i, &count) in freq.iter().enumerate() {
            if count > 0 {
                let p = count as f64 / nf;
                entropy -= p * p.log2();
                if count > max_freq {
                    max_freq = count;
                    _max_freq_byte = i;
                }
            }
        }

        let max_possible_entropy = 8.0f64;
        let redundancy = 1.0 - (entropy / max_possible_entropy);

        let first_half_end = n / 2;
        let mut freq1 = [0u64; 256];
        let mut freq2 = [0u64; 256];
        for (i, &byte) in data.iter().enumerate() {
            if i < first_half_end {
                freq1[byte as usize] += 1;
            } else {
                freq2[byte as usize] += 1;
            }
        }

        let n1 = first_half_end as f64;
        let n2 = (n - first_half_end) as f64;
        let mut mi = 0.0f64;
        if n1 > 0.0 && n2 > 0.0 {
            let mut p1 = [0.0f64; 256];
            let mut p2 = [0.0f64; 256];
            for (i, (&c1, &c2)) in freq1.iter().zip(freq2.iter()).enumerate() {
                if c1 > 0 {
                    p1[i] = c1 as f64 / n1;
                }
                if c2 > 0 {
                    p2[i] = c2 as f64 / n2;
                }
            }
            let p_joint = |b: u8| -> f64 {
                (freq1[b as usize] + freq2[b as usize]) as f64 / nf
            };
            for b in 0..=255u8 {
                if freq[b as usize] > 0 {
                    let pxy = p_joint(b);
                    let px = p1[b as usize];
                    let py = p2[b as usize];
                    if px > 0.0 && py > 0.0 {
                        mi += pxy * (pxy / (px * py)).log2();
                    }
                }
            }
        }

        let compression_ratio_estimate = entropy / max_possible_entropy;

        let byte_freq_json: Vec<serde_json::Value> = (0..=255)
            .filter(|i| freq[*i] > 0)
            .map(|i| {
                serde_json::json!({
                    "byte": i,
                    "count": freq[i],
                    "information": -((freq[i] as f64 / nf).log2())
                })
            })
            .collect();
        let freq_json = serde_json::json!(byte_freq_json).to_string();

        results.push(
            ProcessedObservation::new(
                format!("{id}_it_entropy"),
                "information.entropy",
                entropy.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "shannon_entropy")
            .with_metadata("value", &format!("{:.4}", entropy))
            .with_metadata("max_possible", &format!("{:.4}", max_possible_entropy))
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_it_redundancy"),
                "information.metrics",
                redundancy.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "redundancy")
            .with_metadata("value", &format!("{:.4}", redundancy))
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_it_mutual_information"),
                "information.metrics",
                mi.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "mutual_information")
            .with_metadata("value", &format!("{:.4}", mi))
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_it_compressibility"),
                "information.metrics",
                format!("{:.2}% of max", compression_ratio_estimate * 100.0).into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "compression_ratio")
            .with_metadata("value", &format!("{:.4}", compression_ratio_estimate))
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_it_frequencies"),
                "information.density",
                freq_json.into_bytes(),
                "application/json",
            )
            .with_metadata("metric", "byte_frequencies")
            .with_metadata("total_bytes", &n.to_string())
            .with_metadata("unique_bytes", &freq.iter().filter(|&&c| c > 0).count().to_string())
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_it_metadata"),
                "information.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("entropy_per_byte", &format!("{:.4}", entropy))
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_data_has_zero_entropy() {
        let proc = InformationTheoryProcessor::new();
        let data = vec![0x41u8; 100];
        let results = proc.process("i1", &data, Some("application/x-information-theory"), HashMap::new()).unwrap();
        let entropies: Vec<_> = results.iter().filter(|o| o.kind == "information.entropy").collect();
        let e: f64 = std::str::from_utf8(&entropies[0].data).unwrap().parse().unwrap();
        assert!(e.abs() < 0.001);
    }

    #[test]
    fn uniform_data_has_max_entropy() {
        let proc = InformationTheoryProcessor::new();
        let mut data = Vec::new();
        for i in 0..=255u8 {
            for _ in 0..10 {
                data.push(i);
            }
        }
        let results = proc.process("i2", &data, Some("application/x-information-theory"), HashMap::new()).unwrap();
        let entropies: Vec<_> = results.iter().filter(|o| o.kind == "information.entropy").collect();
        let e: f64 = std::str::from_utf8(&entropies[0].data).unwrap().parse().unwrap();
        assert!((e - 8.0).abs() < 0.1);
    }

    #[test]
    fn computes_redundancy() {
        let proc = InformationTheoryProcessor::new();
        let data = b"aaaaaaaaaaaaaaaaaaaaaaaa";
        let results = proc.process("i3", data, Some("application/x-information-theory"), HashMap::new()).unwrap();
        let reds: Vec<_> = results.iter().filter(|o| o.kind == "information.metrics").collect();
        assert!(!reds.is_empty());
    }

    #[test]
    fn empty_data_returns_error() {
        let proc = InformationTheoryProcessor::new();
        let result = proc.process("i4", b"", Some("application/x-information-theory"), HashMap::new());
        assert!(result.is_err());
    }
}
