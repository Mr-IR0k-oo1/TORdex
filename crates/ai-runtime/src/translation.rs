//! Local translation framework.
//!
//! Provides a structured interface for translation capabilities.
//! The current implementation uses a simple phrase lookup for common
//! technical terms. Full MT requires integrating local models such as
//! Bergamot, CTranslate2, or llama.cpp — this module provides the
//! wiring and data structures for that integration.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Supported languages for translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    English,
    Spanish,
    French,
    German,
    Portuguese,
    Russian,
    Japanese,
    ChineseSimplified,
    Arabic,
}

impl Language {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::German => "de",
            Self::Portuguese => "pt",
            Self::Russian => "ru",
            Self::Japanese => "ja",
            Self::ChineseSimplified => "zh",
            Self::Arabic => "ar",
        }
    }
}

/// Configuration for translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationConfig {
    pub source_language: Option<Language>,
    pub target_language: Language,
    /// When true, falls back to simple phrase lookup if no model available.
    pub allow_phrase_fallback: bool,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            source_language: None,
            target_language: Language::English,
            allow_phrase_fallback: true,
        }
    }
}

/// Result of translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub translated_text: String,
    pub source_language: Option<Language>,
    pub target_language: Language,
    pub confidence: f64,
}

/// Thread-safe marker trait for translation backends.
pub trait TranslationBackend: Send + Sync {
    fn translate(&self, text: &str, config: &TranslationConfig) -> TranslationResult;
    fn name(&self) -> &str;
}

/// Simple phrase-lookup translation engine.
///
/// Contains a small dictionary of common technical terms for
/// demonstration and fallback purposes. Real translation requires
/// an external model backend.
#[derive(Debug, Clone)]
pub struct PhraseTranslationEngine {
    /// Maps (source_lang_code, target_lang_code) → phrase map
    dictionaries: HashMap<(String, String), HashMap<String, String>>,
}

impl PhraseTranslationEngine {
    #[must_use]
    pub fn new() -> Self {
        let mut dicts: HashMap<(String, String), HashMap<String, String>> = HashMap::new();

        // Small English→Spanish technical dictionary
        let mut en_es = HashMap::new();
        en_es.insert("hello".to_string(), "hola".to_string());
        en_es.insert("error".to_string(), "error".to_string());
        en_es.insert("warning".to_string(), "advertencia".to_string());
        en_es.insert("success".to_string(), "éxito".to_string());
        en_es.insert("file".to_string(), "archivo".to_string());
        en_es.insert("directory".to_string(), "directorio".to_string());
        en_es.insert("system".to_string(), "sistema".to_string());
        en_es.insert("network".to_string(), "red".to_string());
        en_es.insert("database".to_string(), "base de datos".to_string());
        en_es.insert("server".to_string(), "servidor".to_string());
        dicts.insert(("en".to_string(), "es".to_string()), en_es);

        Self { dictionaries: dicts }
    }

    /// Translate text using phrase lookup.
    #[must_use]
    pub fn translate(&self, text: &str, config: &TranslationConfig) -> TranslationResult {
        let src = config
            .source_language
            .unwrap_or(Language::English)
            .code()
            .to_string();
        let tgt = config.target_language.code().to_string();

        let key = (src, tgt);
        let dict = self.dictionaries.get(&key);

        match dict {
            Some(phrases) => {
                let mut translated = text.to_string();
                let mut matches = 0;
                let total_words = text.split_whitespace().count().max(1);

                for (en, localized) in phrases {
                    if translated.contains(en.as_str()) {
                        translated = translated.replace(en.as_str(), localized.as_str());
                        matches += 1;
                    }
                }

                TranslationResult {
                    translated_text: translated,
                    source_language: config.source_language,
                    target_language: config.target_language,
                    confidence: matches as f64 / total_words as f64,
                }
            }
            None => TranslationResult {
                translated_text: text.to_string(),
                source_language: config.source_language,
                target_language: config.target_language,
                confidence: 0.0,
            },
        }
    }
}

impl Default for PhraseTranslationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_translation_known_words() {
        let engine = PhraseTranslationEngine::new();
        let config = TranslationConfig {
            source_language: Some(Language::English),
            target_language: Language::Spanish,
            allow_phrase_fallback: true,
        };
        let result = engine.translate("hello system error", &config);
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn phrase_translation_unknown_language_passthrough() {
        let engine = PhraseTranslationEngine::new();
        let config = TranslationConfig {
            source_language: Some(Language::French),
            target_language: Language::English,
            allow_phrase_fallback: true,
        };
        let result = engine.translate("bonjour le monde", &config);
        assert_eq!(result.translated_text, "bonjour le monde");
    }

    #[test]
    fn translation_result_serialization() {
        let result = TranslationResult {
            translated_text: "hola".to_string(),
            source_language: Some(Language::English),
            target_language: Language::Spanish,
            confidence: 1.0,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: TranslationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.translated_text, "hola");
    }
}
