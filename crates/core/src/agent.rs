//! Agent Runtime — agents become users of the kernel.
//!
//! Agents are autonomous units that interact with the system exclusively
//! through kernel APIs. They have no direct database access.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::event_store::EventEnvelope;
use crate::{CoreError, Kernel, Result};

pub type AgentId = Ulid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Running,
    Busy,
    Failed(String),
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub id: AgentId,
    pub name: String,
    pub kind: String,
    pub version: String,
    pub description: String,
    pub status: AgentStatus,
}

/// An autonomous agent that operates through kernel APIs only.
///
/// Every agent declares its identity via `manifest()` and implements
/// lifecycle hooks. The `handle_event` method allows agents to react
/// to domain events published through the event store.
pub trait Agent: Send + Sync {
    fn manifest(&self) -> AgentManifest;

    /// Called once when the agent is first registered with the runtime.
    fn init(&self, _kernel: &Kernel) -> Result<()> {
        Ok(())
    }

    /// Start the agent's background work.
    fn start(&self, _kernel: &Kernel) -> Result<()> {
        Ok(())
    }

    /// Periodic tick — called on every `tick_all` cycle.
    fn tick(&self, _kernel: &Kernel) -> Result<()> {
        Ok(())
    }

    /// Stop the agent cleanly.
    fn stop(&self, _kernel: &Kernel) -> Result<()> {
        Ok(())
    }

    /// React to a domain event from the event store.
    fn handle_event(&self, _kernel: &Kernel, _event: &EventEnvelope) -> Result<()> {
        Ok(())
    }
}

/// Manages agent lifecycle within the kernel.
pub trait AgentRuntime: Send + Sync {
    fn register(&self, agent: Box<dyn Agent>) -> Result<()>;
    fn unregister(&self, id: AgentId) -> Result<()>;
    fn start_agent(&self, id: AgentId, kernel: &Kernel) -> Result<()>;
    fn stop_agent(&self, id: AgentId, kernel: &Kernel) -> Result<()>;
    fn get(&self, id: AgentId) -> Option<Arc<dyn Agent>>;
    fn list(&self) -> Vec<AgentManifest>;
    fn status(&self, id: AgentId) -> Option<AgentStatus>;
    fn tick_all(&self, kernel: &Kernel) -> Result<()>;
    fn dispatch_event(&self, kernel: &Kernel, event: &EventEnvelope) -> Result<()>;
}

// ─── InMemoryAgentRuntime ───────────────────────────────────────────────────

struct RuntimeInner {
    agents: HashMap<AgentId, Arc<dyn Agent>>,
}

pub struct InMemoryAgentRuntime {
    inner: Arc<Mutex<RuntimeInner>>,
}

impl InMemoryAgentRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RuntimeInner {
                agents: HashMap::new(),
            })),
        }
    }
}

impl Default for InMemoryAgentRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRuntime for InMemoryAgentRuntime {
    fn register(&self, agent: Box<dyn Agent>) -> Result<()> {
        let manifest = agent.manifest();
        let id = manifest.id;
        let mut inner = self.inner.lock().unwrap();
        if inner.agents.contains_key(&id) {
            return Err(CoreError::agent(format!(
                "agent '{}' already registered",
                manifest.name
            )));
        }
        inner.agents.insert(id, Arc::from(agent));
        Ok(())
    }

