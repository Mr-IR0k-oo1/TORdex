use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ulid::Ulid;

pub type PluginId = Ulid;

#[derive(Debug, Clone)]
pub struct PluginDescriptor {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<String>,
}

pub trait Plugin: Send + Sync {
    fn descriptor(&self) -> PluginDescriptor;
    fn start(&self) -> Result<(), String>;
    fn stop(&self) -> Result<(), String>;
}

pub trait PluginManager: Send + Sync {
    fn register(&self, plugin: Box<dyn Plugin>) -> PluginId;
    fn unregister(&self, id: PluginId) -> bool;
    fn get(&self, id: PluginId) -> Option<PluginDescriptor>;
    fn list(&self) -> Vec<PluginDescriptor>;
    fn start_all(&self) -> Vec<PluginId>;
    fn stop_all(&self) -> Vec<PluginId>;
}

struct PluginEntry {
    plugin: Box<dyn Plugin>,
    descriptor: PluginDescriptor,
}

pub struct InMemoryPluginManager {
    plugins: Arc<Mutex<HashMap<PluginId, PluginEntry>>>,
}

impl InMemoryPluginManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryPluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager for InMemoryPluginManager {
    fn register(&self, plugin: Box<dyn Plugin>) -> PluginId {
        let desc = plugin.descriptor();
        let id = desc.id;
        let entry = PluginEntry {
            plugin,
            descriptor: desc,
        };
        self.plugins.lock().unwrap().insert(id, entry);
        id
    }

    fn unregister(&self, id: PluginId) -> bool {
        self.plugins.lock().unwrap().remove(&id).is_some()
    }

    fn get(&self, id: PluginId) -> Option<PluginDescriptor> {
        self.plugins
            .lock()
            .unwrap()
            .get(&id)
            .map(|e| e.descriptor.clone())
    }

    fn list(&self) -> Vec<PluginDescriptor> {
        self.plugins
            .lock()
            .unwrap()
            .values()
            .map(|e| e.descriptor.clone())
            .collect()
    }

    fn start_all(&self) -> Vec<PluginId> {
        let ids: Vec<PluginId> = {
            let guard = self.plugins.lock().unwrap();
            guard.values().map(|e| e.descriptor.id).collect()
        };
        ids.into_iter()
            .filter(|id| {
                let guard = self.plugins.lock().unwrap();
                guard.get(id).is_some_and(|e| e.plugin.start().is_ok())
            })
            .collect()
    }

    fn stop_all(&self) -> Vec<PluginId> {
        let ids: Vec<PluginId> = {
            let guard = self.plugins.lock().unwrap();
            guard.values().map(|e| e.descriptor.id).collect()
        };
        ids.into_iter()
            .filter(|id| {
                let guard = self.plugins.lock().unwrap();
                guard.get(id).is_some_and(|e| e.plugin.stop().is_ok())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopPlugin {
        desc: PluginDescriptor,
    }

    impl Plugin for NoopPlugin {
        fn descriptor(&self) -> PluginDescriptor {
            self.desc.clone()
        }

        fn start(&self) -> Result<(), String> {
            Ok(())
        }

        fn stop(&self) -> Result<(), String> {
            Ok(())
        }
    }

    fn make_plugin(name: &str) -> Box<dyn Plugin> {
        Box::new(NoopPlugin {
            desc: PluginDescriptor {
                id: PluginId::new(),
                name: name.into(),
                version: "0.1.0".into(),
                description: String::new(),
                capabilities: Vec::new(),
            },
        })
    }

    #[test]
    fn register_and_list() {
        let pm = InMemoryPluginManager::new();
        let p = make_plugin("test-plugin");
        let id = pm.register(p);
        assert_eq!(pm.list().len(), 1);
        assert!(pm.get(id).is_some());
    }

    #[test]
    fn unregister() {
        let pm = InMemoryPluginManager::new();
        let id = pm.register(make_plugin("temp"));
        assert!(pm.unregister(id));
        assert!(pm.list().is_empty());
    }

    #[test]
    fn start_and_stop_all() {
        let pm = InMemoryPluginManager::new();
        pm.register(make_plugin("a"));
        pm.register(make_plugin("b"));
        let started = pm.start_all();
        assert_eq!(started.len(), 2);
        let stopped = pm.stop_all();
        assert_eq!(stopped.len(), 2);
    }
}
