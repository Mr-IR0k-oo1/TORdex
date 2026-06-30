//! Linux-style driver architecture.
//!
//! Each driver is a self-contained module that registers capabilities with the
//! system. The registry discovers and dispatches operations to the right driver.
//! Drivers emit `SystemEvent`s that feed the event-sourced kernel.
//!
//! ```ignore
//! Driver Registry
//!     │
//!     ├── HTTP Driver       → capabilities: [fetch_html, fetch_json, fetch_binary]
//!     ├── DNS Driver        → capabilities: [resolve_a, resolve_mx, resolve_txt, resolve_aaaa]
//!     ├── Filesystem Driver → capabilities: [read_file, write_file, list_dir]
//!     ├── Tor Driver        → capabilities: [fetch_via_tor, hidden_service_query]
//!     ├── Git Driver        → capabilities: [clone_repo, read_blob, list_refs]
//!     ├── PDF Driver        → capabilities: [extract_text, extract_metadata]
//!     ├── OCR Driver        → capabilities: [ocr_image]
//!     ├── WHOIS Driver      → capabilities: [whois_lookup]
//!     ├── PCAP Driver       → capabilities: [parse_pcap, filter_packets]
//!     ├── ELF/PE/APK Driver → capabilities: [parse_binary, extract_symbols]
//!     └── Media Drivers     → capabilities: [extract_metadata, transcribe, thumbnail]
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

// ─── Capability ──────────────────────────────────────────────────────────────

/// A declared capability of a driver.
///
/// Analogous to Linux's `file_operations` — it tells the system what this
/// driver can do and what data it expects/produces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Programmatic name: "fetch_html", "resolve_a", "read_file"
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON schema for input parameters (empty object if none).
    pub input_schema: Value,
    /// JSON schema for the output value (empty object if none).
    pub output_schema: Value,
}

impl Capability {
    #[must_use]
    pub fn new(
        name: &str,
        description: &str,
        input_schema: Value,
        output_schema: Value,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
            output_schema,
        }
    }
}

// ─── DriverError ─────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("driver '{driver}' has no capability '{capability}'")]
    CapabilityNotFound {
        driver: String,
        capability: String,
    },
    #[error("driver '{0}' not found")]
    DriverNotFound(String),
    #[error("execution error: {0}")]
    Execution(String),
    #[error("invalid parameters: {0}")]
    InvalidParameters(String),
    #[error("driver '{driver}' already registered")]
    AlreadyRegistered { driver: String },
}

// ─── Driver Trait ────────────────────────────────────────────────────────────

/// A self-contained driver module.
///
/// Every driver declares its capabilities, then the system dispatches
/// operations to it by capability name. Drivers are stateless from the
/// system's perspective — state lives in the event store.
pub trait Driver: Send + Sync {
    /// Unique name for this driver (e.g. "http", "dns", "filesystem").
    fn name(&self) -> &str;

    /// Human-readable description.
    fn description(&self) -> &str;

    /// Declare what this driver can do.
    fn capabilities(&self) -> Vec<Capability>;

    /// Execute a capability with the given parameters.
    ///
    /// Returns JSON value on success. The caller is responsible for wrapping
    /// the result in an event and appending it to the event store.
    fn execute(&self, capability: &str, params: Value) -> Result<Value, DriverError>;
}

// ─── DriverInfo ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverInfo {
    pub name: String,
    pub description: String,
    pub capabilities: Vec<Capability>,
}

// ─── Driver Registry ─────────────────────────────────────────────────────────

/// System-wide driver registry. Drivers register here and are discovered by
/// capability name.
pub trait DriverRegistry: Send + Sync {
    fn register(&self, driver: Box<dyn Driver>) -> Result<(), DriverError>;
    fn unregister(&self, name: &str) -> Result<(), DriverError>;
    fn get(&self, name: &str) -> Option<Arc<dyn Driver>>;
    fn find_by_capability(&self, capability: &str) -> Vec<Arc<dyn Driver>>;
    fn list(&self) -> Vec<DriverInfo>;
    fn execute(&self, driver_name: &str, capability: &str, params: Value)
        -> Result<Value, DriverError>;
}

