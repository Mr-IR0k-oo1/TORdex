//! Decision Engine — the "brain" of the kernel.
//!
//! Answers questions using graph state, probability, event history, and
//! similarity analysis. Every operation goes through kernel APIs only.

pub mod analyzers;
pub mod question;

use std::collections::HashMap;

use serde_json::json;
use tordex_core::event_store::SystemEvent;
use tordex_core::{CoreError, Kernel, Result as CoreResult};

use crate::analyzers::{
    Analyzer, ChangeAnalyzer, ImpactAnalyzer, InvestigateAnalyzer, PredictionAnalyzer,
    SimilarityAnalyzer, WhyAnalyzer,
};
use crate::question::{Answer, Question};

/// The Decision Engine routes questions to the appropriate analyzer,
/// collects evidence, and records findings in the kernel.
pub struct DecisionEngine {
    analyzers: HashMap<String, Box<dyn Analyzer>>,
}

impl DecisionEngine {
    #[must_use]
    pub fn new() -> Self {
        let mut analyzers: HashMap<String, Box<dyn Analyzer>> = HashMap::new();
        analyzers.insert("change".to_string(), Box::new(ChangeAnalyzer::new()));
        analyzers.insert("why".to_string(), Box::new(WhyAnalyzer::new()));
        analyzers.insert("impact".to_string(), Box::new(ImpactAnalyzer::new()));
        analyzers.insert(
            "investigate".to_string(),
            Box::new(InvestigateAnalyzer::new()),
        );
        analyzers.insert("similarity".to_string(), Box::new(SimilarityAnalyzer::new()));
        analyzers.insert("predict".to_string(), Box::new(PredictionAnalyzer::new()));
        Self { analyzers }
    }

    /// Answer a single question by dispatching to the appropriate analyzer.
    pub fn analyze(&self, kernel: &Kernel, question: &Question) -> CoreResult<Answer> {
        let analyzer_name = match question {
            Question::WhatChanged { .. } => "change",
            Question::Why { .. } => "why",
            Question::WhatMatters { .. } | Question::WhatIsRisky { .. } => "impact",
            Question::WhatShouldIInvestigate { .. } => "investigate",
            Question::WhatIsSimilar { .. } => "similarity",
            Question::WhatWillLikelyHappen { .. } => "predict",
        };

        let analyzer = self.analyzers.get(analyzer_name).ok_or_else(|| {
            CoreError::agent(format!("no analyzer registered for '{analyzer_name}'"))
        })?;

        let answer = analyzer.answer(kernel, question)?;

        // Record the finding in the kernel object store
        self.record_finding(kernel, &answer)?;

        // Publish to event bus
        self.publish_answer(kernel, &answer)?;

        Ok(answer)
    }

    /// Run all applicable analyzers and return their answers.
    pub fn analyze_all(&self, kernel: &Kernel, questions: &[Question]) -> CoreResult<Vec<Answer>> {
        let mut answers = Vec::with_capacity(questions.len());
        for q in questions {
            answers.push(self.analyze(kernel, q)?);
        }
        Ok(answers)
    }

    /// List registered analyzer names.
    #[must_use]
    pub fn analyzer_names(&self) -> Vec<&str> {
        self.analyzers.keys().map(|s| s.as_str()).collect()
    }

    // ── internal helpers ───────────────────────────────────────────────

    fn record_finding(&self, kernel: &Kernel, answer: &Answer) -> CoreResult<()> {
        let data = json!({
            "question": answer.question,
            "summary": answer.summary,
            "confidence": answer.confidence,
            "severity": answer.severity,
            "evidence_count": answer.evidence.len(),
            "recommendation": answer.recommendation,
            "produced_at": answer.produced_at,
        });

        let data_bytes =
            serde_json::to_vec(&data).map_err(|e| CoreError::serialization(e.to_string()))?;

        let kind = if answer.confidence >= 0.7 {
            "decision_verdict"
        } else {
            "decision_finding"
        };

        kernel.objects.create(kind, &answer.id, &data_bytes);
        Ok(())
    }

