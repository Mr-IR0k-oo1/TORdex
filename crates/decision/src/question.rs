//! Question and Answer types for the Decision Engine.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

/// A question the Decision Engine can answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Question {
    /// What changed in a given time window or aggregate type?
    WhatChanged {
        aggregate_type: Option<String>,
        since: Option<OffsetDateTime>,
        until: Option<OffsetDateTime>,
    },

    /// Why did a specific event or change happen?
    Why {
        aggregate_id: String,
        max_depth: Option<usize>,
    },

    /// What matters most — rank entities/objects by impact.
    WhatMatters {
        kind: Option<String>,
        max_results: Option<usize>,
    },

    /// What is risky — find high-severity findings with negative trends.
    WhatIsRisky {
        min_severity: Option<String>,
        max_results: Option<usize>,
    },

    /// What should I investigate — anomalies and unusual patterns.
    WhatShouldIInvestigate {
        max_results: Option<usize>,
    },

    /// What is similar to a given object or finding.
    WhatIsSimilar {
        aggregate_id: String,
        kind: Option<String>,
        max_results: Option<usize>,
    },

    /// What will likely happen — predictions based on historical trends.
    WhatWillLikelyHappen {
        kind: Option<String>,
        horizon_hours: Option<f64>,
    },
}

/// A supporting fact or piece of evidence for an answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: String,
    pub description: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub confidence: f64,
    pub detail: Option<Value>,
}

impl Evidence {
    #[must_use]
    pub fn new(
        kind: &str,
        description: &str,
        aggregate_id: &str,
        aggregate_type: &str,
        confidence: f64,
    ) -> Self {
        Self {
            kind: kind.to_string(),
            description: description.to_string(),
            aggregate_id: aggregate_id.to_string(),
            aggregate_type: aggregate_type.to_string(),
            confidence,
            detail: None,
        }
    }

    #[must_use]
    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(detail);
        self
    }
}

/// An answer produced by the Decision Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub id: String,
    pub question: Question,
    pub summary: String,
    pub confidence: f64,
    pub severity: String,
    pub evidence: Vec<Evidence>,
    pub recommendation: Option<String>,
    pub produced_at: OffsetDateTime,
    pub metadata: HashMap<String, String>,
}

impl Answer {
    #[must_use]
    pub fn new(question: Question, summary: &str, confidence: f64, severity: &str) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            question,
            summary: summary.to_string(),
            confidence,
            severity: severity.to_string(),
            evidence: Vec::new(),
            recommendation: None,
            produced_at: OffsetDateTime::now_utc(),
            metadata: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_evidence(mut self, evidence: Vec<Evidence>) -> Self {
        self.evidence = evidence;
        self
    }

    #[must_use]
    pub fn with_recommendation(mut self, rec: &str) -> Self {
        self.recommendation = Some(rec.to_string());
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}
