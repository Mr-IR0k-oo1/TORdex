use std::collections::HashMap;

use crate::Document;

/// An in-memory search index storing documents and optional semantic embeddings.
#[derive(Debug)]
pub struct SearchIndex {
    documents: Vec<Document>,
    embeddings: HashMap<String, Vec<f64>>,
}

impl SearchIndex {
    #[must_use]
    pub fn new() -> Self {
        Self {
            documents: Vec::new(),
            embeddings: HashMap::new(),
        }
    }

    /// Add a document to the index.
    pub fn add_document(&mut self, doc: Document) {
        self.documents.push(doc);
    }

    /// Return all indexed documents.
    #[must_use]
    pub fn all_documents(&self) -> Vec<Document> {
        self.documents.clone()
    }

    /// Get the stored embedding vector for a document, if any.
    #[must_use]
    pub fn get_embedding(&self, id: &str) -> Option<Vec<f64>> {
        self.embeddings.get(id).cloned()
    }

    /// Set the embedding vector for a document.
    pub fn set_embedding(&mut self, id: &str, embedding: Vec<f64>) {
        self.embeddings.insert(id.to_string(), embedding);
    }

    /// Number of indexed documents.
    #[must_use]
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Remove all documents and embeddings.
    pub fn clear(&mut self) {
        self.documents.clear();
        self.embeddings.clear();
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}
