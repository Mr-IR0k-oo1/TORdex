//! Decompiler processor — decompiles binaries into high-level representations.
//!
//! Requires external tools (Ghidra, radare2, or similar) at runtime.
//! This implementation detects binary format and provides basic metadata.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct DecompilerProcessor;

impl DecompilerProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn detect_binary_format(&self, data: &[u8]) -> Option<&'static str> {
        if data.len() < 4 {
            return None;
        }
        match &data[..4] {
            [0x7F, b'E', b'L', b'F'] => {
                match data.get(4) {
                    Some(1) => Some("ELF32"),
                    Some(2) => Some("ELF64"),
                    _ => Some("ELF"),
                }
            }
            [0x4D, 0x5A, ..] => Some("PE"),
            [0xCF, 0xFA, 0xED, 0xFE] => Some("Mach-O (32-bit)"),
            [0xCE, 0xFA, 0xED, 0xFE] => Some("Mach-O (64-bit)"),
            [0xCA, 0xFE, 0xBA, 0xBE] => Some("Mach-O Universal"),
            _ => None,
        }
    }
}

impl Default for DecompilerProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for DecompilerProcessor {
    fn name(&self) -> &str {
        "decompiler"
    }

    fn description(&self) -> &str {
        "Detects binary format for decompilation — full decompilation requires Ghidra/radare2 at runtime"
    }

    fn content_types(&self) -> Vec<&str> {
        vec![
            "application/x-elf",
            "application/x-pe",
            "application/x-mach-o",
            "application/x-executable",
            "application/octet-stream",
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

        if let Some(fmt) = self.detect_binary_format(data) {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_format"),
                    "decompiler.format",
                    fmt.as_bytes().to_vec(),
                    "text/plain",
                )
                .with_metadata("source_observation", id),
            );

            // Extract entry point hint (ELF/PE specific)
            if fmt.starts_with("ELF") && data.len() > 24 {
                let e_entry = if fmt == "ELF64" {
                    // ELF64: e_entry at offset 24 (8 bytes)
                    u64::from_le_bytes([
                        data[24], data[25], data[26], data[27],
                        data[28], data[29], data[30], data[31],
                    ])
                } else {
                    // ELF32: e_entry at offset 24 (4 bytes)
                    u32::from_le_bytes([data[24], data[25], data[26], data[27]]) as u64
                };
                if e_entry > 0 {
                    results.push(
                        ProcessedObservation::new(
                            format!("{id}_entry"),
                            "decompiler.metadata",
                            format!("{e_entry:#x}").into_bytes(),
                            "text/plain",
                        )
                        .with_metadata("metric", "entry_point")
                        .with_metadata("value", &format!("{e_entry:#x}"))
                        .with_metadata("source_observation", id),
                    );
                }
            }
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_status"),
                "decompiler.status",
                b"Binary format detected - full decompilation requires Ghidra/radare2 bridge".to_vec(),
                "text/plain",
            )
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "decompiler.metadata",
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
    fn detect_elf64() {
        let proc = DecompilerProcessor::new();
        let mut data = vec![0x7F, b'E', b'L', b'F', 2, 1, 1, 0];
        // ELF64 header with e_entry at offset 24
        data.resize(32, 0);
        data[24..32].copy_from_slice(&0x401000u64.to_le_bytes());
        let results = proc.process("d1", &data, Some("application/x-elf"), HashMap::new()).unwrap();
        let formats: Vec<_> = results.iter().filter(|o| o.kind == "decompiler.format").collect();
        assert_eq!(std::str::from_utf8(&formats[0].data).unwrap(), "ELF64");
    }

    #[test]
    fn detect_pe() {
        let proc = DecompilerProcessor::new();
        let data = vec![0x4D, 0x5A, 0x90, 0x00, 0x03, 0x00, 0x00, 0x00];
        let results = proc.process("d2", &data, Some("application/x-pe"), HashMap::new()).unwrap();
        let formats: Vec<_> = results.iter().filter(|o| o.kind == "decompiler.format").collect();
        assert_eq!(std::str::from_utf8(&formats[0].data).unwrap(), "PE");
    }

    #[test]
    fn unknown_format_still_produces_metadata() {
        let proc = DecompilerProcessor::new();
        let data = b"some random data here";
        let results = proc.process("d3", data, Some("application/octet-stream"), HashMap::new()).unwrap();
        let statuses: Vec<_> = results.iter().filter(|o| o.kind == "decompiler.status").collect();
        assert!(!statuses.is_empty());
        assert!(results.len() >= 2); // status + size
    }
}
