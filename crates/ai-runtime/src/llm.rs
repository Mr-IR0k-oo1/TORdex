//! Local LLM engine interface.
//!
//! Provides an abstraction for running large language models locally.
//! The current implementation is a stub that returns structured responses
//! based on template patterns. Real inference requires integrating a
//! local inference runtime such as llama.cpp, candle, or mistral.rs.
//!
//! All local. No cloud APIs.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Configuration for local LLM execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// Path to the local model file (GGUF, safetensors, etc.).
    pub model_path: Option<String>,
    /// Maximum context size in tokens.
    pub context_size: usize,
    /// Maximum generation length in tokens.
    pub max_tokens: usize,
    /// Temperature for sampling (0.0 = deterministic, 1.0 = creative).
    pub temperature: f64,
    /// Top-p nucleus sampling.
    pub top_p: f64,
    /// System prompt to prepend.
    pub system_prompt: Option<String>,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            model_path: None,
            context_size: 2048,
            max_tokens: 512,
            temperature: 0.7,
            top_p: 0.9,
            system_prompt: None,
        }
    }
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMMessage {
    pub role: String,
    pub content: String,
}

impl LLMMessage {
    #[must_use]
    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: content.to_string(),
        }
    }

    #[must_use]
    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.to_string(),
        }
    }

    #[must_use]
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: content.to_string(),
        }
    }
}

/// Result of LLM inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResult {
    pub content: String,
    pub tokens_generated: usize,
    pub finish_reason: String,
    pub metadata: HashMap<String, String>,
}

/// Trait for local LLM backends.
pub trait LocalLLMBackend: Send + Sync {
    fn generate(&self, messages: &[LLMMessage], config: &LLMConfig) -> LLMResult;
    fn name(&self) -> &str;
}

/// Stub LLM engine that uses simple template-based responses.
///
/// This is a placeholder that demonstrates the interface without
/// requiring actual model files. Replace with a real local inference
/// backend (llama.cpp, candle, etc.) for production use.
#[derive(Debug, Clone)]
pub struct StubLLMEngine;

impl StubLLMEngine {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for StubLLMEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalLLMBackend for StubLLMEngine {
    fn name(&self) -> &str {
        "stub-llm"
    }

    fn generate(&self, messages: &[LLMMessage], config: &LLMConfig) -> LLMResult {
        let last = messages
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("");

        // Simple response templates
        let content = if last.contains("?") {
            format!(
                "Based on the information provided, I would analyze this as follows:\n\n\
                The query \"{last}\" touches on key aspects that require deeper analysis. \
                A local model would process this input using the configured context window \
                of {} tokens and generate a response with temperature {:.1}.",
                config.context_size, config.temperature
            )
        } else if last.to_lowercase().contains("hello") || last.to_lowercase().contains("hi") {
            "Hello! I'm the local TORdex AI assistant. I can help analyze information, \
            answer questions about indexed data, and assist with reasoning tasks."
                .to_string()
        } else {
            format!(
                "I received your input ({} characters). A fully integrated local LLM \
                would process this through the model at {} and provide a detailed response.",
                last.len(),
                config.model_path.as_deref().unwrap_or("local model path")
            )
        };

        let token_count = content.split_whitespace().count();

        LLMResult {
            content,
            tokens_generated: token_count,
            finish_reason: "stop".to_string(),
            metadata: HashMap::new(),
        }
    }
}

impl std::fmt::Debug for dyn LocalLLMBackend + Send + Sync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalLLMBackend")
            .field("name", &self.name())
            .finish()
    }
}

/// Local LLM manager that coordinates model backends.
#[derive(Debug)]
pub struct LLMEngine {
    config: LLMConfig,
    backend: Box<dyn LocalLLMBackend + Send + Sync>,
}

impl LLMEngine {
    #[must_use]
    pub fn new(config: LLMConfig, backend: Box<dyn LocalLLMBackend + Send + Sync>) -> Self {
        Self { config, backend }
    }

    /// Create an engine with the stub (no real model) backend.
    #[must_use]
    pub fn stub() -> Self {
        Self {
            config: LLMConfig::default(),
            backend: Box::new(StubLLMEngine::new()),
        }
    }

    /// Run inference against the local model.
    pub fn chat(&self, messages: &[LLMMessage]) -> LLMResult {
        self.backend.generate(messages, &self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_engine_responds() {
        let engine = LLMEngine::stub();
        let messages = vec![LLMMessage::user("What is the capital of France?")];
        let result = engine.chat(&messages);
        assert!(!result.content.is_empty());
        assert_eq!(result.finish_reason, "stop");
    }

    #[test]
    fn greeting_response() {
        let engine = LLMEngine::stub();
        let messages = vec![LLMMessage::user("Hello!")];
        let result = engine.chat(&messages);
        assert!(result.content.contains("TORdex"));
    }

    #[test]
    fn llm_result_serialization() {
        let result = LLMResult {
            content: "Test response".to_string(),
            tokens_generated: 2,
            finish_reason: "stop".to_string(),
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: LLMResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.content, "Test response");
    }

    #[test]
    fn config_uses_defaults() {
        let config = LLMConfig::default();
        assert_eq!(config.context_size, 2048);
        assert_eq!(config.max_tokens, 512);
        assert!((config.temperature - 0.7).abs() < 0.01);
    }
}
