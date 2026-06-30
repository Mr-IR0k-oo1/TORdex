//! Keyword-based text classification engine.
//!
//! Categories are defined with weighted keyword lists. The classifier
//! scores text against each category and returns the best match.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A classification category with associated keywords.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    /// Keywords that indicate this category. Each keyword carries a weight.
    pub keywords: Vec<(String, f64)>,
    /// Minimum score for a match to be considered valid.
    pub threshold: f64,
}

impl Category {
    #[must_use]
    pub fn new(name: &str, keywords: Vec<(&str, f64)>, threshold: f64) -> Self {
        Self {
            name: name.to_string(),
            keywords: keywords
                .into_iter()
                .map(|(k, w)| (k.to_string(), w))
                .collect(),
            threshold,
        }
    }
}

/// Result of classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// The best-matching label.
    pub label: String,
    /// Confidence score of the best match (0.0–1.0).
    pub confidence: f64,
    /// Scores for all categories.
    pub scores: HashMap<String, f64>,
}

/// Text classifier using keyword matching.
#[derive(Debug, Clone)]
pub struct Classifier {
    categories: Vec<Category>,
}

impl Classifier {
    #[must_use]
    pub fn new() -> Self {
        Self {
            categories: Self::default_categories(),
        }
    }

    /// Create a classifier with custom categories.
    #[must_use]
    pub fn with_categories(categories: Vec<Category>) -> Self {
        Self { categories }
    }

    fn default_categories() -> Vec<Category> {
        vec![
            Category::new(
                "technical",
                vec![
                    ("code", 3.0),
                    ("function", 2.5),
                    ("api", 2.5),
                    ("implementation", 2.0),
                    ("algorithm", 2.5),
                    ("system", 1.5),
                    ("architecture", 2.0),
                    ("database", 2.0),
                    ("server", 1.5),
                    ("deploy", 1.5),
                    ("config", 1.5),
                    ("library", 2.0),
                    ("dependency", 1.5),
                    ("interface", 2.0),
                    ("protocol", 2.0),
                ],
                0.3,
            ),
            Category::new(
                "business",
                vec![
                    ("revenue", 3.0),
                    ("profit", 2.5),
                    ("market", 2.0),
                    ("customer", 2.0),
                    ("growth", 2.0),
                    ("strategy", 2.0),
                    ("investment", 2.0),
                    ("enterprise", 1.5),
                    ("valuation", 2.5),
                    ("acquisition", 2.5),
                ],
                0.3,
            ),
            Category::new(
                "science",
                vec![
                    ("research", 3.0),
                    ("study", 2.0),
                    ("experiment", 2.5),
                    ("analysis", 2.0),
                    ("hypothesis", 2.5),
                    ("theory", 2.0),
                    ("publication", 2.0),
                    ("laboratory", 2.0),
                    ("clinical", 2.5),
                    ("observation", 1.5),
                ],
                0.3,
            ),
            Category::new(
                "security",
                vec![
                    ("vulnerability", 3.0),
                    ("exploit", 3.0),
                    ("attack", 2.5),
                    ("threat", 2.5),
                    ("malware", 3.0),
                    ("encryption", 2.0),
                    ("authentication", 2.0),
                    ("firewall", 2.0),
                    ("breach", 3.0),
                    ("penetration", 2.5),
                    ("cve", 3.0),
                    ("zero-day", 3.0),
                ],
                0.3,
            ),
        ]
    }

    /// Classify the given text against known categories.
    #[must_use]
    pub fn classify(&self, text: &str) -> ClassificationResult {
        let text_lower = text.to_lowercase();
        let mut scores: HashMap<String, f64> = HashMap::new();
        let total_words = text_lower.split_whitespace().count().max(1) as f64;

        for category in &self.categories {
            let mut score = 0.0;
            for (keyword, weight) in &category.keywords {
                let count = text_lower.matches(&keyword.to_lowercase()).count();
                if count > 0 {
                    score += count as f64 * weight;
                }
            }
            // Normalize by text length
            let normalized = score / total_words;
            if normalized >= category.threshold {
                scores.insert(category.name.clone(), normalized);
            }
        }

        // Find best match
        let (label, confidence) = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(l, s)| (l.clone(), *s))
            .unwrap_or_else(|| ("unknown".to_string(), 0.0));

        ClassificationResult {
            label,
            confidence,
            scores,
        }
    }
}

impl Default for Classifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_technical_text() {
        let classifier = Classifier::new();
        let result = classifier.classify(
            "The API implements a function that queries the database server using a custom protocol."
        );
        assert_eq!(result.label, "technical");
    }

    #[test]
    fn classifies_security_text() {
        let classifier = Classifier::new();
        let result = classifier.classify(
            "A critical vulnerability was discovered that allows remote code execution via an authentication bypass exploit."
        );
        assert_eq!(result.label, "security");
    }

    #[test]
    fn unknown_text_returns_unknown() {
        let classifier = Classifier::new();
        let result = classifier.classify("The cat sat on the mat.");
        assert_eq!(result.label, "unknown");
    }

    #[test]
    fn custom_categories() {
        let categories = vec![
            Category::new("rust", vec![("rust", 3.0), ("cargo", 2.0), ("ownership", 2.5)], 0.4),
        ];
        let classifier = Classifier::with_categories(categories);
        let result = classifier.classify("Rust ownership model ensures memory safety via the borrow checker.");
        assert_eq!(result.label, "rust");
    }

    #[test]
    fn classification_result_serialization() {
        let mut scores = HashMap::new();
        scores.insert("tech".to_string(), 0.8);
        let result = ClassificationResult {
            label: "tech".to_string(),
            confidence: 0.8,
            scores,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: ClassificationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.label, "tech");
    }

    #[test]
    fn empty_text_returns_unknown() {
        let classifier = Classifier::new();
        let result = classifier.classify("");
        assert_eq!(result.label, "unknown");
    }
}
