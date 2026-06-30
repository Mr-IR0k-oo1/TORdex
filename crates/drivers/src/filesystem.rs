//! Filesystem driver — read/write files, list directories, read metadata.
//!
//! Capabilities:
//!   - `read_file`    → reads a file's contents (base64-encoded)
//!   - `write_file`   → writes data to a file
//!   - `list_dir`     → lists directory entries
//!   - `file_metadata`→ returns file size, permissions, modified time
//!   - `path_exists`  → checks if a path exists

use std::path::Path;

use serde_json::{json, Value};
use tordex_core::driver::{Capability, Driver, DriverError};

pub struct FilesystemDriver;

impl FilesystemDriver {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn cap_read_file() -> Capability {
        Capability::new(
            "read_file",
            "Read the contents of a file (base64-encoded)",
            json!({"path": {"type": "string", "description": "Absolute path to the file"}}),
            json!({
                "data": {"type": "string", "description": "Base64-encoded file contents"},
                "size": {"type": "integer"},
                "path": {"type": "string"},
            }),
        )
    }

    fn cap_write_file() -> Capability {
        Capability::new(
            "write_file",
            "Write data to a file (overwrites existing)",
            json!({
                "path": {"type": "string"},
                "data": {"type": "string", "description": "Base64-encoded data"},
            }),
            json!({"path": {"type": "string"}, "bytes_written": {"type": "integer"}}),
        )
    }

    fn cap_list_dir() -> Capability {
        Capability::new(
            "list_dir",
            "List entries in a directory",
            json!({"path": {"type": "string"}}),
            json!({
                "path": {"type": "string"},
                "entries": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"},
                            "kind": {"type": "string", "enum": ["file", "dir", "symlink"]},
                        },
                    },
                },
            }),
        )
    }

    fn cap_file_metadata() -> Capability {
        Capability::new(
            "file_metadata",
            "Read file metadata (size, permissions, modified time)",
            json!({"path": {"type": "string"}}),
            json!({
                "path": {"type": "string"},
                "size": {"type": "integer"},
                "is_file": {"type": "boolean"},
                "is_dir": {"type": "boolean"},
                "modified": {"type": "string", "description": "ISO-8601 timestamp or null"},
            }),
        )
    }

    fn cap_path_exists() -> Capability {
        Capability::new(
            "path_exists",
            "Check if a path exists",
            json!({"path": {"type": "string"}}),
            json!({"path": {"type": "string"}, "exists": {"type": "boolean"}}),
        )
    }
}

impl Default for FilesystemDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl Driver for FilesystemDriver {
    fn name(&self) -> &str {
        "filesystem"
    }

    fn description(&self) -> &str {
        "Read, write, and inspect files and directories on the local filesystem"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Self::cap_read_file(),
            Self::cap_write_file(),
            Self::cap_list_dir(),
            Self::cap_file_metadata(),
            Self::cap_path_exists(),
        ]
    }

    fn execute(&self, capability: &str, params: Value) -> Result<Value, DriverError> {
        match capability {
            "read_file" => {
                let path = params["path"]
                    .as_str()
                    .ok_or_else(|| DriverError::InvalidParameters("missing 'path'".into()))?;
                let data = std::fs::read(path)
                    .map_err(|e| DriverError::Execution(format!("read failed: {e}")))?;
                let b64 = base64_encode(&data);
                Ok(json!({
                    "data": b64,
                    "size": data.len(),
                    "path": path,
                }))
            }
            "write_file" => {
                let path = params["path"]
                    .as_str()
                    .ok_or_else(|| DriverError::InvalidParameters("missing 'path'".into()))?;
                let data_str = params["data"].as_str().ok_or_else(|| {
                    DriverError::InvalidParameters("missing 'data' (base64)".into())
                })?;
                let data = base64_decode(data_str)
                    .map_err(|e| DriverError::InvalidParameters(format!("invalid base64: {e}")))?;
                let bytes_written = data.len();
                if let Some(parent) = Path::new(path).parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| DriverError::Execution(format!("mkdir failed: {e}")))?;
                }
                std::fs::write(path, &data)
                    .map_err(|e| DriverError::Execution(format!("write failed: {e}")))?;
                Ok(json!({
                    "path": path,
                    "bytes_written": bytes_written,
                }))
            }
            "list_dir" => {
                let path = params["path"]
                    .as_str()
                    .ok_or_else(|| DriverError::InvalidParameters("missing 'path'".into()))?;
                let read_dir = std::fs::read_dir(path)
                    .map_err(|e| DriverError::Execution(format!("list_dir failed: {e}")))?;
                let mut entries = Vec::new();
                for entry in read_dir {
                    let entry =
                        entry.map_err(|e| DriverError::Execution(format!("read entry: {e}")))?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    let kind = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        "dir"
                    } else if entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
                        "symlink"
                    } else {
                        "file"
                    };
                    entries.push(json!({"name": name, "kind": kind}));
                }
                Ok(json!({"path": path, "entries": entries}))
            }
            "file_metadata" => {
                let path = params["path"]
                    .as_str()
                    .ok_or_else(|| DriverError::InvalidParameters("missing 'path'".into()))?;
                let meta = std::fs::metadata(path)
                    .map_err(|e| DriverError::Execution(format!("metadata failed: {e}")))?;
                let modified = meta
                    .modified()
                    .ok()
                    .map(|t| {
                        let dur = t
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default();
                        // Simple ISO-8601 format
                        let secs = dur.as_secs();
                        format!("{}", secs)
                    });
                Ok(json!({
                    "path": path,
                    "size": meta.len(),
                    "is_file": meta.is_file(),
                    "is_dir": meta.is_dir(),
                    "modified": modified,
                }))
            }
            "path_exists" => {
                let path = params["path"]
                    .as_str()
                    .ok_or_else(|| DriverError::InvalidParameters("missing 'path'".into()))?;
                Ok(json!({"path": path, "exists": Path::new(path).exists()}))
            }
            _ => Err(DriverError::CapabilityNotFound {
                driver: self.name().to_string(),
                capability: capability.to_string(),
            }),
        }
    }
}

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

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let input = input.trim_end_matches('=');
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let chars: Vec<char> = input.chars().collect();
    for chunk in chars.chunks(4) {
        let mut accum = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            let val = decode_six(c).ok_or_else(|| format!("invalid base64 char: {c}"))?;
            accum |= val << (6 * (3 - i));
        }
        output.push((accum >> 16) as u8);
        if chunk.len() > 2 {
            output.push(((accum >> 8) & 0xFF) as u8);
        }
        if chunk.len() > 3 {
            output.push((accum & 0xFF) as u8);
        }
    }
    Ok(output)
}

