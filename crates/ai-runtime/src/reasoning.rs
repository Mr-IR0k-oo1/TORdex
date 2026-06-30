//! Lightweight forward-chaining reasoning engine.
//!
//! Defines facts and rules, then chains through them to derive
//! new conclusions.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A fact in the knowledge base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub predicate: String,
    pub subject: String,
    pub object: Option<String>,
    pub confidence: f64,
}

impl Fact {
    #[must_use]
    pub fn new(predicate: &str, subject: &str) -> Self {
        Self {
            predicate: predicate.to_string(),
            subject: subject.to_string(),
            object: None,
            confidence: 1.0,
        }
    }

    #[must_use]
    pub fn with_object(predicate: &str, subject: &str, object: &str) -> Self {
        Self {
            predicate: predicate.to_string(),
            subject: subject.to_string(),
            object: Some(object.to_string()),
            confidence: 1.0,
        }
    }

    /// Check whether this fact matches a pattern.
    /// `subject` and `object` can be `"*"` to match anything.
    /// A value starting with `$` is treated as a variable binding.
    #[must_use]
    pub fn matches_pattern(
        &self,
        predicate: &str,
        subject: &str,
        object: Option<&str>,
        bindings: &mut HashMap<String, String>,
    ) -> bool {
        if self.predicate != predicate {
            return false;
        }
        if !Self::match_term(&self.subject, subject, "subject", bindings) {
            return false;
        }
        match object {
            Some(obj) => {
                let self_obj = self.object.as_deref().unwrap_or("");
                Self::match_term(self_obj, obj, "object", bindings)
            }
            None => self.object.is_none(),
        }
    }

    fn match_term(
        actual: &str,
        pattern: &str,
        _var_name: &str,
        bindings: &mut HashMap<String, String>,
    ) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some(var) = pattern.strip_prefix('$') {
            match bindings.get(var) {
                Some(existing) => actual == existing,
                None => {
                    bindings.insert(var.to_string(), actual.to_string());
                    true
                }
            }
        } else {
            actual == pattern
        }
    }
}

/// A logical rule: if all conditions match, the conclusion is inferred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    /// Conditions are (predicate, subject, object) triples.
    /// subject/object can be `"*"` to match any value, or `"$VAR"` to
    /// bind a variable that must be consistent across conditions.
    pub conditions: Vec<(String, String, Option<String>)>,
    /// (predicate, subject, object) — can reference bound variables.
    pub conclusion: (String, String, Option<String>),
    /// Confidence multiplier applied to the inferred fact.
    pub confidence: f64,
}

impl Rule {
    #[must_use]
    pub fn new(
        name: &str,
        conditions: Vec<(&str, &str, Option<&str>)>,
        conclusion: (&str, &str, Option<&str>),
        confidence: f64,
    ) -> Self {
        Self {
            name: name.to_string(),
            conditions: conditions
                .into_iter()
                .map(|(p, s, o)| (p.to_string(), s.to_string(), o.map(|v| v.to_string())))
                .collect(),
            conclusion: (
                conclusion.0.to_string(),
                conclusion.1.to_string(),
                conclusion.2.map(|v| v.to_string()),
            ),
            confidence,
        }
    }
}

/// A step in the reasoning chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    pub rule: String,
    pub derived_fact: Fact,
    pub source_facts: Vec<Fact>,
}

/// Result of reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningResult {
    pub conclusions: Vec<Fact>,
    pub chain: Vec<ReasoningStep>,
    pub iteration_count: usize,
}

/// Forward-chaining reasoning engine.
#[derive(Debug, Clone)]
pub struct ReasoningEngine {
    rules: Vec<Rule>,
    max_iterations: usize,
}

