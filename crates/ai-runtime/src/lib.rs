#![forbid(unsafe_code)]
#![allow(clippy::module_name_repetitions)]

//! # TORdex AI Runtime
//!
//! Local-only AI inference engine for knowledge consumption.
//!
//! AI is not the system's foundation — AI is a consumer of the
//! knowledge, evidence, and relationships the system produces.
//!
//! ## Capabilities
//!
//! - **NER** — rule-based named entity recognition (email, URL, date, money, etc.)
//! - **Summarization** — extractive sentence-scoring summarization
//! - **Classification** — keyword-based text classification
//! - **Embeddings** — term-frequency and TF-IDF vector generation
//! - **Reasoning** — forward-chaining rule engine over structured facts
//! - **Translation** — phrase-lookup and interface for local MT models
//! - **LLM** — abstraction for local large language model inference (stub included)
//!
//! All computation is local. No cloud APIs, no external model calls.

pub mod classification;
pub mod embeddings;
pub mod llm;
pub mod models;
pub mod ner;
pub mod reasoning;
pub mod summarization;
pub mod translation;

pub use classification::{Category, ClassificationResult, Classifier};
pub use embeddings::{EmbeddingConfig, EmbeddingEngine, EmbeddingMethod, EmbeddingResult};
pub use llm::{LLMConfig, LLMEngine, LLMMessage, LLMResult, LocalLLMBackend, StubLLMEngine};
pub use models::{AIModel, ModelKind};
pub use ner::{Entity, EntityKind, NEREngine, NERResult};
pub use reasoning::{Fact, ReasoningEngine, ReasoningResult, ReasoningStep, Rule};
pub use summarization::{ExtractiveSummarizer, SummarizationConfig, SummarizationResult};
pub use translation::{
    Language, PhraseTranslationEngine, TranslationBackend, TranslationConfig, TranslationResult,
};

/// Central AI Runtime — provides access to all AI capabilities.
#[derive(Debug)]
pub struct AiRuntime {
    pub ner: NEREngine,
    pub summarizer: ExtractiveSummarizer,
    pub classifier: Classifier,
    pub embeddings: EmbeddingEngine,
    pub reasoner: ReasoningEngine,
    pub translator: PhraseTranslationEngine,
}

impl AiRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ner: NEREngine::new(),
            summarizer: ExtractiveSummarizer::new(),
            classifier: Classifier::new(),
            embeddings: EmbeddingEngine::new(),
            reasoner: ReasoningEngine::new(),
            translator: PhraseTranslationEngine::new(),
        }
    }

    /// List available AI models.
    #[must_use]
    pub fn list_models() -> Vec<AIModel> {
        vec![
            AIModel::new("ner-rules-v1", ModelKind::NER, "1.0.0", "Rule-based named entity recognition (email, URL, date, money, percentage)"),
            AIModel::new("extractive-summarizer-v1", ModelKind::Summarization, "1.0.0", "Extractive summarization via term-frequency sentence scoring"),
            AIModel::new("keyword-classifier-v1", ModelKind::Classification, "1.0.0", "Keyword-based text classification (technical, business, science, security)"),
            AIModel::new("bow-embeddings-v1", ModelKind::Embeddings, "1.0.0", "Bag-of-words term-frequency embedding generation"),
            AIModel::new("tfidf-embeddings-v1", ModelKind::Embeddings, "1.0.0", "TF-IDF weighted embedding generation"),
            AIModel::new("forward-chainer-v1", ModelKind::Reasoning, "1.0.0", "Forward-chaining rule engine for structured reasoning"),
            AIModel::new("phrase-translator-v1", ModelKind::Translation, "1.0.0", "Phrase-lookup translation for common technical terms"),
            AIModel::new("llm-stub-v1", ModelKind::LLM, "1.0.0", "Stub LLM engine (replace with local model backend)"),
        ]
    }
}

impl Default for AiRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_has_all_engines() {
        let rt = AiRuntime::new();
        let result = rt.ner.extract("test@example.com");
        assert!(result.entity_count > 0);

        let summary = rt.summarizer.summarize("First sentence. Second sentence about key topics and themes. Third sentence.");
        assert!(summary.summary_sentence_count > 0);

        let classification = rt.classifier.classify("API implementation with database queries");
        assert_eq!(classification.label, "technical");

        let emb = rt.embeddings.embed("hello world");
        assert!(emb.dimension > 0);

        let translation = rt.translator.translate("hello", &TranslationConfig {
            source_language: Some(Language::English),
            target_language: Language::Spanish,
            allow_phrase_fallback: true,
        });
        assert_eq!(translation.translated_text, "hola");
    }

    #[test]
    fn list_models_includes_all_kinds() {
        let models = AiRuntime::list_models();
        assert_eq!(models.len(), 8);
        let kinds: std::collections::HashSet<ModelKind> =
            models.iter().map(|m| m.kind).collect();
        assert!(kinds.contains(&ModelKind::NER));
        assert!(kinds.contains(&ModelKind::LLM));
    }
}