fn decode_six(c: char) -> Option<u32> {
    Some(match c {
        'A'..='Z' => c as u32 - 'A' as u32,
        'a'..='z' => c as u32 - 'a' as u32 + 26,
        '0'..='9' => c as u32 - '0' as u32 + 52,
        '+' => 62,
        '/' => 63,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_name_and_description() {
        let driver = FilesystemDriver::new();
        assert_eq!(driver.name(), "filesystem");
        assert!(!driver.description().is_empty());
    }

    #[test]
    fn capabilities_are_declared() {
        let driver = FilesystemDriver::new();
        let caps = driver.capabilities();
        let names: Vec<&str> = caps.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"list_dir"));
        assert!(names.contains(&"file_metadata"));
        assert!(names.contains(&"path_exists"));
    }

    #[test]
    fn path_exists_true() {
        let driver = FilesystemDriver::new();
        let result = driver
            .execute("path_exists", json!({"path": "/tmp"}))
            .unwrap();
        assert!(result["exists"].as_bool().unwrap());
    }

    #[test]
    fn path_exists_false() {
        let driver = FilesystemDriver::new();
        let result = driver
            .execute("path_exists", json!({"path": "/nonexistent_path_xyz"}))
            .unwrap();
        assert!(!result["exists"].as_bool().unwrap());
    }

    #[test]
    fn unknown_capability_errors() {
        let driver = FilesystemDriver::new();
        let err = driver
            .execute("nonexistent_cap", json!({}))
            .unwrap_err();
        assert!(matches!(err, DriverError::CapabilityNotFound { .. }));
    }

    #[test]
    fn missing_params_errors() {
        let driver = FilesystemDriver::new();
        let err = driver.execute("read_file", json!({})).unwrap_err();
        assert!(matches!(err, DriverError::InvalidParameters(_)));
    }

    #[test]
    fn write_then_read_roundtrip() {
        let driver = FilesystemDriver::new();
        let test_path = "/tmp/tordex_test_write.txt";
        let test_data = b"Hello, TORdex driver!";

        // Write
        let b64 = base64_encode(test_data);
        driver
            .execute(
                "write_file",
                json!({"path": test_path, "data": b64}),
            )
            .unwrap();

        // Read back
        let result = driver
            .execute("read_file", json!({"path": test_path}))
            .unwrap();
        assert_eq!(result["size"].as_u64().unwrap() as usize, test_data.len());

        let decoded = base64_decode(result["data"].as_str().unwrap()).unwrap();
        assert_eq!(decoded, test_data);

        // Cleanup
        let _ = std::fs::remove_file(test_path);
    }

    #[test]
    fn base64_roundtrip() {
        let data = b"Hello, World! Test data 12345";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn base64_empty() {
        let encoded = base64_encode(b"");
        assert_eq!(encoded, "");
        let decoded = base64_decode("").unwrap();
        assert!(decoded.is_empty());
    }
}