// ─── InMemoryDriverRegistry ──────────────────────────────────────────────────

struct RegistryInner {
    drivers: HashMap<String, Arc<dyn Driver>>,
}

pub struct InMemoryDriverRegistry {
    inner: Arc<Mutex<RegistryInner>>,
}

impl InMemoryDriverRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RegistryInner {
                drivers: HashMap::new(),
            })),
        }
    }
}

impl Default for InMemoryDriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverRegistry for InMemoryDriverRegistry {
    fn register(&self, driver: Box<dyn Driver>) -> Result<(), DriverError> {
        let mut inner = self.inner.lock().unwrap();
        let name = driver.name().to_string();
        if inner.drivers.contains_key(&name) {
            return Err(DriverError::AlreadyRegistered { driver: name });
        }
        inner.drivers.insert(name, Arc::from(driver));
        Ok(())
    }

    fn unregister(&self, name: &str) -> Result<(), DriverError> {
        let mut inner = self.inner.lock().unwrap();
        inner
            .drivers
            .remove(name)
            .ok_or_else(|| DriverError::DriverNotFound(name.to_string()))?;
        Ok(())
    }

    fn get(&self, name: &str) -> Option<Arc<dyn Driver>> {
        self.inner.lock().unwrap().drivers.get(name).cloned()
    }

    fn find_by_capability(&self, capability: &str) -> Vec<Arc<dyn Driver>> {
        self.inner
            .lock()
            .unwrap()
            .drivers
            .values()
            .filter(|d| d.capabilities().iter().any(|c| c.name == capability))
            .cloned()
            .collect()
    }

    fn list(&self) -> Vec<DriverInfo> {
        self.inner
            .lock()
            .unwrap()
            .drivers
            .values()
            .map(|d| DriverInfo {
                name: d.name().to_string(),
                description: d.description().to_string(),
                capabilities: d.capabilities(),
            })
            .collect()
    }

