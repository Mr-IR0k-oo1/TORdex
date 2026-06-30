use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ulid::Ulid;
use serde::{Deserialize, Serialize};

pub type PolicyId = Ulid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub id: PolicyId,
    pub name: String,
    pub action: String,
    pub resource: String,
    pub effect: Decision,
}

pub trait PolicyManager: Send + Sync {
    fn evaluate(&self, action: &str, resource: &str) -> Decision;
    fn register(&self, policy: Policy) -> PolicyId;
    fn remove(&self, id: PolicyId) -> bool;
    fn list(&self) -> Vec<Policy>;
}

pub struct AllowAllPolicy {
    policies: Arc<Mutex<HashMap<PolicyId, Policy>>>,
}

impl AllowAllPolicy {
    #[must_use]
    pub fn new() -> Self {
        Self {
            policies: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for AllowAllPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyManager for AllowAllPolicy {
    fn evaluate(&self, _action: &str, _resource: &str) -> Decision {
        Decision::Allow
    }

    fn register(&self, policy: Policy) -> PolicyId {
        let id = policy.id;
        self.policies.lock().unwrap().insert(id, policy);
        id
    }

    fn remove(&self, id: PolicyId) -> bool {
        self.policies.lock().unwrap().remove(&id).is_some()
    }

    fn list(&self) -> Vec<Policy> {
        self.policies.lock().unwrap().values().cloned().collect()
    }
}

pub struct DenyAllPolicy {
    policies: Arc<Mutex<HashMap<PolicyId, Policy>>>,
}

impl DenyAllPolicy {
    #[must_use]
    pub fn new() -> Self {
        Self {
            policies: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for DenyAllPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyManager for DenyAllPolicy {
    fn evaluate(&self, _action: &str, _resource: &str) -> Decision {
        Decision::Deny
    }

    fn register(&self, policy: Policy) -> PolicyId {
        let id = policy.id;
        self.policies.lock().unwrap().insert(id, policy);
        id
    }

    fn remove(&self, id: PolicyId) -> bool {
        self.policies.lock().unwrap().remove(&id).is_some()
    }

    fn list(&self) -> Vec<Policy> {
        self.policies.lock().unwrap().values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_all_by_default() {
        let pm = AllowAllPolicy::new();
        assert_eq!(
            pm.evaluate("collect", "http://example.com"),
            Decision::Allow
        );
    }

    #[test]
    fn deny_all_by_default() {
        let pm = DenyAllPolicy::new();
        assert_eq!(
            pm.evaluate("collect", "http://example.com"),
            Decision::Deny
        );
    }

    #[test]
    fn register_and_list() {
        let pm = AllowAllPolicy::new();
        let policy = Policy {
            id: PolicyId::new(),
            name: "test".into(),
            action: "read".into(),
            resource: "*".into(),
            effect: Decision::Allow,
        };
        pm.register(policy.clone());
        assert_eq!(pm.list().len(), 1);
    }
}
