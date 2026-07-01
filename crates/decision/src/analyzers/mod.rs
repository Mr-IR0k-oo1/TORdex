//! Analyzers — each answers a category of question using kernel APIs only.

use std::collections::{HashMap, HashSet};

use serde_json::json;
use tordex_core::{Kernel, Result as CoreResult};

use crate::question::{Answer, Evidence, Question};

pub trait Analyzer: Send + Sync {
    fn name(&self) -> &str;
    fn answer(&self, kernel: &Kernel, question: &Question) -> CoreResult<Answer>;
}

// ─── Change Analyzer: "What changed?" ─────────────────────────────────────

pub struct ChangeAnalyzer;

impl ChangeAnalyzer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Analyzer for ChangeAnalyzer {
    fn name(&self) -> &str {
        "change"
    }

    fn answer(&self, kernel: &Kernel, question: &Question) -> CoreResult<Answer> {
        let (agg_type, since, until) = match question {
            Question::WhatChanged {
                aggregate_type,
                since,
                until,
            } => (aggregate_type, since, until),
            _ => return Ok(Answer::new(question.clone(), "wrong question type", 0.0, "error")),
        };

        let mut total_events = 0u64;
        let mut type_counts: HashMap<String, u64> = HashMap::new();
        let mut evidence = Vec::new();

        match agg_type {
            Some(t) => {
                let events = kernel.event_store.read_all(t)?;
                for ev in &events {
                    if let Some(s) = since {
                        if ev.timestamp < *s {
                            continue;
                        }
                    }
                    if let Some(u) = until {
                        if ev.timestamp > *u {
                            continue;
                        }
                    }
                    total_events += 1;
                    *type_counts.entry(ev.event_type.clone()).or_insert(0) += 1;
                }
            }
            None => {
                let all_types = [
                    "Entity", "Observation", "Artifact", "Evidence",
                    "Relationship", "Knowledge", "Finding", "Decision",
                    "Service", "Investigation", "Timeline", "Agent", "Monitoring",
                ];
                for t in &all_types {
                    let events = kernel.event_store.read_all(t)?;
                    let count = events.len() as u64;
                    if count > 0 {
                        type_counts.insert(t.to_string(), count);
                        total_events += count;
                    }
                }
            }
        }

        let summary = if total_events == 0 {
            "No changes detected in the requested scope.".to_string()
        } else {
            let details: Vec<String> = type_counts
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect();
            format!("Detected {total_events} changes — {}", details.join(", "))
        };

        for (et, count) in &type_counts {
            evidence.push(
                Evidence::new("event_count", &format!("{et}: {count} events"), et, "Event", 1.0)
                    .with_detail(json!({"count": count})),
            );
        }

        let confidence = if total_events > 0 { 0.95 } else { 0.5 };
        let severity = if total_events > 100 {
            "high"
        } else if total_events > 10 {
            "medium"
        } else {
            "low"
        };

        Ok(Answer::new(question.clone(), &summary, confidence, severity)
            .with_evidence(evidence))
    }
}

// ─── Why Analyzer: "Why?" ─────────────────────────────────────────────────

pub struct WhyAnalyzer;

impl WhyAnalyzer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Analyzer for WhyAnalyzer {
    fn name(&self) -> &str {
        "why"
    }

