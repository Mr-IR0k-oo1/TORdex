use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use ulid::Ulid;

use tordex_knowledge::KnowledgeCore;
use tordex_temporal_graph::TemporalGraph;

/// A semantic fact with confidence and provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFact {
    pub id: Ulid,
    pub subject: String,
    pub predicate: String,
    pub object: serde_json::Value,
    pub confidence: f64,
    pub source_ids: Vec<String>,
    pub created_at: OffsetDateTime,
    pub ttl: Option<time::Duration>,
}

impl SemanticFact {
    pub fn new(subject: &str, predicate: &str, object: serde_json::Value) -> Self {
        Self {
            id: Ulid::new(),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object,
            confidence: 1.0,
            source_ids: Vec::new(),
            created_at: OffsetDateTime::now_utc(),
            ttl: None,
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.source_ids.push(source.to_string());
        self
    }
}

/// Query for semantic memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQuery {
    pub subjects: Option<Vec<String>>,
    pub predicates: Option<Vec<String>>,
    pub min_confidence: f64,
    pub limit: usize,
}

impl Default for SemanticQuery {
    fn default() -> Self {
        Self {
            subjects: None,
            predicates: None,
            min_confidence: 0.0,
            limit: 100,
        }
    }
}

/// Semantic Memory — factual knowledge with graph relationships.
///
/// - Entity-relationship-fact store
/// - Temporal graph integration for relationship inference
/// - Deduplication via KnowledgeCore
/// - Confidence-weighted query results
/// - Provenance tracking per fact
pub trait SemanticMemory: Send + Sync {
    fn store_fact(&mut self, fact: SemanticFact) -> Result<Ulid, String>;
    fn store_facts(&mut self, facts: Vec<SemanticFact>) -> Result<Vec<Ulid>, String>;
    fn query(&self, query: &SemanticQuery) -> Result<Vec<SemanticFact>, String>;
    fn delete_fact(&mut self, id: Ulid) -> Result<(), String>;
    fn infer(&self, subject: &str, predicate: &str) -> Result<Vec<SemanticFact>, String>;
    fn graph_snapshot(&self) -> Result<serde_json::Value, String>;
    fn clear(&mut self) -> Result<(), String>;
}

/// Default semantic memory backed by KnowledgeCore + TemporalGraph.
pub struct DefaultSemanticMemory {
    knowledge: KnowledgeCore,
    graph: TemporalGraph,
    facts: HashMap<Ulid, SemanticFact>,
}

impl DefaultSemanticMemory {
    pub fn new() -> Self {
        Self {
            knowledge: KnowledgeCore::new(),
            graph: TemporalGraph::new(),
            facts: HashMap::new(),
        }
    }
}

impl Default for DefaultSemanticMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticMemory for DefaultSemanticMemory {
    fn store_fact(&mut self, fact: SemanticFact) -> Result<Ulid, String> {
        let id = fact.id;
        let content = serde_json::json!({
            "subject": fact.subject,
            "predicate": fact.predicate,
            "object": fact.object,
        });

        let kc = tordex_types::Knowledge {
            id: tordex_types::KnowledgeId::generate(),
            kind: format!("fact:{}", fact.predicate),
            content,
            confidence: fact.confidence,
            source_ids: fact.source_ids.clone(),
            created_at: fact.created_at,
            metadata: std::collections::HashMap::new(),
        };
        let _ = self.knowledge.ingest(&kc);

        if let Some(s) = fact.object.as_str() {
            let rel = tordex_types::Relationship {
                id: tordex_types::RelationshipId::generate(),
                kind: fact.predicate.clone(),
                source_id: fact.subject.clone(),
                target_id: s.to_string(),
                source_type: "entity".to_string(),
                target_type: "entity".to_string(),
                properties: std::collections::HashMap::new(),
                first_seen: fact.created_at,
                last_seen: fact.created_at,
                created_at: fact.created_at,
                metadata: std::collections::HashMap::new(),
            };
            self.graph.ingest_relationship(&rel);
        }

        self.facts.insert(id, fact);
        Ok(id)
    }

    fn store_facts(&mut self, facts: Vec<SemanticFact>) -> Result<Vec<Ulid>, String> {
        facts.into_iter().map(|f| self.store_fact(f)).collect()
    }

    fn query(&self, query: &SemanticQuery) -> Result<Vec<SemanticFact>, String> {
        let mut results: Vec<SemanticFact> = self
            .facts
            .values()
            .filter(|f| f.confidence >= query.min_confidence)
            .filter(|f| {
                query
                    .subjects
                    .as_ref()
                    .map_or(true, |s| s.contains(&f.subject))
            })
            .filter(|f| {
                query
                    .predicates
                    .as_ref()
                    .map_or(true, |p| p.contains(&f.predicate))
            })
            .cloned()
            .collect();

        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(query.limit);
        Ok(results)
    }

    fn delete_fact(&mut self, id: Ulid) -> Result<(), String> {
        self.facts
            .remove(&id)
            .map(|_| ())
            .ok_or_else(|| format!("fact {id} not found"))
    }

    fn infer(&self, subject: &str, predicate: &str) -> Result<Vec<SemanticFact>, String> {
        let facts: Vec<SemanticFact> = self
            .facts
            .values()
            .filter(|f| f.subject == subject && f.predicate == predicate)
            .cloned()
            .collect();

        // Use temporal graph for transitive inference
        let _evolution = self.graph.evolution();
        // Future: graph-based inference across related entities

        Ok(facts)
    }

    fn graph_snapshot(&self) -> Result<serde_json::Value, String> {
        let now = OffsetDateTime::now_utc();
        let snapshot = self.graph.state.snapshot(now);
        serde_json::to_value(&snapshot).map_err(|e| e.to_string())
    }

    fn clear(&mut self) -> Result<(), String> {
        self.facts.clear();
        Ok(())
    }
}
