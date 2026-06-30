//! Office processor — extracts text from DOCX, XLSX, and PPTX documents.
//!
//! Uses `zip` + `quick-xml` when the `office` feature is enabled.

use std::collections::HashMap;
use std::io::{BufReader, Cursor, Read};

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct OfficeProcessor;

impl OfficeProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[cfg(feature = "office")]
    fn extract_docx(&self, data: &[u8], id: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        use zip::ZipArchive;

        let cursor = Cursor::new(data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| ProcessorError::InvalidInput(format!("not a valid ZIP/OOXML: {e}")))?;

        let mut results = Vec::new();

        // Detect document type
        let is_docx = archive.by_name("word/document.xml").is_ok();
        let is_xlsx = archive.by_name("xl/workbook.xml").is_ok();
        let is_pptx = archive.by_name("ppt/presentation.xml").is_ok();

        let doc_type = if is_docx { "DOCX" } else if is_xlsx { "XLSX" } else if is_pptx { "PPTX" } else { "Unknown OOXML" };
        results.push(
            ProcessedObservation::new(
                format!("{id}_type"),
                "office.document_type",
                doc_type.as_bytes().to_vec(),
                "text/plain",
            )
            .with_metadata("source_observation", id),
        );

        if is_docx {
            if let Ok(file) = archive.by_name("word/document.xml") {
                let mut reader = BufReader::new(file);
                let mut content = String::new();
                reader.read_to_string(&mut content).ok();
                let text_parts = extract_docx_text(&content);
                let text = text_parts.join(" ");
                if !text.is_empty() {
                    results.push(
                        ProcessedObservation::new(
                            format!("{id}_text"),
                            "office.text",
                            text.into_bytes(),
                            "text/plain",
                        )
                        .with_metadata("format", "docx")
                        .with_metadata("source_observation", id),
                    );
                }
            }
        }

        if is_xlsx {
            if let Ok(file) = archive.by_name("xl/sharedStrings.xml") {
                let mut reader = BufReader::new(file);
                let mut content = String::new();
                reader.read_to_string(&mut content).ok();
                let shared_strings = extract_xlsx_strings(&content);
                if !shared_strings.is_empty() {
                    let json = serde_json::to_string(&shared_strings).unwrap_or_default();
                    results.push(
                        ProcessedObservation::new(
                            format!("{id}_strings"),
                            "office.text",
                            json.into_bytes(),
                            "application/json",
                        )
                        .with_metadata("format", "xlsx")
                        .with_metadata("cell_count", &shared_strings.len().to_string())
                        .with_metadata("source_observation", id),
                    );
                }
            }
        }

        if is_pptx {
            if let Ok(file) = archive.by_name("ppt/slides/slide1.xml") {
                let mut reader = BufReader::new(file);
                let mut content = String::new();
                reader.read_to_string(&mut content).ok();
                let text_parts = extract_pptx_text(&content);
                if !text_parts.is_empty() {
                    results.push(
                        ProcessedObservation::new(
                            format!("{id}_text"),
                            "office.text",
                            text_parts.join(" ").into_bytes(),
                            "text/plain",
                        )
                        .with_metadata("format", "pptx")
                        .with_metadata("source_observation", id),
                    );
                }
            }
        }

        if results.len() <= 1 {
            return Err(ProcessorError::ProcessingFailed(
                "no text content extracted from Office document".into(),
            ));
        }

        Ok(results)
    }

    #[cfg(not(feature = "office"))]
    fn extract_magic(&self, data: &[u8]) -> Vec<ProcessedObservation> {
        let mut results = Vec::new();

        let doc_type = if data.len() > 4 && data[0..4] == [0x50, 0x4B, 0x03, 0x04] {
            if let Ok(s) = std::str::from_utf8(&data[..data.len().min(4096)]) {
                if s.contains("word/") {
                    "DOCX (detected)"
                } else if s.contains("xl/") {
                    "XLSX (detected)"
                } else if s.contains("ppt/") {
                    "PPTX (detected)"
                } else {
                    "ZIP archive (may be OOXML)"
                }
            } else {
                "ZIP archive"
            }
        } else if data.len() > 2 && data[0..2] == [0xD0, 0xCF] {
            "OLE2 (legacy Office)"
        } else {
            "unknown"
        };

        results.push(
            ProcessedObservation::new(
                "_office_type".to_string(),
                "office.metadata",
                doc_type.as_bytes().to_vec(),
                "text/plain",
            )
            .with_metadata("metric", "document_type"),
        );

        results
    }
}

impl Default for OfficeProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for OfficeProcessor {
    fn name(&self) -> &str {
        "office"
    }

    fn description(&self) -> &str {
        "Extracts text from Office documents (DOCX, XLSX, PPTX)"
    }

    fn content_types(&self) -> Vec<&str> {
        vec![
            "application/vnd.openxmlformats-officedocument",
            "application/msword",
            "application/vnd.ms-",
            "application/vnd.oasis.opendocument",
        ]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        #[cfg(feature = "office")]
        {
            self.extract_docx(data, id)
        }

        #[cfg(not(feature = "office"))]
        {
            Ok(self.extract_magic(data))
        }
    }
}

#[cfg(feature = "office")]
fn extract_docx_text(xml: &str) -> Vec<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut text_parts = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:t" => in_text = true,
            Ok(Event::End(ref e)) if e.name().as_ref() == b"w:t" => in_text = false,
            Ok(Event::Text(ref e)) if in_text => {
                if let Ok(t) = e.unescape() {
                    text_parts.push(t.to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    text_parts
}

#[cfg(feature = "office")]
fn extract_xlsx_strings(xml: &str) -> Vec<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut strings = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut in_si = false;
    let mut in_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"si" => in_si = true,
                    b"t" if in_si => in_t = true,
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"si" => in_si = false,
                    b"t" if in_si => in_t = false,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_t => {
                if let Ok(t) = e.unescape() {
                    strings.push(t.to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    strings
}

#[cfg(feature = "office")]
fn extract_pptx_text(xml: &str) -> Vec<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut text_parts = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut in_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"a:t" => in_t = true,
            Ok(Event::End(ref e)) if e.name().as_ref() == b"a:t" => in_t = false,
            Ok(Event::Text(ref e)) if in_t => {
                if let Ok(t) = e.unescape() {
                    text_parts.push(t.to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    text_parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "office"))]
    #[test]
    fn detect_office_format() {
        let proc = OfficeProcessor::new();
        let data = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        let results = proc.process("o1", &data, Some("application/msword"), HashMap::new()).unwrap();
        let types: Vec<_> = results.iter().filter(|o| o.kind == "office.metadata").collect();
        assert!(!types.is_empty());
    }

    #[cfg(feature = "office")]
    #[test]
    fn extract_docx_text_xml() {
        let text = extract_docx_text(
            r#"<?xml version="1.0"?><w:document><w:body><w:p><w:r><w:t>Hello World</w:t></w:r></w:p></w:body></w:document>"#
        );
        assert!(text.contains(&"Hello World".to_string()));
    }
}