    fn answer(&self, kernel: &Kernel, question: &Question) -> CoreResult<Answer> {
        let (aggregate_id, max_depth) = match question {
            Question::Why {
                aggregate_id,
                max_depth,
            } => (aggregate_id, max_depth.unwrap_or(5)),
            _ => return Ok(Answer::new(question.clone(), "wrong question type", 0.0, "error")),
        };

        let mut evidence = Vec::new();
        let mut chain = Vec::new();

        // Find all events for this aggregate
        match kernel.event_store.read_events(aggregate_id) {
            Ok(events) => {
                for ev in &events {
                    chain.push(format!(
                        "[v{}] {} — {}",
                        ev.version, ev.event_type, ev.aggregate_type
                    ));
                    evidence.push(
                        Evidence::new(
                            "event_chain",
                            &format!("v{}: {}", ev.version, ev.event_type),
                            &ev.aggregate_id,
                            &ev.aggregate_type,
                            0.9,
                        )
                        .with_detail(ev.data.clone()),
                    );
                    if chain.len() >= max_depth {
                        break;
                    }
                }
            }
            Err(_) => {
                // Check kernel objects
                let objects = kernel.objects.find_by_label(aggregate_id);
                for obj in &objects {
                    chain.push(format!("found as object kind={}", obj.kind));
                    evidence.push(
                        Evidence::new("object_state", &format!("kind={}", obj.kind), &obj.id.to_string(), &obj.kind, 0.7)
                            .with_detail(json!({"label": obj.label, "data_size": obj.data.len()})),
                    );
                }
            }
        }

        // Check for linked objects (cause-effect relationships)
        if let Ok(id) = ulid::Ulid::from_string(aggregate_id) {
            let links = kernel.objects.links(id);
            for link in &links {
                evidence.push(
                    Evidence::new("relationship", &format!("linked via {}", link.kind), &link.target_id.to_string(), "Relationship", 0.6)
                        .with_detail(json!({"kind": link.kind})),
                );
                chain.push(format!("→ [{}] linked to {}", link.kind, link.target_id));
            }
        }

        let summary = if chain.is_empty() {
            format!("No provenance information found for '{aggregate_id}'.")
        } else {
            format!("Provenance chain ({} steps): {}", chain.len(), chain.join(" → "))
        };

        Ok(Answer::new(question.clone(), &summary, 0.85, "info").with_evidence(evidence))
    }
}

// ─── Impact Analyzer: "What matters?" / "What is risky?" ────────────────

pub struct ImpactAnalyzer;

impl ImpactAnalyzer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn score_kind(&self, kind: &str, event_count: u64, link_count: usize) -> f64 {
        let base = match kind {
            "Critical" | "critical" => 1.0,
            "High" | "high" => 0.8,
            "Medium" | "medium" => 0.5,
            "Low" | "low" => 0.2,
            _ => 0.3,
        };
        let activity = (event_count as f64).ln_1p() / 10.0;
        let connectedness = (link_count as f64).ln_1p() / 5.0;
        (base + activity + connectedness).min(1.0)
    }
}

impl Analyzer for ImpactAnalyzer {
    fn name(&self) -> &str {
        "impact"
    }

    fn answer(&self, kernel: &Kernel, question: &Question) -> CoreResult<Answer> {
        let (kind_filter, max_results, is_risky) = match question {
            Question::WhatMatters { kind, max_results } => {
                (kind.clone(), max_results.unwrap_or(10), false)
            }
            Question::WhatIsRisky {
                min_severity,
                max_results,
            } => {
                let sev = min_severity.clone().unwrap_or_else(|| "medium".to_string());
                (Some(sev), max_results.unwrap_or(10), true)
            }
            _ => return Ok(Answer::new(question.clone(), "wrong question type", 0.0, "error")),
        };

        let mut scored: Vec<(String, String, String, f64)> = Vec::new();

        // Score aggregate types by event volume
        let types_to_check = match &kind_filter {
            Some(k) => vec![k.as_str()],
            None => vec![
                "Entity", "Observation", "Artifact", "Evidence", "Finding",
                "Decision", "Relationship", "Knowledge", "Service",
            ],
        };

        for t in types_to_check {
            let count = kernel.event_store.count(t);
            if count == 0 {
                continue;
            }
            let obj_count = kernel.objects.find_by_kind(t).len();
            let base_score = self.score_kind(t, count, obj_count);

            if is_risky {
                // For risk: favor high-severity findings, CVEs, monitoring alerts
                if t == "Finding" || t == "finding" {
                    let risk_score = (base_score * 1.5).min(1.0);
                    scored.push((t.to_string(), "high_risk".to_string(), format!("{count} findings with risk"), risk_score));
                } else if t == "Monitoring" || t == "monitoring" {
                    let risk_score = (base_score * 1.3).min(1.0);
                    scored.push((t.to_string(), "monitoring_alert".to_string(), format!("{count} monitoring changes"), risk_score));
                } else {
                    scored.push((t.to_string(), "baseline".to_string(), format!("{count} events"), base_score));
                }
            } else {
                scored.push((t.to_string(), "event_volume".to_string(), format!("{count} events"), base_score));
            }
        }

        // Also score kernel objects directly
        let object_kinds = [
            "cve_record", "threat_intel", "monitoring_change", "finding",
        ];
        for kind in &object_kinds {
            let objs = kernel.objects.find_by_kind(kind);
            if objs.is_empty() {
                continue;
            }
            let score = (objs.len() as f64).ln_1p() / 20.0;
            let label = if is_risky { "risk_factor" } else { "object_volume" };
            scored.push((kind.to_string(), label.to_string(), format!("{} objects", objs.len()), score));
        }

        // Sort by score descending
        scored.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);

