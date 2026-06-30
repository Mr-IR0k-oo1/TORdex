//! Local embedding generation engine.
//!
//! Produces numerical vector representations of text using
//! term-frequency (bag-of-words) and TF-IDF weighting.
//! All computation is local — no external model APIs.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Embedding generation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingMethod {
    /// Raw term frequency (bag-of-words).
    BoW,
    /// Term frequency with inverse document frequency weighting.
    /// Requires a corpus of documents for IDF calculation.
    TfIdf,
}

impl EmbeddingMethod {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::BoW => "bow",
            Self::TfIdf => "tfidf",
        }
    }
}

/// Configuration for embedding generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub method: EmbeddingMethod,
    /// Minimum term length (shorter terms are excluded).
    pub min_term_length: usize,
    /// Maximum vocabulary size (most frequent terms kept).
    pub max_vocab_size: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            method: EmbeddingMethod::BoW,
            min_term_length: 2,
            max_vocab_size: 10_000,
        }
    }
}

/// Result of embedding generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    pub vector: Vec<f64>,
    pub dimension: usize,
    pub vocabulary: Vec<String>,
    pub method: EmbeddingMethod,
}

/// Local embedding engine.
#[derive(Debug, Clone)]
pub struct EmbeddingEngine {
    config: EmbeddingConfig,
    /// Document frequency map for TF-IDF (populated via `feed_corpus`).
    df: HashMap<String, usize>,
    /// Total number of documents seen.
    total_docs: usize,
}

impl EmbeddingEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: EmbeddingConfig::default(),
            df: HashMap::new(),
            total_docs: 0,
        }
    }

    #[must_use]
    pub fn with_config(config: EmbeddingConfig) -> Self {
        Self {
            config,
            df: HashMap::new(),
            total_docs: 0,
        }
    }

    /// Feed a batch of documents to compute document frequencies for TF-IDF.
    pub fn feed_corpus(&mut self, docs: &[&str]) {
        for doc in docs {
            self.total_docs += 1;
            let terms: HashSet<String> = self.tokenize(doc).into_iter().collect();
            for term in terms {
                *self.df.entry(term).or_insert(0) += 1;
            }
        }
    }

    /// Generate an embedding vector for the given text.
    #[must_use]
    pub fn embed(&self, text: &str) -> EmbeddingResult {
        let terms = self.tokenize(text);
        let mut term_freq: HashMap<String, usize> = HashMap::new();
        for term in &terms {
            *term_freq.entry(term.clone()).or_insert(0) += 1;
        }

        // Build vocabulary from term frequency (slim down if needed)
        let mut vocab: Vec<String> = {
            let mut pairs: Vec<(String, usize)> = term_freq
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            pairs.sort_by(|a, b| b.1.cmp(&a.1));
            pairs.truncate(self.config.max_vocab_size);
            pairs.into_iter().map(|(k, _)| k).collect()
        };
        vocab.sort();

        let max_freq = term_freq.values().max().copied().unwrap_or(1) as f64;
        let vector: Vec<f64> = vocab
            .iter()
            .map(|term| {
                let tf = *term_freq.get(term).unwrap_or(&0) as f64 / max_freq;
                match self.config.method {
                    EmbeddingMethod::BoW => tf,
                    EmbeddingMethod::TfIdf => {
                        let df = self.df.get(term).copied().unwrap_or(1) as f64;
                        let idf = if self.total_docs > 0 {
                            (self.total_docs as f64 / df).ln()
                        } else {
                            1.0
                        };
                        tf * idf
                    }
                }
            })
            .collect();

        let dimension = vector.len();
        EmbeddingResult {
            vector,
            dimension,
            vocabulary: vocab,
            method: self.config.method,
        }
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()))
            .filter(|w| w.len() >= self.config.min_term_length)
            .map(|w| w.to_string())
            .collect()
    }
}

impl Default for EmbeddingEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bow_embedding_produces_vector() {
        let engine = EmbeddingEngine::new();
        let result = engine.embed("hello world hello");
        assert!(result.dimension > 0);
        assert_eq!(result.method, EmbeddingMethod::BoW);
    }

    #[test]
    fn empty_text_produces_zero_vector() {
        let engine = EmbeddingEngine::new();
        let result = engine.embed("");
        assert_eq!(result.dimension, 0);
    }

    #[test]
    fn tfidf_after_corpus() {
        let mut engine = EmbeddingEngine::with_config(EmbeddingConfig {
            method: EmbeddingMethod::TfIdf,
            min_term_length: 2,
            max_vocab_size: 10_000,
        });
        engine.feed_corpus(&["the cat sat on the mat", "the dog ran in the park"]);
        let result = engine.embed("the cat ran");
        assert!(result.dimension > 0);
        assert_eq!(result.method, EmbeddingMethod::TfIdf);
    }

    #[test]
    fn embedding_result_serialization() {
        let result = EmbeddingResult {
            vector: vec![0.1, 0.2, 0.3],
            dimension: 3,
            vocabulary: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            method: EmbeddingMethod::BoW,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: EmbeddingResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.dimension, 3);
        assert_eq!(back.vector, vec![0.1, 0.2, 0.3]);
    }
}
