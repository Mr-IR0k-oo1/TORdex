//! Strategy-specific search helpers.
//!
//! This module provides utilities used by the search engine strategies.
//! Currently a placeholder for future strategy-specific optimizations.

use std::collections::HashMap;

use crate::Document;

/// Build a word-frequency vector for a set of documents.
/// Returns a map from document ID to its word-frequency vector.
#[must_use]
pub fn build_word_frequencies(docs: &[Document]) -> HashMap<String, Vec<(String, usize)>> {
    let mut result = HashMap::new();
    for doc in docs {
        let mut freq: HashMap<String, usize> = HashMap::new();
        for word in doc.body.to_lowercase().split_whitespace() {
            *freq.entry(word.to_string()).or_insert(0) += 1;
        }
        for word in doc.title.to_lowercase().split_whitespace() {
            *freq.entry(word.to_string()).or_insert(0) += 3;
        }
        let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1));
        result.insert(doc.id.clone(), pairs);
    }
    result
}

/// Score a document for a keyword — returns term frequency count.
#[must_use]
pub fn keyword_score(doc: &Document, term: &str) -> usize {
    let term_lower = term.to_lowercase();
    let body_count = doc.body.to_lowercase().matches(&term_lower).count();
    let title_count = doc.title.to_lowercase().matches(&term_lower).count();
    title_count * 3 + body_count
}
