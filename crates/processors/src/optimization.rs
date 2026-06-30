use std::collections::HashMap;
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct OptimizationProcessor;

impl OptimizationProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OptimizationProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for OptimizationProcessor {
    fn name(&self) -> &str {
        "optimization"
    }

    fn description(&self) -> &str {
        "Analyzes data for optimization potential: objective functions, constraints, and solution spaces"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-optimization", "application/x-mathematics"]
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
                "cannot perform optimization analysis on empty data".into(),
            ));
        }

        let text = std::str::from_utf8(data).ok();
        let mut constraints: Vec<String> = Vec::new();
        let mut objectives: Vec<String> = Vec::new();
        let mut variables: Vec<String> = Vec::new();

        if let Some(s) = text {
            for line in s.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if line.to_lowercase().contains("minimize") || line.to_lowercase().contains("maximize") {
                    objectives.push(line.to_string());
                }

                if line.to_lowercase().contains("subject to")
                    || line.contains('<')
                    || line.contains('>')
                    || line.contains("leq")
                    || line.contains("geq")
                {
                    constraints.push(line.to_string());
                }

                for word in line.split_whitespace() {
                    if word.chars().all(|c| c.is_ascii_alphabetic())
                        && word.len() >= 2
                        && word.chars().next().unwrap().is_lowercase()
                    {
                        let var = word.trim_matches(|c: char| !c.is_ascii_alphabetic());
                        if !var.is_empty() && !variables.contains(&var.to_string()) {
                            variables.push(var.to_string());
                        }
                    }
                }
            }
        }

        let mut numeric_data: Vec<f64> = Vec::new();
        if let Some(s) = text {
            for token in s.split_whitespace() {
                if let Ok(n) = token.parse::<f64>() {
                    numeric_data.push(n);
                }
            }
        }

        if numeric_data.is_empty() && variables.is_empty() {
            numeric_data = data.iter().map(|b| *b as f64).collect();
        }

        let var_count = variables.len();
        let constraint_count = constraints.len();
        let objective_count = objectives.len();

        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;
        for &val in &numeric_data {
            if val < min_val {
                min_val = val;
            }
            if val > max_val {
                max_val = val;
            }
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_opt_summary"),
                "optimization.summary",
                format!("{var_count} variables, {constraint_count} constraints, {objective_count} objectives").into_bytes(),
                "text/plain",
            )
            .with_metadata("variable_count", &var_count.to_string())
            .with_metadata("constraint_count", &constraint_count.to_string())
            .with_metadata("objective_count", &objective_count.to_string())
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_opt_range"),
                "optimization.solution",
                format!("range: [{min_val}, {max_val}]").into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "value_range")
            .with_metadata("min", &format!("{:.4}", min_val))
            .with_metadata("max", &format!("{:.4}", max_val))
            .with_metadata("source_observation", id),
        );

        if !variables.is_empty() {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_opt_variables"),
                    "optimization.variables",
                    serde_json::json!(variables).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("metric", "variables")
                .with_metadata("source_observation", id),
            );
        }

        if !constraints.is_empty() {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_opt_constraints"),
                    "optimization.constraint",
                    serde_json::json!(constraints).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("metric", "constraints")
                .with_metadata("source_observation", id),
            );
        }

        if !objectives.is_empty() {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_opt_objectives"),
                    "optimization.objective",
                    serde_json::json!(objectives).to_string().into_bytes(),
                    "application/json",
                )
                .with_metadata("metric", "objectives")
                .with_metadata("source_observation", id),
            );
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_opt_metadata"),
                "optimization.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("data_points", &numeric_data.len().to_string())
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_optimization_problem() {
        let proc = OptimizationProcessor::new();
        let data = b"minimize cost\nsubject to x + y <= 10\nx >= 0\ny >= 0";
        let results = proc.process("o1", data, Some("application/x-optimization"), HashMap::new()).unwrap();
        let summaries: Vec<_> = results.iter().filter(|o| o.kind == "optimization.summary").collect();
        assert!(!summaries.is_empty());
    }

    #[test]
    fn extracts_numeric_range() {
        let proc = OptimizationProcessor::new();
        let data = b"3.0 7.0 1.0 9.0 2.0";
        let results = proc.process("o2", data, Some("application/x-optimization"), HashMap::new()).unwrap();
        let solutions: Vec<_> = results.iter().filter(|o| o.kind == "optimization.solution").collect();
        assert!(!solutions.is_empty());
        let sol = std::str::from_utf8(&solutions[0].data).unwrap();
        assert!(sol.contains("1") && sol.contains("9"));
    }

    #[test]
    fn extracts_variables() {
        let proc = OptimizationProcessor::new();
        let data = b"cost + revenue + profit";
        let results = proc.process("o3", data, Some("application/x-optimization"), HashMap::new()).unwrap();
        let vars: Vec<_> = results.iter().filter(|o| o.kind == "optimization.variables").collect();
        assert!(!vars.is_empty());
    }

    #[test]
    fn empty_data_returns_error() {
        let proc = OptimizationProcessor::new();
        let result = proc.process("o4", b"", Some("application/x-optimization"), HashMap::new());
        assert!(result.is_err());
    }
}
