//! HTTP driver — fetch URLs, download content, interact with HTTP APIs.
//!
//! Capabilities:
//!   - `fetch`        → Fetch any URL, returns headers, status, body (base64)
//!   - `fetch_html`   → Fetch and return HTML content as text
//!   - `fetch_json`   → Fetch and parse JSON response
//!   - `head_request` → Issue HTTP HEAD request, return headers only

use serde_json::{json, Value};
use tordex_core::driver::{Capability, Driver, DriverError};

pub struct HttpDriver;

impl HttpDriver {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn cap_fetch() -> Capability {
        Capability::new(
            "fetch",
            "Fetch a URL and return the full response (headers, status, body)",
            json!({
                "url": {"type": "string"},
                "method": {"type": "string", "enum": ["GET", "POST", "PUT", "DELETE"], "default": "GET"},
                "headers": {"type": "object", "description": "Optional request headers"},
                "body": {"type": "string", "description": "Optional request body"},
            }),
            json!({
                "url": {"type": "string"},
                "status": {"type": "integer"},
                "headers": {"type": "object"},
                "body": {"type": "string", "description": "Base64-encoded response body"},
                "content_type": {"type": "string"},
            }),
        )
    }

    fn cap_fetch_html() -> Capability {
        Capability::new(
            "fetch_html",
            "Fetch a URL and return the HTML as text",
            json!({"url": {"type": "string"}}),
            json!({
                "url": {"type": "string"},
                "status": {"type": "integer"},
                "html": {"type": "string"},
            }),
        )
    }

    fn cap_fetch_json() -> Capability {
        Capability::new(
            "fetch_json",
            "Fetch a URL and parse the response as JSON",
            json!({"url": {"type": "string"}}),
            json!({
                "url": {"type": "string"},
                "status": {"type": "integer"},
                "data": {"type": "object"},
            }),
        )
    }

    fn cap_head() -> Capability {
        Capability::new(
            "head_request",
            "Issue an HTTP HEAD request and return headers",
            json!({"url": {"type": "string"}}),
            json!({
                "url": {"type": "string"},
                "status": {"type": "integer"},
                "headers": {"type": "object"},
            }),
        )
    }
}

impl Default for HttpDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl Driver for HttpDriver {
    fn name(&self) -> &str {
        "http"
    }

    fn description(&self) -> &str {
        "Fetch URLs over HTTP/HTTPS with support for GET, POST, HEAD, and custom headers"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Self::cap_fetch(),
            Self::cap_fetch_html(),
            Self::cap_fetch_json(),
            Self::cap_head(),
        ]
    }

    fn execute(&self, capability: &str, params: Value) -> Result<Value, DriverError> {
        match capability {
            "fetch" | "fetch_html" | "fetch_json" | "head_request" => {
                let url = params["url"].as_str().ok_or_else(|| {
                    DriverError::InvalidParameters("missing 'url'".into())
                })?;
                let url = url.to_string();

                tokio::runtime::Handle::current()
                    .block_on(self.do_fetch(capability, &url, params.clone()))
            }
            _ => Err(DriverError::CapabilityNotFound {
                driver: self.name().to_string(),
                capability: capability.to_string(),
            }),
        }
    }
}

impl HttpDriver {
    async fn do_fetch(
        &self,
        capability: &str,
        url: &str,
        params: Value,
    ) -> Result<Value, DriverError> {
        let client = reqwest::Client::builder()
            .user_agent("TORdex/0.1")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| DriverError::Execution(format!("client build: {e}")))?;

        let method = params
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET");

        let mut req = match method {
            "GET" => client.get(url),
            "POST" => {
                let body = params
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                client.post(url).body(body.to_string())
            }
            "PUT" => {
                let body = params
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                client.put(url).body(body.to_string())
            }
            "DELETE" => client.delete(url),
            "HEAD" => client.head(url),
            _ => {
                return Err(DriverError::InvalidParameters(format!(
                    "unsupported method: {method}"
                )));
            }
        };

        // Apply custom headers
        if let Some(headers) = params.get("headers").and_then(|v| v.as_object()) {
            for (key, val) in headers {
                if let Some(val_str) = val.as_str() {
                    req = req.header(key.as_str(), val_str);
                }
            }
        }

        let resp = req
            .send()
            .await
            .map_err(|e| DriverError::Execution(format!("request failed: {e}")))?;

        let status = resp.status().as_u16() as u64;

        // Collect response headers
        let resp_headers: Value = resp
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    json!(v.to_str().unwrap_or("<binary>")),
                )
            })
            .collect();

        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        match capability {
            "head_request" => Ok(json!({
                "url": url,
                "status": status,
                "headers": resp_headers,
            })),
            "fetch_html" => {
                let text = resp
                    .text()
                    .await
                    .map_err(|e| DriverError::Execution(format!("read body: {e}")))?;
                Ok(json!({
                    "url": url,
                    "status": status,
                    "html": text,
                }))
            }
            "fetch_json" => {
                let data: Value = resp
                    .json()
                    .await
                    .map_err(|e| DriverError::Execution(format!("parse json: {e}")))?;
                Ok(json!({
                    "url": url,
                    "status": status,
                    "data": data,
                }))
            }
            "fetch" => {
                let bytes = resp
                    .bytes()
                    .await
                    .map_err(|e| DriverError::Execution(format!("read body: {e}")))?;
                let b64 = base64_encode(&bytes);
                Ok(json!({
                    "url": url,
                    "status": status,
                    "headers": resp_headers,
                    "body": b64,
                    "content_type": content_type,
                }))
            }
            _ => unreachable!(),
        }
    }
}

/// Minimal base64 encoding (no external dep).
fn base64_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        let chars = [
            encode_six(triple >> 18),
            encode_six((triple >> 12) & 0x3F),
            if chunk.len() > 1 {
                encode_six((triple >> 6) & 0x3F)
            } else {
                '='
            },
            if chunk.len() > 2 {
                encode_six(triple & 0x3F)
            } else {
                '='
            },
        ];
        out.write_char(chars[0]).unwrap();
        out.write_char(chars[1]).unwrap();
        out.write_char(chars[2]).unwrap();
        out.write_char(chars[3]).unwrap();
    }
    out
}

fn encode_six(val: u32) -> char {
    match val {
        0..=25 => (b'A' + val as u8) as char,
        26..=51 => (b'a' + val as u8 - 26) as char,
        52..=61 => (b'0' + val as u8 - 52) as char,
        62 => '+',
        63 => '/',
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_name_and_description() {
        let driver = HttpDriver::new();
        assert_eq!(driver.name(), "http");
        assert!(!driver.description().is_empty());
    }

    #[test]
    fn capabilities_are_declared() {
        let driver = HttpDriver::new();
        let caps = driver.capabilities();
        let names: Vec<&str> = caps.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"fetch"));
        assert!(names.contains(&"fetch_html"));
        assert!(names.contains(&"fetch_json"));
        assert!(names.contains(&"head_request"));
    }

    #[test]
    fn unknown_capability_errors() {
        let driver = HttpDriver::new();
        let err = driver.execute("nonexistent", json!({})).unwrap_err();
        assert!(matches!(err, DriverError::CapabilityNotFound { .. }));
    }

    #[test]
    fn missing_url_errors() {
        let driver = HttpDriver::new();
        let err = driver.execute("fetch", json!({})).unwrap_err();
        assert!(matches!(err, DriverError::InvalidParameters(_)));
    }
}
