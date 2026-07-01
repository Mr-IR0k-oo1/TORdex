//! libtordex — Intelligence Kernel microkernel.
//!
//! Pure-primitive foundation with no knowledge of Tor, HTTP, or any I/O protocol.
//! Thirteen modules, each exposing a trait + default in-memory implementation.

#![allow(clippy::module_name_repetitions)]

pub mod agent;
pub mod driver;
pub mod error;
pub mod event;
pub mod processor;
pub mod event_store;
pub mod id;
pub mod ipc;
pub mod memory;
pub mod object;
pub mod policy;
pub mod plugin;
pub mod scheduler;
pub mod security;
pub mod storage;
pub mod time;

pub use agent::{
    Agent, AgentId, AgentManifest, AgentRuntime, AgentStatus, InMemoryAgentRuntime,
};
pub use error::CoreError;
pub use id::{
    ArtifactId, CollectionId, DecisionId, EntityId, EventId, EvidenceId, FindingId,
    InvestigationId, KnowledgeId, ObservationId, RelationshipId, ServiceId, SessionId, SourceId,
    TimelineId,
};
pub use time::now;

pub use driver::{
    Capability, Driver, DriverError, DriverInfo, DriverRegistry, InMemoryDriverRegistry,
};
pub use processor::{
    DerivedArtifact, InMemoryProcessorRegistry, ProcessedObservation, Processor, ProcessorError,
    ProcessorRegistry,
};
pub use event::{Event, EventManager, InMemoryEventManager};
pub use event_store::{
    Aggregate, EventEnvelope, EventStore, EventStoreError, InMemoryEventStore,
    InMemorySnapshotStore, Projector, Snapshot, SnapshotError, SnapshotStore, SystemEvent,
};
pub use ipc::{Ipc, InMemoryIpc, Message};
pub use memory::{MemoryManager, MemoryStats, TrackingAllocator};
pub use object::{InMemoryObjectManager, Link, Object, ObjectManager};
pub use policy::{AllowAllPolicy, Decision, DenyAllPolicy, Policy, PolicyManager};
pub use plugin::{InMemoryPluginManager, Plugin, PluginDescriptor, PluginManager};
pub use scheduler::{Scheduler, SimpleScheduler, TaskId, TaskStatus};
pub use security::{BasicSecurity, Identity, IdentityId, SecurityManager};
pub use storage::{Entry, InMemoryStorage, StorageManager};

/// Crate-wide result alias.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;

/// Microkernel composition root.
///
/// Holds one instance of every kernel module. Each module is boxed so
/// implementations can be swapped at runtime.
pub struct Kernel {
    pub scheduler: Box<dyn Scheduler>,
    pub memory: Box<dyn MemoryManager>,
    pub event: Box<dyn EventManager>,
    pub event_store: Box<dyn EventStore>,
    pub snapshots: Box<dyn SnapshotStore>,
    pub drivers: Box<dyn DriverRegistry>,
    pub policy: Box<dyn PolicyManager>,
    pub storage: Box<dyn StorageManager>,
    pub security: Box<dyn SecurityManager>,
    pub plugin: Box<dyn PluginManager>,
    pub ipc: Box<dyn Ipc>,
    pub objects: Box<dyn ObjectManager>,
    pub agents: Box<dyn AgentRuntime>,
}

impl Kernel {
    #[must_use]
    pub fn new() -> Self {
        Self {
            scheduler: Box::new(SimpleScheduler::new()),
            memory: Box::new(TrackingAllocator::new()),
            event: Box::new(InMemoryEventManager::new()),
            event_store: Box::new(InMemoryEventStore::new()),
            snapshots: Box::new(InMemorySnapshotStore::new()),
            drivers: Box::new(InMemoryDriverRegistry::new()),
            policy: Box::new(AllowAllPolicy::new()),
            storage: Box::new(InMemoryStorage::new()),
            security: Box::new(BasicSecurity::new()),
            plugin: Box::new(InMemoryPluginManager::new()),
            ipc: Box::new(InMemoryIpc::new()),
            objects: Box::new(InMemoryObjectManager::new()),
            agents: Box::new(InMemoryAgentRuntime::new()),
        }
    }
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_default_creates_all_modules() {
        let kernel = Kernel::new();
        assert_eq!(kernel.scheduler.running_count(), 0);
        let stats = kernel.memory.stats();
        assert_eq!(stats.live_allocations, 0);
        let sub = kernel.event.subscribe("test");
        kernel.event.publish("test", b"msg");
        assert_eq!(kernel.event.poll(sub).len(), 1);
        assert_eq!(
            kernel.policy.evaluate("any", "resource"),
            Decision::Allow
        );
        assert!(!kernel.storage.exists("x"));
        assert_eq!(kernel.security.hash(b"a"), kernel.security.hash(b"a"));
        assert!(kernel.plugin.list().is_empty());
        let ep = kernel.ipc.register_endpoint();
        assert_eq!(kernel.ipc.pending_count(ep), 0);
        assert!(kernel.objects.find_by_kind("x").is_empty());
        assert!(kernel.drivers.list().is_empty());
    }
}
