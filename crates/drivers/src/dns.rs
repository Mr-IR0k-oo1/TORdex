//! DNS driver — resolve DNS records for a domain.
//!
//! Capabilities:
//!   - `resolve_a`     → IPv4 addresses (via system resolver)
//!   - `resolve_aaaa`  → IPv6 addresses (via system resolver)
//!   - `resolve_mx`    → Mail exchange records (stub)
//!   - `resolve_txt`   → TXT records (stub)
//!   - `resolve_ns`    → Nameserver records (stub)
//!   - `resolve_ptr`   → Reverse DNS (stub)

use serde_json::{json, Value};
use tordex_core::driver::{Capability, Driver, DriverError};

use tokio::net::lookup_host;

pub struct DnsDriver;

impl DnsDriver {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn cap_a() -> Capability {
        Capability::new(
            "resolve_a",
            "Resolve IPv4 (A) records for a domain using the system resolver",
            json!({"domain": {"type": "string"}}),
            json!({
                "domain": {"type": "string"},
                "addresses": {"type": "array", "items": {"type": "string"}},
            }),
        )
    }

    fn cap_aaaa() -> Capability {
        Capability::new(
            "resolve_aaaa",
            "Resolve IPv6 (AAAA) records for a domain using the system resolver",
            json!({"domain": {"type": "string"}}),
            json!({
                "domain": {"type": "string"},
                "addresses": {"type": "array", "items": {"type": "string"}},
            }),
        )
    }

    fn cap_mx() -> Capability {
        Capability::new(
            "resolve_mx",
            "Resolve MX records for a domain (requires external DNS resolver)",
            json!({"domain": {"type": "string"}}),
            json!({
                "domain": {"type": "string"},
                "records": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "priority": {"type": "integer"},
                            "target": {"type": "string"},
                        },
                    },
                },
            }),
        )
    }

    fn cap_txt() -> Capability {
        Capability::new(
            "resolve_txt",
            "Resolve TXT records for a domain (requires external DNS resolver)",
            json!({"domain": {"type": "string"}}),
            json!({
                "domain": {"type": "string"},
                "records": {"type": "array", "items": {"type": "string"}},
            }),
        )
    }

    fn cap_ns() -> Capability {
        Capability::new(
            "resolve_ns",
            "Resolve NS records for a domain (requires external DNS resolver)",
            json!({"domain": {"type": "string"}}),
            json!({
                "domain": {"type": "string"},
                "records": {"type": "array", "items": {"type": "string"}},
            }),
        )
    }

    fn cap_ptr() -> Capability {
        Capability::new(
            "resolve_ptr",
            "Reverse DNS lookup (PTR record)",
            json!({"address": {"type": "string", "description": "IP address"}}),
            json!({
                "address": {"type": "string"},
                "hostname": {"type": "string"},
            }),
        )
    }
}

impl Default for DnsDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl Driver for DnsDriver {
    fn name(&self) -> &str {
        "dns"
    }

    fn description(&self) -> &str {
        "Resolve DNS records using the system resolver and optional external resolvers"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Self::cap_a(),
            Self::cap_aaaa(),
            Self::cap_mx(),
            Self::cap_txt(),
            Self::cap_ns(),
            Self::cap_ptr(),
        ]
    }

    fn execute(&self, capability: &str, params: Value) -> Result<Value, DriverError> {
        match capability {
            "resolve_a" | "resolve_aaaa" => {
                let domain = params["domain"].as_str().ok_or_else(|| {
                    DriverError::InvalidParameters("missing 'domain'".into())
                })?;

                // Use Tokio's async lookup_host — need a runtime handle
                let addrs = tokio::runtime::Handle::current()
                    .block_on(async {
                        // lookup_host requires port, append a dummy one
                        lookup_host(format!("{domain}:0")).await
                    })
                    .map_err(|e| DriverError::Execution(format!("DNS lookup failed: {e}")))?;

                let addresses: Vec<String> = addrs
                    .map(|addr| addr.ip().to_string())
                    .filter(|ip| {
                        if capability == "resolve_a" {
                            !ip.contains(':')
                        } else {
                            ip.contains(':')
                        }
                    })
                    .collect();

                Ok(json!({"domain": domain, "addresses": addresses}))
            }
            "resolve_mx" => {
                let domain = params["domain"].as_str().ok_or_else(|| {
                    DriverError::InvalidParameters("missing 'domain'".into())
                })?;
                // Stub — would use hickory-resolver in production
                Ok(json!({
                    "domain": domain,
                    "records": [],
                    "note": "MX resolution requires hickory-resolver; reporting empty"
                }))
            }
            "resolve_txt" => {
                let domain = params["domain"].as_str().ok_or_else(|| {
                    DriverError::InvalidParameters("missing 'domain'".into())
                })?;
                Ok(json!({
                    "domain": domain,
                    "records": [],
                    "note": "TXT resolution requires hickory-resolver; reporting empty"
                }))
            }
            "resolve_ns" => {
                let domain = params["domain"].as_str().ok_or_else(|| {
                    DriverError::InvalidParameters("missing 'domain'".into())
                })?;
                Ok(json!({
                    "domain": domain,
                    "records": [],
                    "note": "NS resolution requires hickory-resolver; reporting empty"
                }))
            }
            "resolve_ptr" => {
                let address = params["address"].as_str().ok_or_else(|| {
                    DriverError::InvalidParameters("missing 'address'".into())
                })?;
                Ok(json!({
                    "address": address,
                    "hostname": null,
                    "note": "PTR resolution requires hickory-resolver; reporting null"
                }))
            }
            _ => Err(DriverError::CapabilityNotFound {
                driver: self.name().to_string(),
                capability: capability.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_name_and_description() {
        let driver = DnsDriver::new();
        assert_eq!(driver.name(), "dns");
        assert!(!driver.description().is_empty());
    }

    #[test]
    fn capabilities_are_declared() {
        let driver = DnsDriver::new();
        let caps = driver.capabilities();
        let names: Vec<&str> = caps.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"resolve_a"));
        assert!(names.contains(&"resolve_aaaa"));
        assert!(names.contains(&"resolve_mx"));
    }

    #[test]
    fn unknown_capability_errors() {
        let driver = DnsDriver::new();
        let err = driver.execute("nonexistent", json!({})).unwrap_err();
        assert!(matches!(err, DriverError::CapabilityNotFound { .. }));
    }

    #[test]
    fn missing_params_errors() {
        let driver = DnsDriver::new();
        let err = driver.execute("resolve_a", json!({})).unwrap_err();
        assert!(matches!(err, DriverError::InvalidParameters(_)));
    }

    #[test]
    fn mx_returns_stub_note() {
        let driver = DnsDriver::new();
        let result = driver
            .execute("resolve_mx", json!({"domain": "example.com"}))
            .unwrap();
        assert!(result["note"].as_str().unwrap().contains("hickory-resolver"));
    }

    #[test]
    fn txt_returns_stub_note() {
        let driver = DnsDriver::new();
        let result = driver
            .execute("resolve_txt", json!({"domain": "example.com"}))
            .unwrap();
        assert!(result["note"].as_str().unwrap().contains("hickory-resolver"));
    }
}