    fn publish_answer(&self, kernel: &Kernel, answer: &Answer) -> CoreResult<()> {
        let payload = serde_json::to_string(answer)
            .map_err(|e| CoreError::serialization(e.to_string()))?;
        kernel
            .event
            .publish("decision.analysis", payload.as_bytes());
        Ok(())
    }

    /// Emit a `DecisionMade` system event when a high-confidence verdict is reached.
    pub fn emit_decision(
        &self,
        kernel: &Kernel,
        answer: &Answer,
        actor: &str,
    ) -> CoreResult<()> {
        if answer.confidence < 0.7 {
            return Ok(()); // only emit decisions for high-confidence results
        }

        let finding_ids: Vec<String> = answer
            .evidence
            .iter()
            .map(|e| e.aggregate_id.clone())
            .collect();

        let decision = SystemEvent::DecisionMade {
            id: answer.id.clone(),
            finding_ids,
            kind: "analysis".to_string(),
            rationale: answer.summary.clone(),
            actor: actor.to_string(),
            status: "pending".to_string(),
        };

        let envelope = decision.into_envelope(
            kernel.event_store.latest_version(&answer.id) + 1,
        );

        kernel
            .event_store
            .append(envelope)
            .map_err(|e| CoreError::infra(e.to_string()))?;

        Ok(())
    }
}

