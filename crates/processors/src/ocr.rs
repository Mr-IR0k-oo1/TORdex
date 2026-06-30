//! OCR processor — optical character recognition for images.
//!
//! This processor requires external Tesseract/Leptonica bindings at runtime.
//! The current implementation detects image formats and provides basic metadata.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct OcrProcessor;

impl OcrProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for OcrProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for OcrProcessor {
    fn name(&self) -> &str {
        "ocr"
    }

    fn description(&self) -> &str {
        "Optical character recognition for images — provides text detection and language identification"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["image/"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let mut results = Vec::new();

        let fmt = detect_image_format(data).unwrap_or("unknown");
        results.push(
            ProcessedObservation::new(
                format!("{id}_format"),
                "ocr.metadata",
                fmt.as_bytes().to_vec(),
                "text/plain",
            )
            .with_metadata("metric", "image_format")
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "ocr.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_note"),
                "ocr.status",
                b"OCR requires Tesseract runtime - text extraction not available in this build".to_vec(),
                "text/plain",
            )
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }
}

fn detect_image_format(data: &[u8]) -> Option<&'static str> {
    if data.len() < 4 {
        return None;
    }
    match &data[..4] {
        [0x89, b'P', b'N', b'G'] => Some("PNG"),
        [0xFF, 0xD8, 0xFF, ..] => Some("JPEG"),
        [b'G', b'I', b'F', b'8'] => Some("GIF"),
        [0x42, 0x4D, ..] => Some("BMP"),
        [0x00, 0x00, 0x01, 0x00] => Some("ICO"),
        _ if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" => Some("WebP"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_ocr_input_format() {
        let proc = OcrProcessor::new();
        let data = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let results = proc.process("ocr1", &data, Some("image/png"), HashMap::new()).unwrap();
        let formats: Vec<_> = results.iter().filter(|o| o.kind == "ocr.metadata").collect();
        assert!(!formats.is_empty());
    }
}