        let mut evidence = Vec::new();
        let mut top_items = Vec::new();

        for (name, label, desc, score) in &scored {
            evidence.push(
                Evidence::new(label, desc, name, "Aggregate", *score)
                    .with_detail(json!({"score": score})),
            );
            top_items.push(format!("{name} ({desc}, score={score:.2})"));
        }

        let label = if is_risky { "risk analysis" } else { "impact analysis" };
        let summary = format!(
            "{}: top {} items — {}",
            label,
            scored.len(),
            top_items.join("; ")
        );
        let severity = if is_risky { "high" } else { "medium" };

        Ok(Answer::new(question.clone(), &summary, 0.8, severity).with_evidence(evidence))
    }
}

// ─── Investigate Analyzer: "What should I investigate?" ──────────────────

pub struct InvestigateAnalyzer;

impl InvestigateAnalyzer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Analyzer for InvestigateAnalyzer {
    fn name(&self) -> &str {
        "investigate"
    }

    fn answer(&self, kernel: &Kernel, question: &Question) -> CoreResult<Answer> {
        let max_results = match question {
            Question::WhatShouldIInvestigate { max_results } => max_results.unwrap_or(5),
            _ => return Ok(Answer::new(question.clone(), "wrong question type", 0.0, "error")),
        };

        let mut leads: Vec<(String, String, String, f64)> = Vec::new();

        // 1. High-severity CVEs with no investigation yet
        let cves = kernel.objects.find_by_kind("cve_record");
        let investigations = kernel.objects.find_by_kind("investigation");
        let investigated_ids: HashSet<String> = investigations
            .iter()
            .filter_map(|o| {
                serde_json::from_slice::<serde_json::Value>(&o.data)
                    .ok()
                    .and_then(|v| v.get("cve_id").and_then(|c| c.as_str().map(String::from)))
            })
            .collect();

        for cve in &cves {
            if !investigated_ids.contains(&cve.label) {
                if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&cve.data) {
                    let severity = data.get("severity").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let score = match severity {
                        "critical" => 1.0,
                        "high" => 0.8,
                        "medium" => 0.5,
                        _ => 0.2,
                    };
                    leads.push(("cve".to_string(), cve.label.clone(), format!("uninvestigated CVE: {} (severity: {})", cve.label, severity), score));
                }
            }
        }

