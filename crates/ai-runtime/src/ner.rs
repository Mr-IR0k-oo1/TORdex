//! Rule-based Named Entity Recognition engine.
//!
//! Uses regex patterns to extract common entity types from text
//! without external ML dependencies.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Kinds of entities the NER engine can recognise.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Person,
    Organization,
    Location,
    Date,
    Time,
    Email,
    Url,
    Phone,
    Money,
    Percentage,
    Product,
    Event,
    Custom(String),
}

impl EntityKind {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Person => "PERSON",
            Self::Organization => "ORG",
            Self::Location => "LOC",
            Self::Date => "DATE",
            Self::Time => "TIME",
            Self::Email => "EMAIL",
            Self::Url => "URL",
            Self::Phone => "PHONE",
            Self::Money => "MONEY",
            Self::Percentage => "PERCENT",
            Self::Product => "PRODUCT",
            Self::Event => "EVENT",
            Self::Custom(s) => s,
        }
    }
}

/// A single extracted entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub text: String,
    pub kind: EntityKind,
    pub confidence: f64,
    pub start: usize,
    pub end: usize,
}

/// Result of NER analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NERResult {
    pub entities: Vec<Entity>,
    pub entity_count: usize,
}

/// Rule-based NER engine.
#[derive(Debug, Clone)]
pub struct NEREngine {
    patterns: Vec<(EntityKind, regex::Regex)>,
}

impl NEREngine {
    /// Create a new NER engine with built-in pattern rules.
    #[must_use]
    pub fn new() -> Self {
        Self { patterns: Self::builtin_patterns() }
    }

    fn builtin_patterns() -> Vec<(EntityKind, regex::Regex)> {
        let mut p = Vec::new();

        // Email
        if let Ok(re) = regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}") {
            p.push((EntityKind::Email, re));
        }

        // URL
        if let Ok(re) = regex::Regex::new(r"https?://[^\s,;)]+") {
            p.push((EntityKind::Url, re));
        }

        // ISO date (must come before Phone to avoid date→phone mis-classification)
        if let Ok(re) = regex::Regex::new(r"\d{4}-\d{2}-\d{2}") {
            p.push((EntityKind::Date, re));
        }

        // Phone (basic international)
        if let Ok(re) = regex::Regex::new(r"\+?[\d\- \(\)]{7,20}") {
            p.push((EntityKind::Phone, re));
        }

        // US date format
        if let Ok(re) = regex::Regex::new(r"\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]* \d{1,2},?\s*\d{4}\b") {
            p.push((EntityKind::Date, re));
        }

        // Time
        if let Ok(re) = regex::Regex::new(r"\b\d{1,2}:\d{2}(?::\d{2})?\s*(?:AM|PM|am|pm)?\b") {
            p.push((EntityKind::Time, re));
        }

        // Money
        if let Ok(re) = regex::Regex::new(r"\$\s?\d+(?:,\d{3})*(?:\.\d{1,2})?") {
            p.push((EntityKind::Money, re));
        }

        // Percentage
        if let Ok(re) = regex::Regex::new(r"\b\d+(?:\.\d+)?\s*%") {
            p.push((EntityKind::Percentage, re));
        }

        // Capitalised multi-word phrases (potential Person / Org / Location)
        if let Ok(re) = regex::Regex::new(r"\b[A-Z][a-z]+(?:\s+[A-Z][a-z]+){1,3}") {
            p.push((EntityKind::Person, re));
        }

        p
    }

    /// Extract entities from text.
    #[must_use]
    pub fn extract(&self, text: &str) -> NERResult {
        let mut entities = Vec::new();
        let mut seen: HashMap<(usize, usize, String), f64> = HashMap::new();

        for (kind, re) in &self.patterns {
            for cap in re.find_iter(text) {
                let start = cap.start();
                let end = cap.end();
                let matched = cap.as_str().to_string();
                let key = (start, end, kind.name().to_string());

                let confidence = match kind {
                    EntityKind::Email | EntityKind::Url => 0.95,
                    EntityKind::Phone => 0.85,
                    EntityKind::Date | EntityKind::Time => 0.80,
                    EntityKind::Money | EntityKind::Percentage => 0.85,
                    EntityKind::Person => {
                        // Lower confidence for long phrases that may not be names
                        let words: Vec<&str> = matched.split_whitespace().collect();
                        if words.len() <= 4 { 0.60 } else { 0.40 }
                    }
                    _ => 0.70,
                };

                seen.entry(key)
                    .and_modify(|e| *e = f64::max(*e, confidence))
                    .or_insert(confidence);
            }
        }

        for ((start, end, _), confidence) in seen {
            let text_slice = &text[start..end];
            // Determine entity kind by re-matching against patterns
            let kind = self.resolve_kind(text_slice);
            entities.push(Entity {
                text: text_slice.to_string(),
                kind,
                confidence,
                start,
                end,
            });
        }

        // Sort by position
        entities.sort_by(|a, b| a.start.cmp(&b.start));

        let entity_count = entities.len();
        NERResult { entities, entity_count }
    }

    fn resolve_kind(&self, text: &str) -> EntityKind {
        for (kind, re) in &self.patterns {
            if re.is_match(text) {
                return kind.clone();
            }
        }
        EntityKind::Custom("UNKNOWN".to_string())
    }
}

impl Default for NEREngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_email() {
        let engine = NEREngine::new();
        let result = engine.extract("Contact support@example.com for help.");
        assert!(result.entities.iter().any(|e| e.kind == EntityKind::Email));
    }

    #[test]
    fn extracts_url() {
        let engine = NEREngine::new();
        let result = engine.extract("Visit https://tordex.io for details.");
        assert!(result.entities.iter().any(|e| e.kind == EntityKind::Url));
    }

    #[test]
    fn extracts_date_iso() {
        let engine = NEREngine::new();
        let result = engine.extract("Released on 2025-01-15.");
        assert!(result.entities.iter().any(|e| e.kind == EntityKind::Date));
    }

    #[test]
    fn extracts_money() {
        let engine = NEREngine::new();
        let result = engine.extract("Cost: $1,299.99");
        assert!(result.entities.iter().any(|e| e.kind == EntityKind::Money));
    }

    #[test]
    fn extracts_percentage() {
        let engine = NEREngine::new();
        let result = engine.extract("Growth rate was 23.5%");
        assert!(result.entities.iter().any(|e| e.kind == EntityKind::Percentage));
    }

    #[test]
    fn empty_text_returns_empty() {
        let engine = NEREngine::new();
        let result = engine.extract("");
        assert_eq!(result.entity_count, 0);
    }

    #[test]
    fn extracts_multiple_entities() {
        let engine = NEREngine::new();
        let result = engine.extract(
            "Contact alice@example.com or visit https://example.com before 2025-06-01."
        );
        assert!(result.entity_count >= 3);
    }

    #[test]
    fn ner_result_serialization_roundtrip() {
        let result = NERResult {
            entities: vec![Entity {
                text: "test@test.com".to_string(),
                kind: EntityKind::Email,
                confidence: 0.95,
                start: 0,
                end: 13,
            }],
            entity_count: 1,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: NERResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.entity_count, 1);
        assert_eq!(back.entities[0].text, "test@test.com");
    }
}
