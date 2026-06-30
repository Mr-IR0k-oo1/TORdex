use std::collections::HashMap;
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct VectorSpaceProcessor;

impl VectorSpaceProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VectorSpaceProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for VectorSpaceProcessor {
    fn name(&self) -> &str {
        "vector_spaces"
    }

    fn description(&self) -> &str {
        "Analyzes data as vectors: dimensionality, basis estimation, linear independence checks"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-vector-spaces", "application/x-mathematics"]
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
                "cannot perform vector analysis on empty data".into(),
            ));
        }

        let n = data.len();

        let dimension = n;

        let mean = data.iter().map(|b| *b as f64).sum::<f64>() / n as f64;
        let mut centered = Vec::with_capacity(n);
        for &b in data {
            centered.push(b as f64 - mean);
        }

        let magnitude: f64 = centered.iter().map(|x| x * x).sum::<f64>().sqrt();

        let variance = centered.iter().map(|x| x * x).sum::<f64>() / n as f64;
        let std_dev = variance.sqrt();

        let mut unique_byte_positions: HashMap<u8, Vec<usize>> = HashMap::new();
        for (i, &b) in data.iter().enumerate() {
            unique_byte_positions.entry(b).or_default().push(i);
        }

        let rank_estimate = unique_byte_positions.len().min(dimension);

        let sparsity = 1.0 - (rank_estimate as f64 / 256.0f64.min(dimension as f64));

        results.push(
            ProcessedObservation::new(
                format!("{id}_vs_summary"),
                "vector.summary",
                format!("dimension={dimension}, rank≈{rank_estimate}, norm={magnitude:.2}").into_bytes(),
                "text/plain",
            )
            .with_metadata("dimension", &dimension.to_string())
            .with_metadata("rank_estimate", &rank_estimate.to_string())
            .with_metadata("norm", &format!("{:.4}", magnitude))
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_vs_dimension"),
                "vector.space",
                dimension.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "dimension")
            .with_metadata("value", &dimension.to_string())
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_vs_basis"),
                "vector.basis",
                serde_json::json!({
                    "mean_vector": format!("{:.4}", mean),
                    "std_dev": format!("{:.4}", std_dev),
                    "magnitude": format!("{:.4}", magnitude),
                    "rank_estimate": rank_estimate,
                    "sparsity": format!("{:.4}", sparsity),
                }).to_string().into_bytes(),
                "application/json",
            )
            .with_metadata("metric", "basis_statistics")
            .with_metadata("source_observation", id),
        );

        let dot_self = magnitude * magnitude;
        if dimension >= 2 {
            let half = dimension / 2;
            let first_half: f64 = data[..half].iter().map(|b| *b as f64).sum();
            let second_half: f64 = data[half..].iter().map(|b| *b as f64).sum();
            let cross_corr = data[..half]
                .iter()
                .zip(data[half..].iter())
                .map(|(a, b)| (*a as f64) * (*b as f64))
                .sum::<f64>();

            results.push(
                ProcessedObservation::new(
                    format!("{id}_vs_linearity"),
                    "vector.metrics",
                    serde_json::json!({
                        "dot_self": dot_self,
                        "cross_correlation": cross_corr,
                        "half1_sum": first_half,
                        "half2_sum": second_half,
                    }).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("metric", "linear_dependence")
                .with_metadata("source_observation", id),
            );
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_vs_metadata"),
                "vector.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("dimension", &dimension.to_string())
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_vector_analysis() {
        let proc = VectorSpaceProcessor::new();
        let data = b"\x01\x02\x03\x04";
        let results = proc.process("v1", data, Some("application/x-vector-spaces"), HashMap::new()).unwrap();
        let summaries: Vec<_> = results.iter().filter(|o| o.kind == "vector.summary").collect();
        assert!(!summaries.is_empty());
        assert!(std::str::from_utf8(&summaries[0].data).unwrap().contains("dimension=4"));
    }

    #[test]
    fn computes_dimension() {
        let proc = VectorSpaceProcessor::new();
        let data = b"\x00\x00\x00\x00\x00";
        let results = proc.process("v2", data, Some("application/x-vector-spaces"), HashMap::new()).unwrap();
        let dims: Vec<_> = results.iter().filter(|o| o.kind == "vector.space").collect();
        assert_eq!(dims.len(), 1);
        assert_eq!(std::str::from_utf8(&dims[0].data).unwrap(), "5");
    }

    #[test]
    fn zero_vector_has_zero_norm() {
        let proc = VectorSpaceProcessor::new();
        let data = vec![0u8; 10];
        let results = proc.process("v3", &data, Some("application/x-vector-spaces"), HashMap::new()).unwrap();
        let summaries: Vec<_> = results.iter().filter(|o| o.kind == "vector.summary").collect();
        assert!(std::str::from_utf8(&summaries[0].data).unwrap().contains("norm=0.00"));
    }

    #[test]
    fn empty_data_returns_error() {
        let proc = VectorSpaceProcessor::new();
        let result = proc.process("v4", b"", Some("application/x-vector-spaces"), HashMap::new());
        assert!(result.is_err());
    }
}