    fn execute(
        &self,
        driver_name: &str,
        capability: &str,
        params: Value,
    ) -> Result<Value, DriverError> {
        let driver = self
            .get(driver_name)
            .ok_or_else(|| DriverError::DriverNotFound(driver_name.to_string()))?;

        // Verify capability exists before dispatching
        let has_cap = driver
            .capabilities()
            .iter()
            .any(|c| c.name == capability);
        if !has_cap {
            return Err(DriverError::CapabilityNotFound {
                driver: driver_name.to_string(),
                capability: capability.to_string(),
            });
        }

        driver.execute(capability, params)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Mock Driver for testing ───────────────────────────────────────────

    struct MockDriver {
        name: String,
        caps: Vec<Capability>,
        fail: bool,
    }

    impl MockDriver {
        fn new(name: &str, caps: Vec<(&str, &str)>) -> Self {
            Self {
                name: name.to_string(),
                caps: caps
                    .into_iter()
                    .map(|(n, d)| Capability::new(n, d, json!({}), json!({})))
                    .collect(),
                fail: false,
            }
        }

        fn failing(name: &str) -> Self {
            Self {
                name: name.to_string(),
                caps: vec![Capability::new("op", "always fails", json!({}), json!({}))],
                fail: true,
            }
        }
    }

    impl Driver for MockDriver {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "mock driver for testing"
        }

        fn capabilities(&self) -> Vec<Capability> {
            self.caps.clone()
        }

        fn execute(&self, capability: &str, params: Value) -> Result<Value, DriverError> {
            if self.fail {
                return Err(DriverError::Execution("mock failure".into()));
            }
            Ok(json!({
                "driver": self.name,
                "capability": capability,
                "params": params,
            }))
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────

    #[test]
    fn registry_register_and_list() {
        let reg = InMemoryDriverRegistry::new();
        reg.register(Box::new(MockDriver::new("test", vec![("op1", "Op 1")])))
            .unwrap();

        let list = reg.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test");
        assert_eq!(list[0].capabilities.len(), 1);
    }

    #[test]
    fn registry_duplicate_errors() {
        let reg = InMemoryDriverRegistry::new();
        reg.register(Box::new(MockDriver::new("dup", vec![])))
            .unwrap();
        let err = reg
            .register(Box::new(MockDriver::new("dup", vec![])))
            .unwrap_err();
        assert!(matches!(err, DriverError::AlreadyRegistered { .. }));
    }

    #[test]
    fn registry_unregister() {
        let reg = InMemoryDriverRegistry::new();
        reg.register(Box::new(MockDriver::new("temp", vec![])))
            .unwrap();
        reg.unregister("temp").unwrap();
        assert!(reg.list().is_empty());
    }

    #[test]
    fn registry_unregister_missing_errors() {
        let reg = InMemoryDriverRegistry::new();
        let err = reg.unregister("ghost").unwrap_err();
        assert!(matches!(err, DriverError::DriverNotFound(_)));
    }

    #[test]
    fn registry_get_returns_driver() {
        let reg = InMemoryDriverRegistry::new();
        reg.register(Box::new(MockDriver::new("get-test", vec![])))
            .unwrap();
        let driver = reg.get("get-test");
        assert!(driver.is_some());
        assert_eq!(driver.unwrap().name(), "get-test");
    }

    #[test]
    fn registry_get_missing_returns_none() {
        let reg = InMemoryDriverRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn registry_find_by_capability() {
        let reg = InMemoryDriverRegistry::new();
        reg.register(Box::new(MockDriver::new(
            "driver-a",
            vec![("read", "Read"), ("write", "Write")],
        )))
        .unwrap();
        reg.register(Box::new(MockDriver::new(
            "driver-b",
            vec![("read", "Read")],
        )))
        .unwrap();
        reg.register(Box::new(MockDriver::new(
            "driver-c",
            vec![("delete", "Delete")],
        )))
        .unwrap();

        let readers = reg.find_by_capability("read");
        assert_eq!(readers.len(), 2);

        let deleters = reg.find_by_capability("delete");
        assert_eq!(deleters.len(), 1);

        let none = reg.find_by_capability("nonexistent");
        assert!(none.is_empty());
    }

    #[test]
    fn registry_execute_dispatches_to_driver() {
        let reg = InMemoryDriverRegistry::new();
        reg.register(Box::new(MockDriver::new(
            "http",
            vec![("fetch", "Fetch a URL")],
        )))
        .unwrap();

        let result = reg
            .execute("http", "fetch", json!({"url": "https://example.com"}))
            .unwrap();
        assert_eq!(result["driver"], "http");
        assert_eq!(result["capability"], "fetch");
        assert_eq!(result["params"]["url"], "https://example.com");
    }

    #[test]
    fn registry_execute_missing_driver() {
        let reg = InMemoryDriverRegistry::new();
        let err = reg
            .execute("ghost", "op", json!({}))
            .unwrap_err();
        assert!(matches!(err, DriverError::DriverNotFound(_)));
    }

    #[test]
    fn registry_execute_missing_capability() {
        let reg = InMemoryDriverRegistry::new();
        reg.register(Box::new(MockDriver::new(
            "svc",
            vec![("read", "Read")],
        )))
        .unwrap();

        let err = reg
            .execute("svc", "delete", json!({}))
            .unwrap_err();
        assert!(matches!(
            err,
            DriverError::CapabilityNotFound { .. }
        ));
    }

    #[test]
    fn registry_execute_driver_error_propagates() {
        let reg = InMemoryDriverRegistry::new();
        reg.register(Box::new(MockDriver::failing("bad")))
            .unwrap();

        let err = reg.execute("bad", "op", json!({})).unwrap_err();
        assert!(matches!(err, DriverError::Execution(_)));
    }

    #[test]
    fn capability_builder() {
        let cap = Capability::new(
            "fetch_html",
            "Fetch HTML content",
            json!({"url": {"type": "string"}}),
            json!({"html": {"type": "string"}}),
        );
        assert_eq!(cap.name, "fetch_html");
        assert!(cap.input_schema["url"].is_object());
    }

    #[test]
    fn driver_info_serde_roundtrip() {
        let info = DriverInfo {
            name: "test".into(),
            description: "desc".into(),
            capabilities: vec![Capability::new("op", "op desc", json!({}), json!({}))],
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: DriverInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test");
        assert_eq!(back.capabilities.len(), 1);
    }
}
