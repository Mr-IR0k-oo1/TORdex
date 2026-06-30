use std::collections::HashMap;

use tordex_ai_runtime::{
    AiRuntime, ClassificationResult, EmbeddingConfig, EmbeddingMethod, EmbeddingResult,
    ExtractiveSummarizer, Language, NERResult, ReasoningResult, SummarizationConfig,
    SummarizationResult, TranslationConfig, TranslationResult,
};
use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

/// A processor that bridges the local AI Runtime into the observation pipeline.
///
/// Accepts `application/x-ai-runtime` content type and provides
/// NER, summarization, classification, embeddings, reasoning, translation,
/// and LLM inference — all local, no cloud APIs.
///
/// Use `action` metadata to select the operation:
/// - `"ner"` — named entity recognition
/// - `"summarize"` — extractive text summarization
/// - `"classify"` — text classification
/// - `"embed"` — generate text embeddings (set `method` metadata to `"bow"` or `"tfidf"`)
/// - `"reason"` — forward-chaining reasoning (body is JSON facts+optional rules)
/// - `"translate"` — phrase-based translation
/// - `"llm"` — local LLM inference (stub)
/// - `"models"` — list available AI models
pub struct AiRuntimeProcessor {
    runtime: std::sync::Mutex<AiRuntime>,
}

impl AiRuntimeProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            runtime: std::sync::Mutex::new(AiRuntime::new()),
        }
    }
}

impl Default for AiRuntimeProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for AiRuntimeProcessor {
    fn name(&self) -> &str {
        "AiRuntimeProcessor"
    }