        // 2. Monitoring changes indicating unreachable services
        let changes = kernel.objects.find_by_kind("monitoring_change");
        for change in &changes {
            if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&change.data) {
                let change_type = data.get("change_type").and_then(|v| v.as_str()).unwrap_or("");
                if change_type == "poll_failed" || change_type == "unreachable" {
                    let subject = data.get("subject").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let score = 0.7;
                    leads.push(("service_outage".to_string(), subject.to_string(), format!("unreachable: {subject}"), score));
                }
            }
        }

        // 3. Threat intelligence items
        let threats = kernel.objects.find_by_kind("threat_intel");
        for t in &threats {
            if !investigated_ids.contains(&t.label) {
                let score = 0.75;
                leads.push(("threat".to_string(), t.label.clone(), format!("unassessed threat: {}", t.label), score));
            }
        }

        // 4. Findings with highest severity
        let findings = kernel.objects.find_by_kind("finding");
        for f in &findings {
            if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&f.data) {
                let severity = data.get("severity").and_then(|v| v.as_str()).unwrap_or("low");
                let score = match severity {
                    "Critical" | "critical" => 0.9,
                    "High" | "high" => 0.7,
                    _ => 0.0,
                };
                if score > 0.0 {
                    leads.push(("finding".to_string(), f.label.clone(), format!("{severity} finding: {}", f.label), score));
                }
            }
        }

        // Sort by score desc, take top N
        leads.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        leads.truncate(max_results);

        let mut evidence = Vec::new();
        let mut items = Vec::new();

        for (kind, id, desc, score) in &leads {
            evidence.push(Evidence::new("investigation_lead", desc, id, kind, *score));
            items.push(format!("{desc} (confidence={score:.2})"));
        }

        let summary = if items.is_empty() {
            "No items requiring investigation found.".to_string()
        } else {
            format!("Investigation leads ({}): {}", leads.len(), items.join("; "))
        };

        Ok(Answer::new(question.clone(), &summary, 0.85, "medium").with_evidence(evidence))
    }
}

// ─── Similarity Analyzer: "What is similar?" ─────────────────────────────

pub struct SimilarityAnalyzer;

impl SimilarityAnalyzer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn text_similarity(&self, a: &str, b: &str) -> f64 {
        if a.is_empty() && b.is_empty() {
            return 1.0;
        }
        let tokens_a: HashSet<&str> = a.split_whitespace().collect();
        let tokens_b: HashSet<&str> = b.split_whitespace().collect();
        let intersection = tokens_a.intersection(&tokens_b).count();
        let union = tokens_a.union(&tokens_b).count();
        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }
}

impl Analyzer for SimilarityAnalyzer {
    fn name(&self) -> &str {
        "similarity"
    }

