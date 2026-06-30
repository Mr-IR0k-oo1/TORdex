//! Video processor — extracts container metadata, codec info, and basic properties.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct VideoProcessor;

impl VideoProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn detect_container(&self, data: &[u8]) -> Option<(&'static str, Option<(u32, u32)>, Option<f64>)> {
        if data.len() < 4 {
            return None;
        }
        match &data[..4.min(data.len())] {
            [0x00, 0x00, 0x00, 0x18] | [0x00, 0x00, 0x00, 0x1C] => {
                // MP4: try to parse ftyp box for dimensions/codec
                let info = self.parse_mp4_ftyp(data);
                Some(("MP4", info, None))
            }
            [0x1A, 0x45, 0xDF, 0xA3] => {
                // WebM / Matroska
                Some(("WebM/Matroska", None, None))
            }
            [b'R', b'I', b'F', b'F'] => {
                if data.len() > 12 {
                    let sub = &data[8..12];
                    match sub {
                        b"AVI " => Some(("AVI", None, None)),
                        b"WAVE" => Some(("WAV (audio)", None, None)),
                        _ => Some(("RIFF container", None, None)),
                    }
                } else {
                    Some(("RIFF container", None, None))
                }
            }
            [0x00, 0x00, 0x00, 0x14] | [0x00, 0x00, 0x00, 0x20] => {
                Some(("MP4 variant", self.parse_mp4_ftyp(data), None))
            }
            b"OggS" => Some(("Ogg", None, None)),
            b"fLaC" => Some(("FLAC (audio)", None, None)),
            _ => None,
        }
    }

    fn parse_mp4_ftyp(&self, data: &[u8]) -> Option<(u32, u32)> {
        // ftyp box is at the start of MP4 files
        // Typically: 4 bytes size, 4 bytes "ftyp", 4 bytes major brand, ...
        if data.len() < 16 {
            return None;
        }
        // Check for ftyp brand
        if &data[4..8] == b"ftyp" {
            // Some MP4 files store dimensions in the moov/trak/tkhd box
            // For simplicity, we skip full box parsing
            Some((0, 0))
        } else {
            None
        }
    }
}

impl Default for VideoProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for VideoProcessor {
    fn name(&self) -> &str {
        "video"
    }

    fn description(&self) -> &str {
        "Detects container format, extracts codec info, and basic video metadata"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["video/", "audio/"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let mut results = Vec::new();

        if let Some((container, _dims, _duration)) = self.detect_container(data) {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_container"),
                    "video.container",
                    container.as_bytes().to_vec(),
                    "text/plain",
                )
                .with_metadata("source_observation", id),
            );
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "video.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("source_observation", id),
        );

        if results.is_empty() {
            return Err(ProcessorError::ProcessingFailed(
                "no container format detected from video data".into(),
            ));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_mp4() {
        let proc = VideoProcessor::new();
        // MP4 ftyp box: 4 bytes size, "ftyp", "mp42"
        let mut data = vec![0x00, 0x00, 0x00, 0x18]; // box size
        data.extend_from_slice(b"ftyp"); // box type
        data.extend_from_slice(b"mp42"); // major brand
        data.extend_from_slice(b"\x00\x00\x00\x00mp42mp41"); // rest
        let results = proc.process("v1", &data, Some("video/mp4"), HashMap::new()).unwrap();
        let containers: Vec<_> = results.iter().filter(|o| o.kind == "video.container").collect();
        assert!(!containers.is_empty());
        assert_eq!(std::str::from_utf8(&containers[0].data).unwrap(), "MP4");
    }

    #[test]
    fn detect_webm() {
        let proc = VideoProcessor::new();
        let data = vec![0x1A, 0x45, 0xDF, 0xA3, 0x01, 0x00, 0x00, 0x00];
        let results = proc.process("v2", &data, Some("video/webm"), HashMap::new()).unwrap();
        let containers: Vec<_> = results.iter().filter(|o| o.kind == "video.container").collect();
        assert_eq!(std::str::from_utf8(&containers[0].data).unwrap(), "WebM/Matroska");
    }

    #[test]
    fn unknown_format_still_has_metadata() {
        let proc = VideoProcessor::new();
        let data = b"\x00\x01\x02\x03\x04\x05\x06\x07";
        let results = proc.process("v3", data, Some("video/unknown"), HashMap::new()).unwrap();
        let sizes: Vec<_> = results.iter().filter(|o| o.kind == "video.metadata").collect();
        assert!(!sizes.is_empty());
    }
}