    fn description(&self) -> &str {
        "Local AI Runtime — NER, summarization, classification, embeddings, reasoning, translation, and LLM inference. All local. No cloud APIs."
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/x-ai-runtime"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let action = metadata
            .get("action")
            .map(|s| s.as_str())
            .unwrap_or("ner");

        let text = String::from_utf8_lossy(data).to_string();

        let runtime = self.runtime.lock().map_err(|e| {
            ProcessorError::ProcessingFailed(format!("lock error: {e}"))
        })?;

        match action {
            "ner" => {
                let result: NERResult = runtime.ner.extract(&text);
                let output = serde_json::to_value(&result).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "ai.ner",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-ai-runtime+ner",
                )
                .with_metadata("entity_count", &result.entity_count.to_string())])
            }

            "summarize" => {
                let config = SummarizationConfig {
                    max_sentences: metadata
                        .get("max_sentences")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(5),
                    compression_ratio: metadata
                        .get("compression_ratio")
                        .and_then(|v| v.parse().ok()),
                    min_sentence_length: metadata
                        .get("min_sentence_length")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(20),
                };
                let summarizer = ExtractiveSummarizer::with_config(config);
                let result: SummarizationResult = summarizer.summarize(&text);
                let output = serde_json::to_value(&result).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "ai.summary",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-ai-runtime+summary",
                )
                .with_metadata(
                    "original_sentences",
                    &result.original_sentence_count.to_string(),
                )
                .with_metadata(
                    "summary_sentences",
                    &result.summary_sentence_count.to_string(),
                )
                .with_metadata(
                    "compression_ratio",
                    &format!("{:.3}", result.compression_ratio),
                )])
            }

            "classify" => {
                let result: ClassificationResult = runtime.classifier.classify(&text);
                let output = serde_json::to_value(&result).unwrap_or_default();
                let confidence_str = format!("{:.4}", result.confidence);
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "ai.classification",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-ai-runtime+classification",
                )
                .with_metadata("label", &result.label)
                .with_metadata("confidence", &confidence_str)])
            }

            "embed" => {
                let method = match metadata.get("method").map(|s| s.as_str()) {
                    Some("tfidf") => EmbeddingMethod::TfIdf,
                    _ => EmbeddingMethod::BoW,
                };
                let config = EmbeddingConfig {
                    method,
                    min_term_length: metadata
                        .get("min_term_length")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(2),
                    max_vocab_size: metadata
                        .get("max_vocab_size")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(10_000),
                };
                // Use a standalone engine to avoid borrowing conflicts
                let engine = tordex_ai_runtime::EmbeddingEngine::with_config(config);
                let result: EmbeddingResult = engine.embed(&text);
                let output = serde_json::to_value(&result).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "ai.embeddings",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-ai-runtime+embeddings",
                )
                .with_metadata("dimension", &result.dimension.to_string())
                .with_metadata("method", method.name())])
            }

            "reason" => {
                // Body expected as JSON with "facts" array and optional "rules" array
                let json_value: serde_json::Value =
                    serde_json::from_slice(data).map_err(|e| {
                        ProcessorError::InvalidInput(format!("invalid JSON: {e}"))
                    })?;

                let facts: Vec<tordex_ai_runtime::Fact> =
                    serde_json::from_value(json_value["facts"].clone()).map_err(|e| {
                        ProcessorError::InvalidInput(format!("invalid facts: {e}"))
                    })?;

                // If custom rules provided, create a temporary engine with them
                let engine = if let Some(rules_val) = json_value.get("rules") {
                    let rules: Vec<tordex_ai_runtime::Rule> =
                        serde_json::from_value(rules_val.clone()).map_err(|e| {
                            ProcessorError::InvalidInput(format!("invalid rules: {e}"))
                        })?;
                    tordex_ai_runtime::ReasoningEngine::with_rules(rules)
                } else {
                    runtime.reasoner.clone()
                };

                let result: ReasoningResult = engine.reason(&facts);
                let output = serde_json::to_value(&result).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "ai.reasoning",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-ai-runtime+reasoning",
                )
                .with_metadata(
                    "conclusions",
                    &result.conclusions.len().to_string(),
                )
                .with_metadata(
                    "iterations",
                    &result.iteration_count.to_string(),
                )])
            }

            "translate" => {
                let source = metadata.get("source_language");
                let target = metadata
                    .get("target_language")
                    .map(|s| s.as_str())
                    .unwrap_or("en");

                let target_lang = match target {
                    "es" => Language::Spanish,
                    "fr" => Language::French,
                    "de" => Language::German,
                    "pt" => Language::Portuguese,
                    _ => Language::English,
                };

                let source_lang = source.and_then(|s| match s.as_str() {
                    "en" => Some(Language::English),
                    "es" => Some(Language::Spanish),
                    "fr" => Some(Language::French),
                    "de" => Some(Language::German),
                    _ => None,
                });

                let config = TranslationConfig {
                    source_language: source_lang,
                    target_language: target_lang,
                    allow_phrase_fallback: true,
                };

                let result: TranslationResult =
                    runtime.translator.translate(&text, &config);
                let confidence_str = format!("{:.4}", result.confidence);
                let output = serde_json::to_value(&result).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "ai.translation",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-ai-runtime+translation",
                )
                .with_metadata("confidence", &confidence_str)
                .with_metadata("target_language", target)])
            }

            "llm" => {
                let engine = tordex_ai_runtime::LLMEngine::stub();
                let messages = vec![tordex_ai_runtime::LLMMessage::user(&text)];
                let result = engine.chat(&messages);
                let output = serde_json::to_value(&result).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "ai.llm",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-ai-runtime+llm",
                )
                .with_metadata(
                    "tokens_generated",
                    &result.tokens_generated.to_string(),
                )
                .with_metadata("finish_reason", &result.finish_reason)])
            }

            "models" => {
                let models = AiRuntime::list_models();
                let output = serde_json::to_value(&models).unwrap_or_default();
                Ok(vec![ProcessedObservation::new(
                    id.to_string(),
                    "ai.models",
                    serde_json::to_vec(&output).unwrap_or_default(),
                    "application/x-ai-runtime+models",
                )
                .with_metadata("model_count", &models.len().to_string())])
            }

            _ => Err(ProcessorError::InvalidInput(format!(
                "unknown action: {action}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_processor() -> AiRuntimeProcessor {
        AiRuntimeProcessor::new()
    }

    #[test]
    fn name_and_content_types() {
        let p = make_processor();
        assert_eq!(p.name(), "AiRuntimeProcessor");
        assert!(p.content_types().contains(&"application/x-ai-runtime"));
    }

    #[test]
    fn ner_extracts_entities() {
        let p = make_processor();
        let results = p
            .process(
                "obs_001",
                b"Contact support@example.com or visit https://tordex.io",
                Some("application/x-ai-runtime"),
                HashMap::from([("action".into(), "ner".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "ai.ner");
        let count: usize = results[0]
            .metadata
            .get("entity_count")
            .unwrap()
            .parse()
            .unwrap();
        assert!(count >= 2);
    }

    #[test]
    fn summarize_short_text() {
        let p = make_processor();
        let text = "First important sentence with key terms. Second minor sentence. Third sentence about topics and themes.";
        let results = p
            .process(
                "obs_001",
                text.as_bytes(),
                Some("application/x-ai-runtime"),
                HashMap::from([("action".into(), "summarize".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "ai.summary");
        assert!(results[0]
            .metadata
            .get("summary_sentences")
            .unwrap()
            .parse::<usize>()
            .unwrap()
            > 0);
    }

    #[test]
    fn classify_technical_text() {
        let p = make_processor();
        let results = p
            .process(
                "obs_001",
                b"API implementation with database queries and server configuration",
                Some("application/x-ai-runtime"),
                HashMap::from([("action".into(), "classify".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "ai.classification");
        assert_eq!(results[0].metadata.get("label").unwrap(), "technical");
    }

    #[test]
    fn embed_bow() {
        let p = make_processor();
        let results = p
            .process(
                "obs_001",
                b"hello world hello",
                Some("application/x-ai-runtime"),
                HashMap::from([("action".into(), "embed".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "ai.embeddings");
        let dim: usize = results[0]
            .metadata
            .get("dimension")
            .unwrap()
            .parse()
            .unwrap();
        assert!(dim > 0);
        assert_eq!(results[0].metadata.get("method").unwrap(), "bow");
    }

    #[test]
    fn reason_with_facts() {
        let p = make_processor();
        let body = serde_json::json!({
            "facts": [
                {"predicate": "is_a", "subject": "Rust", "object": "systems_language", "confidence": 1.0},
                {"predicate": "is_a", "subject": "systems_language", "object": "programming_language", "confidence": 1.0}
            ],
            "rules": [
                {
                    "name": "transitive",
                    "conditions": [
                        ["is_a", "$X", "$Y"],
                        ["is_a", "$Y", "$Z"]
                    ],
                    "conclusion": ["is_a", "$X", "$Z"],
                    "confidence": 0.9
                }
            ]
        });
        let results = p
            .process(
                "obs_001",
                &serde_json::to_vec(&body).unwrap(),
                Some("application/x-ai-runtime"),
                HashMap::from([("action".into(), "reason".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "ai.reasoning");
        let conclusions: usize = results[0]
            .metadata
            .get("conclusions")
            .unwrap()
            .parse()
            .unwrap();
        assert!(conclusions > 0);
    }

    #[test]
    fn translate_english_to_spanish() {
        let p = make_processor();
        let results = p
            .process(
                "obs_001",
                b"hello system",
                Some("application/x-ai-runtime"),
                HashMap::from([
                    ("action".into(), "translate".into()),
                    ("source_language".into(), "en".into()),
                    ("target_language".into(), "es".into()),
                ]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "ai.translation");
        let confidence: f64 = results[0]
            .metadata
            .get("confidence")
            .unwrap()
            .parse()
            .unwrap();
        assert!(confidence > 0.0);
    }

    #[test]
    fn llm_stub_responds() {
        let p = make_processor();
        let results = p
            .process(
                "obs_001",
                b"What is the capital of France?",
                Some("application/x-ai-runtime"),
                HashMap::from([("action".into(), "llm".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "ai.llm");
        assert_eq!(results[0].metadata.get("finish_reason").unwrap(), "stop");
    }

    #[test]
    fn list_models() {
        let p = make_processor();
        let results = p
            .process(
                "obs_001",
                b"{}",
                Some("application/x-ai-runtime"),
                HashMap::from([("action".into(), "models".into())]),
            )
            .unwrap();
        assert_eq!(results[0].kind, "ai.models");
        assert_eq!(
            results[0].metadata.get("model_count").unwrap(),
            "8"
        );
    }

    #[test]
    fn unknown_action_returns_error() {
        let p = make_processor();
        let result = p.process(
            "obs_001",
            b"{}",
            Some("application/x-ai-runtime"),
            HashMap::from([("action".into(), "bogus".into())]),
        );
        assert!(result.is_err());
    }
}