impl ReasoningEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            max_iterations: 10,
        }
    }

    /// Create with a set of rules.
    #[must_use]
    pub fn with_rules(rules: Vec<Rule>) -> Self {
        Self {
            rules,
            max_iterations: 10,
        }
    }

    /// Add a rule to the engine.
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Run forward chaining from the given facts.
    #[must_use]
    pub fn reason(&self, facts: &[Fact]) -> ReasoningResult {
        let mut known: Vec<Fact> = facts.to_vec();
        let mut chain: Vec<ReasoningStep> = Vec::new();
        let mut iteration = 0;

        loop {
            if iteration >= self.max_iterations {
                break;
            }
            let mut new_facts: Vec<Fact> = Vec::new();

            for rule in &self.rules {
                let bindings = self.find_all_bindings(&rule.conditions, &known);

                for binding in &bindings {
                    let pred = self.substitute(&rule.conclusion.0, binding);
                    let subj = self.substitute(&rule.conclusion.1, binding);
                    let obj = rule
                        .conclusion
                        .2
                        .as_ref()
                        .map(|o| self.substitute(o, binding));

                    let inferred = Fact {
                        predicate: pred,
                        subject: subj,
                        object: obj,
                        confidence: rule.confidence,
                    };

                    if !known.contains(&inferred) && !new_facts.contains(&inferred) {
                        let source_facts: Vec<Fact> = known
                            .iter()
                            .filter(|kf| {
                                rule.conditions.iter().any(|(p, s, o)| {
                                    let mut b = HashMap::new();
                                    kf.matches_pattern(p, s, o.as_deref(), &mut b)
                                })
                            })
                            .cloned()
                            .collect();

                        chain.push(ReasoningStep {
                            rule: rule.name.clone(),
                            derived_fact: inferred.clone(),
                            source_facts,
                        });
                        new_facts.push(inferred);
                    }
                }
            }

            if new_facts.is_empty() {
                break;
            }

            known.append(&mut new_facts);
            iteration += 1;
        }

        let conclusions: Vec<Fact> = known
            .iter()
            .filter(|f| !facts.contains(f))
            .cloned()
            .collect();

        ReasoningResult {
            conclusions,
            chain,
            iteration_count: iteration,
        }
    }

    /// Find all variable bindings that satisfy all conditions.
    fn find_all_bindings(
        &self,
        conditions: &[(String, String, Option<String>)],
        facts: &[Fact],
    ) -> Vec<HashMap<String, String>> {
        let mut results = vec![HashMap::new()];

        for (pred, subj, obj) in conditions {
            let mut next_results = Vec::new();

            for bindings in &results {
                let mut found = false;

                for fact in facts {
                    let mut candidate = bindings.clone();
                    if fact.matches_pattern(pred, subj, obj.as_deref(), &mut candidate) {
                        found = true;
                        next_results.push(candidate);
                    }
                }

                if !found {
                    // This branch can't satisfy all conditions — don't propagate
                }
            }

            results = next_results;
            if results.is_empty() {
                break;
            }
        }

        results
    }

    fn substitute(&self, val: &str, bindings: &HashMap<String, String>) -> String {
        if let Some(var) = val.strip_prefix('$') {
            bindings.get(var).cloned().unwrap_or_else(|| val.to_string())
        } else {
            val.to_string()
        }
    }
}

impl Default for ReasoningEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fact_creation_and_matching() {
        let fact = Fact::with_object("is_a", "Rust", "language");
        let mut bindings = HashMap::new();
        assert!(fact.matches_pattern("is_a", "Rust", Some("language"), &mut bindings));
        assert!(fact.matches_pattern("is_a", "*", Some("language"), &mut bindings));
    }

    #[test]
    fn fact_variable_binding() {
        let fact = Fact::with_object("is_a", "Rust", "language");
        let mut bindings = HashMap::new();
        assert!(fact.matches_pattern("is_a", "$X", Some("language"), &mut bindings));
        assert_eq!(bindings.get("X").unwrap(), "Rust");
    }

    #[test]
    fn simple_forward_chaining() {
        let rules = vec![Rule::new(
            "transitive_is_a",
            vec![
                ("is_a", "$X", Some("$Y")),
                ("is_a", "$Y", Some("$Z")),
            ],
            ("is_a", "$X", Some("$Z")),
            0.9,
        )];

        let engine = ReasoningEngine::with_rules(rules);
        let facts = vec![
            Fact::with_object("is_a", "Rust", "systems_language"),
            Fact::with_object("is_a", "systems_language", "programming_language"),
        ];

        let result = engine.reason(&facts);
        assert!(!result.conclusions.is_empty());
        let has_transitive = result
            .conclusions
            .iter()
            .any(|f| f.predicate == "is_a" && f.subject == "Rust");
        assert!(has_transitive);
    }

    #[test]
    fn no_rules_no_new_facts() {
        let engine = ReasoningEngine::new();
        let facts = vec![Fact::new("test", "value")];
        let result = engine.reason(&facts);
        assert!(result.conclusions.is_empty());
    }

    #[test]
    fn reasoning_result_serialization() {
        let result = ReasoningResult {
            conclusions: vec![Fact::new("derived", "value")],
            chain: Vec::new(),
            iteration_count: 1,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: ReasoningResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.conclusions.len(), 1);
    }

    #[test]
    fn multiple_rules_chain() {
        let rules = vec![
            Rule::new(
                "implies_danger",
                vec![("has_vulnerability", "$X", None)],
                ("is", "$X", Some("dangerous")),
                0.8,
            ),
            Rule::new(
                "needs_attention",
                vec![("is", "$X", Some("dangerous"))],
                ("action_required", "$X", Some("patch")),
                0.9,
            ),
        ];

        let engine = ReasoningEngine::with_rules(rules);
        let facts = vec![Fact::new("has_vulnerability", "libfoo")];
        let result = engine.reason(&facts);
        assert!(result.conclusions.iter().any(|f| f.predicate == "action_required"));
    }
}
