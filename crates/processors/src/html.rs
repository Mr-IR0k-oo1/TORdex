//! HTML processor — extracts text, metadata, links, and heading structure.

use std::collections::HashMap;

use scraper::{Html, Selector};

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct HtmlProcessor;

impl HtmlProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for HtmlProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for HtmlProcessor {
    fn name(&self) -> &str {
        "html"
    }

    fn description(&self) -> &str {
        "Extracts text, metadata, links, and structure from HTML"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["text/html", "application/xhtml+xml"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let html_str =
            std::str::from_utf8(data).map_err(|e| ProcessorError::InvalidInput(e.to_string()))?;
        let document = Html::parse_document(html_str);
        let mut results = Vec::new();

        // 1. Extract page title
        if let Ok(sel) = Selector::parse("title") {
            if let Some(el) = document.select(&sel).next() {
                let title = el.text().collect::<String>();
                results.push(
                    ProcessedObservation::new(
                        format!("{id}_title"),
                        "html.title",
                        title.into_bytes(),
                        "text/plain",
                    )
                    .with_metadata("source_observation", id),
                );
            }
        }

        // 2. Extract meta tags (og:, twitter:, description, keywords)
        if let Ok(sel) = Selector::parse("meta[name], meta[property]") {
            for el in document.select(&sel) {
                let name = el
                    .value()
                    .attr("name")
                    .or_else(|| el.value().attr("property"))
                    .unwrap_or("")
                    .to_string();
                let content = el.value().attr("content").unwrap_or("").to_string();
                if !name.is_empty() && !content.is_empty() {
                    results.push(
                        ProcessedObservation::new(
                            format!("{id}_meta_{}", &sanitize_key(&name)),
                            "html.metadata",
                            content.into_bytes(),
                            "text/plain",
                        )
                        .with_metadata("meta_name", &name)
                        .with_metadata("source_observation", id),
                    );
                }
            }
        }

        // 3. Extract heading structure
        for level in 1..=6 {
            let selector_str = format!("h{level}");
            if let Ok(sel) = Selector::parse(&selector_str) {
                for (i, el) in document.select(&sel).enumerate() {
                    let text = el.text().collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        results.push(
                            ProcessedObservation::new(
                                format!("{id}_h{level}_{i}"),
                                "html.heading",
                                text.into_bytes(),
                                "text/plain",
                            )
                            .with_metadata("heading_level", &level.to_string())
                            .with_metadata("source_observation", id),
                        );
                    }
                }
            }
        }

        // 4. Extract all text (remove HTML tags)
        if let Ok(sel) = Selector::parse("body") {
            if let Some(body) = document.select(&sel).next() {
                let text = body.text().collect::<Vec<_>>().join(" ");
                let text = collapse_whitespace(&text);
                if !text.is_empty() {
                    results.push(
                        ProcessedObservation::new(
                            format!("{id}_text"),
                            "html.extracted_text",
                            text.into_bytes(),
                            "text/plain",
                        )
                        .with_metadata("source_observation", id),
                    );
                }
            }
        }

        // 5. Extract all links
        if let Ok(sel) = Selector::parse("a[href]") {
            for (i, el) in document.select(&sel).enumerate() {
                let href = el.value().attr("href").unwrap_or("").to_string();
                let link_text = el.text().collect::<String>().trim().to_string();
                if !href.is_empty() && !href.starts_with('#') {
                    let mut meta = HashMap::new();
                    meta.insert("url".to_string(), href);
                    meta.insert("source_observation".to_string(), id.to_string());
                    if !link_text.is_empty() {
                        meta.insert("link_text".to_string(), link_text);
                    }
                    results.push(ProcessedObservation {
                        id: format!("{id}_link_{i}"),
                        kind: "html.link".to_string(),
                        data: Vec::new(),
                        content_type: "text/plain".to_string(),
                        metadata: meta,
                        derived: Vec::new(),
                    });
                }
            }
        }

        if results.is_empty() {
            return Err(ProcessorError::ProcessingFailed(
                "no content extracted from HTML".to_string(),
            ));
        }

        Ok(results)
    }
}

fn sanitize_key(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !in_space {
                result.push(' ');
                in_space = true;
            }
        } else {
            result.push(c);
            in_space = false;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_title() {
        let proc = HtmlProcessor::new();
        let html = b"<html><head><title>Test Page</title></head><body><p>Hello</p></body></html>";
        let results = proc.process("p1", html, Some("text/html"), HashMap::new()).unwrap();
        let titles: Vec<_> = results.iter().filter(|o| o.kind == "html.title").collect();
        assert_eq!(titles.len(), 1);
        assert_eq!(std::str::from_utf8(&titles[0].data).unwrap(), "Test Page");
    }

    #[test]
    fn extract_text() {
        let proc = HtmlProcessor::new();
        let html = b"<html><body><p>Hello world</p><p>Second para</p></body></html>";
        let results = proc.process("p2", html, Some("text/html"), HashMap::new()).unwrap();
        let texts: Vec<_> = results.iter().filter(|o| o.kind == "html.extracted_text").collect();
        assert_eq!(texts.len(), 1);
        let text = std::str::from_utf8(&texts[0].data).unwrap();
        assert!(text.contains("Hello world"));
        assert!(text.contains("Second para"));
    }

    #[test]
    fn extract_links() {
        let proc = HtmlProcessor::new();
        let html = b"<html><body><a href='https://example.com'>Example</a><a href='/relative'>Rel</a></body></html>";
        let results = proc.process("p3", html, Some("text/html"), HashMap::new()).unwrap();
        let links: Vec<_> = results.iter().filter(|o| o.kind == "html.link").collect();
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn extract_headings() {
        let proc = HtmlProcessor::new();
        let html = b"<html><body><h1>Main Title</h1><h2>Sub Title</h2></body></html>";
        let results = proc.process("p4", html, Some("text/html"), HashMap::new()).unwrap();
        let headings: Vec<_> = results.iter().filter(|o| o.kind == "html.heading").collect();
        assert_eq!(headings.len(), 2);
    }

    #[test]
    fn error_on_empty_html() {
        let proc = HtmlProcessor::new();
        let result = proc.process("p5", b"<html></html>", Some("text/html"), HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn invalid_utf8_returns_error() {
        let proc = HtmlProcessor::new();
        let result = proc.process("p6", &[0xFF, 0xFE], Some("text/html"), HashMap::new());
        assert!(result.is_err());
    }
}