    fn unregister(&self, id: AgentId) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner
            .agents
            .remove(&id)
            .ok_or_else(|| CoreError::agent(format!("agent '{}' not found", id)))?;
        Ok(())
    }

    fn start_agent(&self, id: AgentId, kernel: &Kernel) -> Result<()> {
        let agent = {
            let inner = self.inner.lock().unwrap();
            inner.agents.get(&id).cloned()
        };
        match agent {
            Some(a) => a.start(kernel),
            None => Err(CoreError::agent(format!("agent '{}' not found", id))),
        }
    }

    fn stop_agent(&self, id: AgentId, kernel: &Kernel) -> Result<()> {
        let agent = {
            let inner = self.inner.lock().unwrap();
            inner.agents.get(&id).cloned()
        };
        match agent {
            Some(a) => a.stop(kernel),
            None => Err(CoreError::agent(format!("agent '{}' not found", id))),
        }
    }

    fn get(&self, id: AgentId) -> Option<Arc<dyn Agent>> {
        self.inner.lock().unwrap().agents.get(&id).cloned()
    }

    fn list(&self) -> Vec<AgentManifest> {
        self.inner
            .lock()
            .unwrap()
            .agents
            .values()
            .map(|a| a.manifest())
            .collect()
    }

    fn status(&self, id: AgentId) -> Option<AgentStatus> {
        self.inner
            .lock()
            .unwrap()
            .agents
            .get(&id)
            .map(|a| a.manifest().status)
    }

    fn tick_all(&self, kernel: &Kernel) -> Result<()> {
        let agents: Vec<Arc<dyn Agent>> = self
            .inner
            .lock()
            .unwrap()
            .agents
            .values()
            .cloned()
            .collect();
        for agent in &agents {
            agent.tick(kernel)?;
        }
        Ok(())
    }

    fn dispatch_event(&self, kernel: &Kernel, event: &EventEnvelope) -> Result<()> {
        let agents: Vec<Arc<dyn Agent>> = self
            .inner
            .lock()
            .unwrap()
            .agents
            .values()
            .cloned()
            .collect();
        for agent in &agents {
            agent.handle_event(kernel, event)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Kernel;

    struct TestAgent {
        id: AgentId,
    }

    impl TestAgent {
        fn new() -> Self {
            Self {
                id: AgentId::new(),
            }
        }
    }

    impl Agent for TestAgent {
        fn manifest(&self) -> AgentManifest {
            AgentManifest {
                id: self.id,
                name: "test".into(),
                kind: "test".into(),
                version: "0.1.0".into(),
                description: "test agent".into(),
                status: AgentStatus::Idle,
            }
        }
    }

    #[test]
    fn register_and_list() {
        let runtime = InMemoryAgentRuntime::new();
        let agent = TestAgent::new();
        let id = agent.id;
        runtime.register(Box::new(agent)).unwrap();
        let list = runtime.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test");
        assert!(runtime.get(id).is_some());
    }

    #[test]
    fn unregister_agent() {
        let runtime = InMemoryAgentRuntime::new();
        let agent = TestAgent::new();
        let id = agent.id;
        runtime.register(Box::new(agent)).unwrap();
        runtime.unregister(id).unwrap();
        assert!(runtime.list().is_empty());
    }

    #[test]
    fn duplicate_register_errors() {
        let runtime = InMemoryAgentRuntime::new();
        let _agent_a = TestAgent::new();
        let _agent_b = TestAgent::new();
        assert!(runtime.unregister(AgentId::new()).is_err());
    }

    #[test]
    fn start_and_stop_agent() {
        let runtime = InMemoryAgentRuntime::new();
        let kernel = Kernel::new();
        let agent = TestAgent::new();
        let id = agent.id;
        runtime.register(Box::new(agent)).unwrap();
        runtime.start_agent(id, &kernel).unwrap();
        runtime.stop_agent(id, &kernel).unwrap();
    }

    #[test]
    fn tick_all_and_dispatch_event() {
        let runtime = InMemoryAgentRuntime::new();
        let kernel = Kernel::new();
        let agent = TestAgent::new();
        runtime.register(Box::new(agent)).unwrap();
        runtime.tick_all(&kernel).unwrap();
        let envelope = crate::event_store::EventEnvelope::new(
            "test".into(),
            "Test",
            "Created",
            1,
            serde_json::json!({}),
        );
        runtime.dispatch_event(&kernel, &envelope).unwrap();
    }

    #[test]
    fn status_of_missing_agent() {
        let runtime = InMemoryAgentRuntime::new();
        assert!(runtime.status(AgentId::new()).is_none());
    }
}
