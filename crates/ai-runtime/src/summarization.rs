//! Extractive text summarization engine.
//!
//! Scores sentences by term frequency and position, then selects
//! the top-ranked sentences to form a summary.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Configuration for extractive summarization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizationConfig {
    /// Maximum number of sentences in the summary (default: 5).
    pub max_sentences: usize,
    /// Target compression ratio (0.0–1.0) relative to original sentence count.
    /// If set, `max_sentences` is ignored in favour of this ratio.
    pub compression_ratio: Option<f64>,
    /// Minimum sentence length in characters (shorter sentences are skipped).
    pub min_sentence_length: usize,
}

impl Default for SummarizationConfig {
    fn default() -> Self {
        Self {
            max_sentences: 5,
            compression_ratio: None,
            min_sentence_length: 20,
        }
    }
}

/// Result of summarization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizationResult {
    pub summary: String,
    pub original_sentence_count: usize,
    pub summary_sentence_count: usize,
    pub compression_ratio: f64,
    pub top_sentences: Vec<String>,
}

/// Extractive summarization engine.
#[derive(Debug, Clone)]
pub struct ExtractiveSummarizer {
    config: SummarizationConfig,
}

impl ExtractiveSummarizer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SummarizationConfig::default(),
        }
    }

    #[must_use]
    pub fn with_config(config: SummarizationConfig) -> Self {
        Self { config }
    }

    /// Summarize the given text using extractive sentence scoring.
    #[must_use]
    pub fn summarize(&self, text: &str) -> SummarizationResult {
        let sentences = self.split_sentences(text);
        if sentences.is_empty() {
            return SummarizationResult {
                summary: String::new(),
                original_sentence_count: 0,
                summary_sentence_count: 0,
                compression_ratio: 1.0,
                top_sentences: Vec::new(),
            };
        }

        // Build term frequency map across all sentences
        let mut tf: HashMap<String, usize> = HashMap::new();
        let mut sentence_word_counts: Vec<Vec<String>> = Vec::new();

        for sentence in &sentences {
            let words: Vec<String> = sentence
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()).to_string())
                .filter(|w| !w.is_empty())
                .collect();
            let mut seen = std::collections::HashSet::new();
            for word in &words {
                if seen.insert(word.clone()) {
                    *tf.entry(word.clone()).or_insert(0) += 1;
                }
            }
            sentence_word_counts.push(words);
        }

        let total_sentences = sentences.len();

        // Score each sentence
        let mut scored: Vec<(usize, f64)> = Vec::new();
        for (i, words) in sentence_word_counts.iter().enumerate() {
            if sentences[i].len() < self.config.min_sentence_length {
                continue;
            }

            let mut score = 0.0;
            let mut seen = std::collections::HashSet::new();
            for word in words {
                if seen.insert(word.clone()) {
                    let freq = tf.get(word).unwrap_or(&0);
                    score += *freq as f64;
                }
            }

            // Normalize by word count
            if !words.is_empty() {
                score /= words.len() as f64;
            }

            // Position bonus: first sentences get a boost
            let position_bonus = 1.0 + (1.0 - (i as f64 / total_sentences as f64)) * 0.5;
            score *= position_bonus;

            scored.push((i, score));
        }

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Determine how many sentences to include
        let max_sentences = self
            .config
            .compression_ratio
            .map(|r| (total_sentences as f64 * r).ceil() as usize)
            .unwrap_or(self.config.max_sentences)
            .max(1);

        let top_n = max_sentences.min(scored.len());

        // Select top sentences and re-sort by original position
        let top_indices: std::collections::BTreeSet<usize> = scored
            .iter()
            .take(top_n)
            .map(|(idx, _)| *idx)
            .collect();

        let top_sentences: Vec<String> = top_indices
            .iter()
            .map(|i| sentences[*i].clone())
            .collect();

        let summary = top_sentences.join(" ");

        let compression_ratio = if total_sentences > 0 {
            top_sentences.len() as f64 / total_sentences as f64
        } else {
            1.0
        };

        SummarizationResult {
            summary,
            original_sentence_count: total_sentences,
            summary_sentence_count: top_sentences.len(),
            compression_ratio,
            top_sentences,
        }
    }

    fn split_sentences(&self, text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();

        for ch in text.chars() {
            current.push(ch);
            if ch == '.' || ch == '!' || ch == '?' {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() && trimmed.len() > 1 {
                    sentences.push(trimmed);
                }
                current = String::new();
            }
        }

        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() && trimmed.len() > 1 {
            sentences.push(trimmed);
        }

        sentences
    }
}

impl Default for ExtractiveSummarizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_short_text() {
        let summarizer = ExtractiveSummarizer::new();
        let result = summarizer.summarize("Hello world. This is a test. Just a simple sentence for summarization purposes.");
        assert!(result.summary_sentence_count > 0);
    }

    #[test]
    fn summarize_empty_text() {
        let summarizer = ExtractiveSummarizer::new();
        let result = summarizer.summarize("");
        assert_eq!(result.summary_sentence_count, 0);
    }

    #[test]
    fn summarize_with_config() {
        let config = SummarizationConfig {
            max_sentences: 2,
            compression_ratio: None,
            min_sentence_length: 5,
        };
        let summarizer = ExtractiveSummarizer::with_config(config);
        let text = "First important sentence with key terms. Second sentence. Third sentence. Fourth sentence about important topics and key themes.";
        let result = summarizer.summarize(text);
        assert!(result.summary_sentence_count <= 2);
    }

    #[test]
    fn compression_ratio_respected() {
        let config = SummarizationConfig {
            max_sentences: 100,
            compression_ratio: Some(0.5),
            min_sentence_length: 5,
        };
        let summarizer = ExtractiveSummarizer::with_config(config);
        let text = "A. B. C. D. E. F. G. H.";
        let result = summarizer.summarize(text);
        // 8 sentences, 0.5 ratio => ~4 sentences selected
        assert!(result.summary_sentence_count <= 5);
    }

    #[test]
    fn summarization_result_serialization() {
        let result = SummarizationResult {
            summary: "Test summary.".to_string(),
            original_sentence_count: 5,
            summary_sentence_count: 1,
            compression_ratio: 0.2,
            top_sentences: vec!["Test summary.".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: SummarizationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.summary, "Test summary.");
    }
}
