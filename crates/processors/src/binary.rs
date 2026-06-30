//! Binary processor — extracts strings, detects architecture, and identifies
//! embedded metadata from binary files.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

const MIN_STRING_LENGTH: usize = 4;

/// Common magic bytes for file type detection.
fn detect_magic(data: &[u8]) -> Option<&'static str> {
    if data.len() < 4 {
        return None;
    }
    match &data[..4] {
        [0x7F, b'E', b'L', b'F'] => Some("ELF"),
        [0x4D, 0x5A, ..] => Some("PE"),
        [0xCF, 0xFA, 0xED, 0xFE] => Some("Mach-O (32-bit)"),
        [0xCE, 0xFA, 0xED, 0xFE] => Some("Mach-O (64-bit)"),
        [0xCA, 0xFE, 0xBA, 0xBE] => Some("Mach-O universal"),
        [0x89, b'P', b'N', b'G'] => Some("PNG image"),
        [0xFF, 0xD8, 0xFF, ..] => Some("JPEG image"),
        [b'G', b'I', b'F', b'8'] => Some("GIF image"),
        [0x25, 0x50, 0x44, 0x46] => Some("PDF"),
        [0x50, 0x4B, 0x03, 0x04] => Some("ZIP archive"),
        [0x1F, 0x8B, 0x08, ..] => Some("GZIP archive"),
        [0x42, 0x5A, 0x68, ..] => Some("BZIP2 archive"),
        [0xFD, 0x37, 0x7A, 0x58] => Some("XZ archive"),
        [b'r', b'a', b'r', b'!'] => Some("RAR archive"),
        [0x00, 0x00, 0x00, 0x18] => Some("MP4 video"),
        [b'R', b'I', b'F', b'F'] => Some("AVI/WAV container"),
        _ => None,
    }
}

pub struct BinaryProcessor;

impl BinaryProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn extract_printable_strings(&self, data: &[u8]) -> Vec<String> {
        let mut strings = Vec::new();
        let mut current = Vec::new();

        for &byte in data {
            if byte.is_ascii_graphic() || byte == b' ' || byte == b'\t' {
                current.push(byte);
            } else {
                if current.len() >= MIN_STRING_LENGTH {
                    if let Ok(s) = String::from_utf8(current.clone()) {
                        strings.push(s);
                    }
                }
                current.clear();
            }
        }
        // Check remaining
        if current.len() >= MIN_STRING_LENGTH {
            if let Ok(s) = String::from_utf8(current) {
                strings.push(s);
            }
        }

        strings
    }

    fn extract_elf_arch(data: &[u8]) -> Option<String> {
        if data.len() < 20 || &data[..4] != [0x7F, b'E', b'L', b'F'] {
            return None;
        }
        let arch = match data[4] {
            1 => "32-bit",
            2 => "64-bit",
            _ => "unknown",
        };
        let endian = match data[5] {
            1 => "Little Endian",
            2 => "Big Endian",
            _ => "unknown",
        };
        Some(format!("ELF {arch} {endian}"))
    }
}

impl Default for BinaryProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for BinaryProcessor {
    fn name(&self) -> &str {
        "binary"
    }

    fn description(&self) -> &str {
        "Extracts strings, identifies file types, and detects binary metadata"
    }

    fn content_types(&self) -> Vec<&str> {
        vec![
            "application/octet-stream",
            "application/x-executable",
            "application/x-elf",
            "application/x-sharedlib",
            "application/x-object",
        ]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let mut results = Vec::new();

        // File type detection via magic bytes
        if let Some(ftype) = detect_magic(data) {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_magic"),
                    "binary.file_type",
                    ftype.as_bytes().to_vec(),
                    "text/plain",
                )
                .with_metadata("source_observation", id),
            );

            // ELF architecture
            if let Some(arch) = Self::extract_elf_arch(data) {
                results.push(
                    ProcessedObservation::new(
                        format!("{id}_arch"),
                        "binary.architecture",
                        arch.into_bytes(),
                        "text/plain",
                    )
                    .with_metadata("source_observation", id),
                );
            }
        }

        // Printable strings
        let strings = self.extract_printable_strings(data);
        if !strings.is_empty() {
            let strings_json = serde_json::to_string(&strings).unwrap_or_default();
            results.push(
                ProcessedObservation::new(
                    format!("{id}_strings"),
                    "binary.strings",
                    strings_json.into_bytes(),
                    "application/json",
                )
                .with_metadata("string_count", &strings.len().to_string())
                .with_metadata("source_observation", id),
            );
        }

        // Size
        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "binary.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_elf() {
        let proc = BinaryProcessor::new();
        let data = vec![0x7F, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let results = proc.process("b1", &data, Some("application/octet-stream"), HashMap::new()).unwrap();
        let types: Vec<_> = results.iter().filter(|o| o.kind == "binary.file_type").collect();
        assert!(!types.is_empty());
        assert_eq!(std::str::from_utf8(&types[0].data).unwrap(), "ELF");
        let archs: Vec<_> = results.iter().filter(|o| o.kind == "binary.architecture").collect();
        assert!(!archs.is_empty());
        assert!(std::str::from_utf8(&archs[0].data).unwrap().contains("64-bit"));
    }

    #[test]
    fn extract_strings() {
        let proc = BinaryProcessor::new();
        let data = b"AAAA\x00hello world\x00BBBB\x00some_text_here\x00CCCC";
        let results = proc.process("b2", data, Some("application/octet-stream"), HashMap::new()).unwrap();
        let strings_obs: Vec<_> = results.iter().filter(|o| o.kind == "binary.strings").collect();
        assert!(!strings_obs.is_empty());
        let json_str = std::str::from_utf8(&strings_obs[0].data).unwrap();
        assert!(json_str.contains("hello world"));
    }

    #[test]
    fn detect_pdf() {
        let proc = BinaryProcessor::new();
        let data = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj";
        let results = proc.process("b3", data, Some("application/octet-stream"), HashMap::new()).unwrap();
        let types: Vec<_> = results.iter().filter(|o| o.kind == "binary.file_type").collect();
        assert_eq!(std::str::from_utf8(&types[0].data).unwrap(), "PDF");
    }

    #[test]
    fn unknown_format_still_produces_size() {
        let proc = BinaryProcessor::new();
        let data = b"\x00\x01\x02\x03 some text here";
        let results = proc.process("b4", data, Some("application/octet-stream"), HashMap::new()).unwrap();
        let sizes: Vec<_> = results.iter().filter(|o| o.kind == "binary.metadata").collect();
        assert!(!sizes.is_empty());
    }
}