impl Default for DecisionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tordex_core::event_store::EventEnvelope;
    use tordex_core::Kernel;

    #[test]
    fn engine_creates_with_default_analyzers() {
        let engine = DecisionEngine::new();
        let names = engine.analyzer_names();
        assert!(names.contains(&"change"));
        assert!(names.contains(&"why"));
        assert!(names.contains(&"impact"));
        assert!(names.contains(&"investigate"));
        assert!(names.contains(&"similarity"));
        assert!(names.contains(&"predict"));
        assert_eq!(names.len(), 6);
    }

    #[test]
    fn change_analyzer_returns_answer() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        // Seed some events
        for i in 0..5 {
            let ev = EventEnvelope::new(
                format!("entity_{i}"),
                "Entity",
                "Created",
                1,
                json!({"index": i}),
            );
            kernel.event_store.append(ev).unwrap();
        }

        let question = Question::WhatChanged {
            aggregate_type: Some("Entity".to_string()),
            since: None,
            until: None,
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert!(answer.confidence > 0.0);
        assert!(!answer.summary.is_empty());
        assert!(!answer.evidence.is_empty());
    }

    #[test]
    fn change_analyzer_no_events() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        let question = Question::WhatChanged {
            aggregate_type: Some("Entity".to_string()),
            since: None,
            until: None,
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert_eq!(answer.confidence, 0.5);
    }

    #[test]
    fn why_analyzer_traces_events() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        let id = "test_agg_1";
        for v in 1..=3 {
            let ev = EventEnvelope::new(
                id.to_string(),
                "Entity",
                if v == 1 { "Created" } else { "Updated" },
                v,
                json!({"version": v}),
            );
            kernel.event_store.append(ev).unwrap();
        }

        let question = Question::Why {
            aggregate_id: id.to_string(),
            max_depth: None,
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert!(answer.confidence > 0.0);
        assert_eq!(answer.evidence.len(), 3);
    }

    #[test]
    fn why_analyzer_missing_aggregate() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        let question = Question::Why {
            aggregate_id: "nonexistent".to_string(),
            max_depth: None,
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert!(answer.summary.contains("No provenance"));
    }

    #[test]
    fn impact_analyzer_scores_activity() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        // Seed many Entity events
        for i in 0..20 {
            let ev = EventEnvelope::new(
                format!("e{i}"),
                "Entity",
                "Created",
                1,
                json!({"i": i}),
            );
            kernel.event_store.append(ev).unwrap();
        }

        let question = Question::WhatMatters {
            kind: Some("Entity".to_string()),
            max_results: Some(5),
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert!(answer.confidence > 0.0);
        assert!(answer.severity == "medium" || answer.severity == "high");
    }

    #[test]
    fn risk_analyzer_identifies_high_severity() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        // Seed a finding
        kernel.objects.create(
            "finding",
            "CVE-2024-0001",
            br#"{"severity": "critical"}"#,
        );

        let question = Question::WhatIsRisky {
            min_severity: Some("medium".to_string()),
            max_results: Some(10),
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert_eq!(answer.severity, "high");
    }

    #[test]
    fn investigate_analyzer_finds_uninvestigated_cves() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        kernel.objects.create(
            "cve_record",
            "CVE-2024-9999",
            br#"{"severity": "critical"}"#,
        );

        let question = Question::WhatShouldIInvestigate {
            max_results: Some(5),
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert!(!answer.evidence.is_empty());
    }

    #[test]
    fn investigate_analyzer_no_leads() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        let question = Question::WhatShouldIInvestigate {
            max_results: Some(5),
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert!(answer.evidence.is_empty());
        assert!(answer.summary.contains("No items"));
    }

    #[test]
    fn similarity_analyzer_finds_similar_objects() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        let _id1 = kernel.objects.create("entity", "tor onion service v3", b"onion service data");
        let _id2 = kernel.objects.create("entity", "tor onion service", b"related data");
        let _id3 = kernel.objects.create("entity", "ssh server", b"completely different");

        // Find the first object
        let ref_objects = kernel.objects.find_by_label("tor onion service v3");
        assert!(!ref_objects.is_empty());
        let ref_id = ref_objects[0].id.to_string();

        let question = Question::WhatIsSimilar {
            aggregate_id: ref_id,
            kind: Some("entity".to_string()),
            max_results: Some(10),
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        // Should find at least the second object as similar
        assert_eq!(answer.evidence.len(), 1);
    }

    #[test]
    fn prediction_analyzer_projects_trends() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        for i in 0..10 {
            let ev = EventEnvelope::new(
                format!("e{i}"),
                "Entity",
                "Created",
                1,
                json!({"i": i}),
            );
            kernel.event_store.append(ev).unwrap();
        }

        let question = Question::WhatWillLikelyHappen {
            kind: Some("Entity".to_string()),
            horizon_hours: Some(24.0),
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert!(answer.summary.contains("Entity"));
    }

    #[test]
    fn prediction_no_data() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        let question = Question::WhatWillLikelyHappen {
            kind: Some("Entity".to_string()),
            horizon_hours: Some(24.0),
        };

        let answer = engine.analyze(&kernel, &question).unwrap();
        assert!(answer.summary.contains("Insufficient"));
    }

    #[test]
    fn analyze_all_returns_multiple_answers() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        let questions = vec![
            Question::WhatChanged {
                aggregate_type: None,
                since: None,
                until: None,
            },
            Question::WhatShouldIInvestigate {
                max_results: Some(3),
            },
        ];

        let answers = engine.analyze_all(&kernel, &questions).unwrap();
        assert_eq!(answers.len(), 2);
    }

    #[test]
    fn emit_decision_records_system_event() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        let answer = Answer::new(
            Question::WhatChanged {
                aggregate_type: None,
                since: None,
                until: None,
            },
            "test decision",
            0.85,
            "high",
        );

        engine
            .emit_decision(&kernel, &answer, "test_actor")
            .unwrap();

        let events = kernel.event_store.read_events(&answer.id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "Made");
    }

    #[test]
    fn emit_decision_low_confidence_skipped() {
        let kernel = Kernel::new();
        let engine = DecisionEngine::new();

        let answer = Answer::new(
            Question::WhatChanged {
                aggregate_type: None,
                since: None,
                until: None,
            },
            "test low confidence",
            0.5,
            "low",
        );

        engine
            .emit_decision(&kernel, &answer, "test_actor")
            .unwrap();

        let events = kernel.event_store.read_events(&answer.id).unwrap();
        assert!(events.is_empty());
    }
}