    fn answer(&self, kernel: &Kernel, question: &Question) -> CoreResult<Answer> {
        let (aggregate_id, kind_filter, max_results) = match question {
            Question::WhatIsSimilar {
                aggregate_id,
                kind,
                max_results,
            } => (aggregate_id, kind, max_results.unwrap_or(10)),
            _ => return Ok(Answer::new(question.clone(), "wrong question type", 0.0, "error")),
        };

        let mut evidence = Vec::new();
        let mut similarities: Vec<(String, String, f64)> = Vec::new();

        // Find the reference object
        let ref_object = kernel.objects.read(
            ulid::Ulid::from_string(aggregate_id).unwrap_or(ulid::Ulid::nil()),
        );

        if let Some(ref_obj) = ref_object {
            let ref_label = &ref_obj.label;
            let ref_data_str = String::from_utf8_lossy(&ref_obj.data);

            // Compare against all objects of the same kind (or all kinds)
            let kinds_to_check = match kind_filter {
                Some(k) => vec![k.clone()],
                None => {
                    let mut kinds = Vec::new();
                    // Check known kinds that have objects
                    for k in &[
                        "entity", "observation", "artifact", "evidence", "finding",
                        "cve_record", "threat_intel", "monitoring_change",
                    ] {
                        if !kernel.objects.find_by_kind(k).is_empty() {
                            kinds.push(k.to_string());
                        }
                    }
                    if kinds.is_empty() {
                        kinds.push(ref_obj.kind.clone());
                    }
                    kinds
                }
            };

            for kind in &kinds_to_check {
                let objects = kernel.objects.find_by_kind(kind);
                for obj in &objects {
                    if obj.id == ref_obj.id {
                        continue;
                    }
                    let obj_data_str = String::from_utf8_lossy(&obj.data);

                    // Label similarity
                    let label_sim = self.text_similarity(ref_label, &obj.label);
                    // Data similarity (rough content overlap)
                    let data_sim = self.text_similarity(&ref_data_str, &obj_data_str);

                    let combined = (label_sim * 0.4 + data_sim * 0.6).max(0.0);
                    if combined > 0.1 {
                        similarities.push((obj.id.to_string(), obj.label.clone(), combined));
                    }
                }
            }

            // Sort by similarity desc
            similarities.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            similarities.truncate(max_results);

            for (id, label, score) in &similarities {
                evidence.push(
                    Evidence::new("similar_object", &format!("{label} (similarity={score:.2})"), id, "Object", *score),
                );
            }
        } else {
            // Try finding by label
            let objects = kernel.objects.find_by_label(aggregate_id);
            if let Some(obj) = objects.first() {
                evidence.push(
                    Evidence::new("reference", &format!("found by label: {}", obj.label), &obj.id.to_string(), &obj.kind, 0.8),
                );
            }
        }

        let summary = if similarities.is_empty() {
            format!("No similar objects found for '{aggregate_id}'.")
        } else {
            format!(
                "Found {} similar objects — {}",
                similarities.len(),
                similarities
                    .iter()
                    .map(|(_, l, s)| format!("{l} ({s:.2})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        Ok(Answer::new(question.clone(), &summary, 0.75, "info").with_evidence(evidence))
    }
}

// ─── Prediction Analyzer: "What will likely happen?" ─────────────────────

pub struct PredictionAnalyzer;

impl PredictionAnalyzer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Analyzer for PredictionAnalyzer {
    fn name(&self) -> &str {
        "predict"
    }

    fn answer(&self, kernel: &Kernel, question: &Question) -> CoreResult<Answer> {
        let (kind_filter, horizon_hours) = match question {
            Question::WhatWillLikelyHappen {
                kind,
                horizon_hours,
            } => (kind, horizon_hours.unwrap_or(24.0)),
            _ => return Ok(Answer::new(question.clone(), "wrong question type", 0.0, "error")),
        };

        let mut evidence = Vec::new();
        let mut predictions: Vec<String> = Vec::new();

        let types_to_analyze = match kind_filter {
            Some(k) => vec![k.clone()],
            None => vec![
                "Entity".to_string(),
                "Observation".to_string(),
                "Artifact".to_string(),
                "Evidence".to_string(),
                "Finding".to_string(),
                "Relationship".to_string(),
                "Monitoring".to_string(),
            ],
        };

        for t in &types_to_analyze {
            let count = kernel.event_store.count(t) as f64;
            if count == 0.0 {
                continue;
            }

            // Simple trend: project current event rate forward
            // Assumes events are roughly linear over time
            let projected = count * (1.0 + horizon_hours / 8760.0); // yearly linear projection scaled to hours
            let delta = projected - count;

            let direction = if delta > 1.0 {
                "increasing"
            } else if delta < -1.0 {
                "decreasing"
            } else {
                "stable"
            };

            predictions.push(format!(
                "{t}: ~{projected:.0} events in {horizon_hours}h ({direction}, +{delta:.1})"
            ));

            evidence.push(
                Evidence::new(
                    "trend_projection",
                    &format!("{t}: {count} → ~{projected:.0} ({direction})"),
                    t,
                    "Aggregate",
                    0.6,
                )
                .with_detail(json!({
                    "current": count,
                    "projected": projected,
                    "delta": delta,
                    "direction": direction,
                    "horizon_hours": horizon_hours,
                })),
            );
        }

        // Check for growth patterns in kernel objects
        let object_growth_kinds = [
            "cve_record", "threat_intel", "monitoring_change", "finding",
        ];
        for kind in &object_growth_kinds {
            let objs = kernel.objects.find_by_kind(kind);
            if objs.len() > 1 {
                let growth_rate = (objs.len() as f64).ln_1p() / 10.0;
                let direction = if growth_rate > 0.3 { "accelerating" } else { "steady" };
                predictions.push(format!("{kind}: {} objects ({direction})", objs.len()));
                evidence.push(
                    Evidence::new("object_growth", &format!("{kind}: {} objs ({direction})", objs.len()), kind, "Object", growth_rate.min(1.0)),
                );
            }
        }

        let summary = if predictions.is_empty() {
            "Insufficient data for prediction.".to_string()
        } else {
            format!("Predictions ({horizon_hours}h horizon): {}", predictions.join("; "))
        };

        let confidence = if predictions.len() > 2 { 0.6 } else { 0.4 };

        Ok(Answer::new(question.clone(), &summary, confidence, "info").with_evidence(evidence))
    }
}
