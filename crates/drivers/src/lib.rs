//! Built-in TORdex drivers.
//!
//! Each driver is a self-contained module that registers capabilities with the
//! kernel's `DriverRegistry`. New drivers can be added as additional modules
//! or external crates.
//!
//! # Included Drivers
//!
//! | Module       | Capabilities                                           |
//! |--------------|--------------------------------------------------------|
//! | `http`       | fetch, fetch_html, fetch_json, head_request            |
//! | `dns`        | resolve_a, resolve_aaaa, resolve_mx, resolve_txt, ns, ptr |
//! | `filesystem` | read_file, write_file, list_dir, file_metadata, exists |

pub mod dns;
pub mod filesystem;
pub mod http;

use tordex_core::driver::{Driver, DriverError, DriverRegistry};

/// Register all built-in drivers into a `DriverRegistry`.
///
/// Returns a list of driver names that were successfully registered.
pub fn register_all(registry: &dyn DriverRegistry) -> Result<Vec<String>, DriverError> {
    let drivers: Vec<Box<dyn Driver>> = vec![
        Box::new(http::HttpDriver::new()),
        Box::new(dns::DnsDriver::new()),
        Box::new(filesystem::FilesystemDriver::new()),
    ];

    let mut names = Vec::with_capacity(drivers.len());
    for driver in drivers {
        let name = driver.name().to_string();
        registry.register(driver)?;
        names.push(name);
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tordex_core::driver::InMemoryDriverRegistry;
    use serde_json::json;

    #[test]
    fn register_all_built_in_drivers() {
        let reg = InMemoryDriverRegistry::new();
        let names = register_all(&reg).unwrap();
        assert!(names.contains(&"http".to_string()));
        assert!(names.contains(&"dns".to_string()));
        assert!(names.contains(&"filesystem".to_string()));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn registry_lists_all_drivers() {
        let reg = InMemoryDriverRegistry::new();
        register_all(&reg).unwrap();
        let list = reg.list();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn dispatch_http_capability() {
        let reg = InMemoryDriverRegistry::new();
        register_all(&reg).unwrap();

        // Find drivers that can resolve DNS
        let dns_drivers = reg.find_by_capability("resolve_a");
        assert_eq!(dns_drivers.len(), 1);
        assert_eq!(dns_drivers[0].name(), "dns");

        // Find drivers that can fetch
        let http_drivers = reg.find_by_capability("fetch_html");
        assert_eq!(http_drivers.len(), 1);
        assert_eq!(http_drivers[0].name(), "http");

        // Find drivers that can read files
        let fs_drivers = reg.find_by_capability("read_file");
        assert_eq!(fs_drivers.len(), 1);
        assert_eq!(fs_drivers[0].name(), "filesystem");
    }

    #[test]
    fn execute_filesystem_path_exists() {
        let reg = InMemoryDriverRegistry::new();
        register_all(&reg).unwrap();

        let result = reg
            .execute("filesystem", "path_exists", json!({"path": "/tmp"}))
            .unwrap();
        assert!(result["exists"].as_bool().unwrap());
    }

    #[test]
    fn execute_dns_missing_domain() {
        let reg = InMemoryDriverRegistry::new();
        register_all(&reg).unwrap();

        let err = reg
            .execute("dns", "resolve_a", json!({}))
            .unwrap_err();
        assert!(matches!(err, DriverError::InvalidParameters(_)));
    }

    #[test]
    fn execute_http_missing_url() {
        let reg = InMemoryDriverRegistry::new();
        register_all(&reg).unwrap();

        let err = reg
            .execute("http", "fetch", json!({}))
            .unwrap_err();
        assert!(matches!(err, DriverError::InvalidParameters(_)));
    }
}
