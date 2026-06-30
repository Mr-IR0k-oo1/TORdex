//! PDF processor — extracts text, metadata, and page structure.
//!
//! Uses `lopdf` when the `pdf` feature is enabled; falls back gracefully.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct PdfProcessor;

impl PdfProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[cfg(feature = "pdf")]
    fn extract_with_lopdf(&self, data: &[u8], id: &str) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        use lopdf::Document;
        let mut results = Vec::new();

        let doc = Document::load_mem(data)
            .map_err(|e| ProcessorError::ProcessingFailed(format!("PDF parse error: {e}")))?;

        let pages = doc.get_pages();
        let page_count = pages.len();
        results.push(
            ProcessedObservation::new(
                format!("{id}_pages"),
                "pdf.metadata",
                page_count.to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "page_count")
            .with_metadata("value", &page_count.to_string())
            .with_metadata("source_observation", id),
        );

        // Extract text from each page (lopdf extract_text takes page numbers)
        for page_num in pages.keys() {
            if let Ok(text) = doc.extract_text(&[*page_num]) {
                let cleaned = text.trim().to_string();
                if !cleaned.is_empty() {
                    results.push(
                        ProcessedObservation::new(
                            format!("{id}_page_{page_num}"),
                            "pdf.text",
                            cleaned.into_bytes(),
                            "text/plain",
                        )
                        .with_metadata("page_number", &page_num.to_string())
                        .with_metadata("source_observation", id),
                    );
                }
            }
        }

        // Extract document metadata from trailer Info dict
        if let Ok(info_ref) = doc.trailer.get(b"Info").and_then(|o| o.as_reference()) {
            if let Ok(info_obj) = doc.get_object(info_ref) {
                if let Ok(info_dict) = info_obj.as_dict() {
                    for (key, value) in info_dict {
                        let key_str = String::from_utf8_lossy(key).to_string();
                        let val_str = match value {
                            lopdf::Object::String(s, _) => Some(String::from_utf8_lossy(s).to_string()),
                            lopdf::Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
                            _ => None,
                        };
                        if let Some(v) = val_str {
                            if !v.is_empty() {
                                results.push(
                                    ProcessedObservation::new(
                                        format!("{id}_meta_{key_str}"),
                                        "pdf.metadata",
                                        v.into_bytes(),
                                        "text/plain",
                                    )
                                    .with_metadata("meta_key", &key_str)
                                    .with_metadata("source_observation", id),
                                );
                            }
                        }
                    }
                }
            }
        }

        if results.is_empty() {
            return Err(ProcessorError::ProcessingFailed(
                "no content extracted from PDF".into(),
            ));
        }

        Ok(results)
    }

    #[cfg(not(feature = "pdf"))]
    fn extract_magic(&self, data: &[u8], id: &str) -> Vec<ProcessedObservation> {
        let mut results = Vec::new();

        let version = if data.len() > 5 && &data[0..5] == b"%PDF-" {
            let end = data[5..].iter().position(|&b| b == b'\n' || b == b'\r').unwrap_or(10);
            String::from_utf8_lossy(&data[5..5 + end.min(10)]).to_string()
        } else {
            "unknown".to_string()
        };

        results.push(
            ProcessedObservation::new(
                format!("{id}_version"),
                "pdf.metadata",
                version.into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "pdf_version")
            .with_metadata("source_observation", id),
        );

        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "pdf.metadata",
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

impl Default for PdfProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for PdfProcessor {
    fn name(&self) -> &str {
        "pdf"
    }

    fn description(&self) -> &str {
        "Extracts text, metadata, and page structure from PDF documents"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["application/pdf"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        #[cfg(feature = "pdf")]
        {
            self.extract_with_lopdf(data, id)
        }

        #[cfg(not(feature = "pdf"))]
        {
            Ok(self.extract_magic(data, id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "pdf"))]
    #[test]
    fn detect_pdf_version() {
        let proc = PdfProcessor::new();
        let data = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj";
        let results = proc.process("p1", data, Some("application/pdf"), HashMap::new()).unwrap();
        let versions: Vec<_> = results.iter().filter(|o| {
            o.metadata.get("metric") == Some(&"pdf_version".to_string())
        }).collect();
        assert!(!versions.is_empty());
    }
}
