//! Image processor — extracts metadata, dimensions, color info, and EXIF data.
//!
//! Uses the `image` crate when the `images` feature is enabled.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct ImageProcessor;

impl ImageProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[cfg(feature = "images")]
    fn extract_with_image_crate(&self, data: &[u8], id: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        use image::GenericImageView;
        let mut results = Vec::new();

        if let Ok(img) = image::load_from_memory(data) {
            let (w, h) = img.dimensions();
            results.push(
                ProcessedObservation::new(
                    format!("{id}_dimensions"),
                    "image.dimensions",
                    format!("{w}x{h}").into_bytes(),
                    "text/plain",
                )
                .with_metadata("width", &w.to_string())
                .with_metadata("height", &h.to_string())
                .with_metadata("source_observation", id),
            );

            let color_type = format!("{:?}", img.color());
            results.push(
                ProcessedObservation::new(
                    format!("{id}_color"),
                    "image.metadata",
                    color_type.into_bytes(),
                    "text/plain",
                )
                .with_metadata("metric", "color_type")
                .with_metadata("source_observation", id),
            );

            let pixel_count = w as u64 * h as u64;
            results.push(
                ProcessedObservation::new(
                    format!("{id}_pixels"),
                    "image.metadata",
                    pixel_count.to_string().into_bytes(),
                    "text/plain",
                )
                .with_metadata("metric", "pixel_count")
                .with_metadata("value", &pixel_count.to_string())
                .with_metadata("source_observation", id),
            );
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "image.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("source_observation", id),
        );

        Ok(results)
    }

    #[cfg(not(feature = "images"))]
    fn extract_basic(&self, data: &[u8], id: &str) -> Vec<ProcessedObservation> {
        let mut results = Vec::new();
        let fmt = detect_image_format(data).unwrap_or("unknown");

        results.push(
            ProcessedObservation::new(
                format!("{id}_format"),
                "image.metadata",
                fmt.as_bytes().to_vec(),
                "text/plain",
            )
            .with_metadata("metric", "format")
            .with_metadata("source_observation", id),
        );

        // Parse basic dimensions from headers
        let dims = parse_image_dimensions(data);
        if let Some((w, h)) = dims {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_dimensions"),
                    "image.dimensions",
                    format!("{w}x{h}").into_bytes(),
                    "text/plain",
                )
                .with_metadata("width", &w.to_string())
                .with_metadata("height", &h.to_string())
                .with_metadata("source_observation", id),
            );
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "image.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("source_observation", id),
        );

        results
    }
}

impl Default for ImageProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for ImageProcessor {
    fn name(&self) -> &str {
        "images"
    }

    fn description(&self) -> &str {
        "Extracts metadata, EXIF, dimensions, and color information from images"
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
        #[cfg(feature = "images")]
        {
            self.extract_with_image_crate(data, id)
        }

        #[cfg(not(feature = "images"))]
        {
            Ok(self.extract_basic(data, id))
        }
    }
}

/// Detect image format from magic bytes.
#[cfg(not(feature = "images"))]
fn detect_image_format(data: &[u8]) -> Option<&'static str> {
    if data.len() < 4 {
        return None;
    }
    match &data[..4] {
        [0x89, b'P', b'N', b'G'] => Some("PNG"),
        [0xFF, 0xD8, 0xFF, ..] => Some("JPEG"),
        [b'G', b'I', b'F', b'8'] => Some("GIF"),
        [b'B', b'M', ..] => Some("BMP"),
        [0x00, 0x00, 0x01, 0x00] => Some("ICO"),
        _ => {
            if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
                Some("WebP")
            } else {
                None
            }
        }
    }
}

/// Parse image dimensions from common format headers (without full decode).
#[cfg(not(feature = "images"))]
fn parse_image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 16 {
        return None;
    }
    match &data[..4] {
        [0x89, b'P', b'N', b'G'] => {
            // PNG: IHDR chunk at offset 16: 4 bytes width, 4 bytes height (big-endian)
            if data.len() < 24 {
                return None;
            }
            let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            Some((w, h))
        }
        [0xFF, 0xD8, 0xFF, ..] => {
            // JPEG: scan for SOF marker (0xFF 0xC0 or 0xFF 0xC2)
            let mut i = 2;
            while i + 8 < data.len() {
                if data[i] == 0xFF && matches!(data[i + 1], 0xC0 | 0xC2) {
                    let h = u16::from_be_bytes([data[i + 5], data[i + 6]]);
                    let w = u16::from_be_bytes([data[i + 7], data[i + 8]]);
                    return Some((w as u32, h as u32));
                }
                i += 1;
            }
            None
        }
        [b'G', b'I', b'F', b'8'] => {
            // GIF: width at offset 6, height at offset 8 (little-endian)
            if data.len() < 10 {
                return None;
            }
            let w = u16::from_le_bytes([data[6], data[7]]);
            let h = u16::from_le_bytes([data[8], data[9]]);
            Some((w as u32, h as u32))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "images"))]
    #[test]
    fn detect_png_format() {
        let proc = ImageProcessor::new();
        let png_header = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let results = proc.process("i1", &png_header, Some("image/png"), HashMap::new()).unwrap();
        let formats: Vec<_> = results.iter().filter(|o| {
            o.metadata.get("metric") == Some(&"format".to_string())
        }).collect();
        assert!(!formats.is_empty());
        assert_eq!(std::str::from_utf8(&formats[0].data).unwrap(), "PNG");
    }

    #[cfg(not(feature = "images"))]
    #[test]
    fn detect_jpeg_format() {
        let proc = ImageProcessor::new();
        let jpeg_header = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
        let results = proc.process("i2", &jpeg_header, Some("image/jpeg"), HashMap::new()).unwrap();
        let formats: Vec<_> = results.iter().filter(|o| {
            o.metadata.get("metric") == Some(&"format".to_string())
        }).collect();
        assert!(!formats.is_empty());
    }

    #[cfg(not(feature = "images"))]
    #[test]
    fn parse_png_dimensions() {
        let mut data = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend_from_slice(b"\x00\x00\x00\x0dIHDR");
        data.extend_from_slice(&100u32.to_be_bytes());
        data.extend_from_slice(&50u32.to_be_bytes());
        data.extend_from_slice(b"\x08\x02\x00\x00\x00");
        data.extend_from_slice(b"\x00\x00\x00\x00");

        let dims = parse_image_dimensions(&data);
        assert_eq!(dims, Some((100, 50)));
    }

    #[test]
    fn unknown_format_still_produces_size() {
        let proc = ImageProcessor::new();
        let data = b"\x00\x01\x02\x03 some random data here";
        let results = proc.process("i3", data, Some("image/unknown"), HashMap::new()).unwrap();
        let sizes: Vec<_> = results.iter().filter(|o| {
            o.metadata.get("metric") == Some(&"byte_size".to_string())
        }).collect();
        assert!(!sizes.is_empty());
    }
}
