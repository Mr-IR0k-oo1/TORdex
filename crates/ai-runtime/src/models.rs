//! AI model registry — defines the model types and capabilities
//! available in the local AI Runtime.

use serde::{Deserialize, Serialize};

/// Kinds of AI models supported by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelKind {
    NER,
    Summarization,
    Classification,
    Embeddings,
    Reasoning,
    Translation,
    LLM,
}

impl ModelKind {
    /// Human-readable label for this model kind.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::NER => "named-entity-recognition",
            Self::Summarization => "summarization",
            Self::Classification => "classification",
            Self::Embeddings => "embeddings",
            Self::Reasoning => "reasoning",
            Self::Translation => "translation",
            Self::LLM => "llm",
        }
    }
}

/// A registered AI model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIModel {
    pub name: String,
    pub kind: ModelKind,
    pub version: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl AIModel {
    #[must_use]
    pub fn new(name: &str, kind: ModelKind, version: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            kind,
            version: version.to_string(),
            description: description.to_string(),
            parameters: serde_json::Value::Null,
        }
    }
}
