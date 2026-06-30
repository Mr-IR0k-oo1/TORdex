//! Markdown processor — extracts text, headings, code blocks, and frontmatter.

use std::collections::HashMap;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct MarkdownProcessor;

impl MarkdownProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for MarkdownProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for MarkdownProcessor {
    fn name(&self) -> &str {
        "markdown"
    }

    fn description(&self) -> &str {
        "Extracts text, headings, code blocks, and frontmatter from Markdown"
    }

    fn content_types(&self) -> Vec<&str> {
        vec!["text/markdown", "text/x-markdown"]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let md_str =
            std::str::from_utf8(data).map_err(|e| ProcessorError::InvalidInput(e.to_string()))?;
        let mut results = Vec::new();

        // Check for YAML frontmatter
        if let Some(fm) = extract_frontmatter(md_str) {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_frontmatter"),
                    "markdown.frontmatter",
                    fm.into_bytes(),
                    "text/yaml",
                )
                .with_metadata("source_observation", id),
            );
        }

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(md_str, options);
        let mut headings: Vec<(u32, String)> = Vec::new();
        let mut text_parts: Vec<String> = Vec::new();
        let mut code_blocks: Vec<String> = Vec::new();
        let mut in_code_block = false;
        let mut heading_level = 0u32;
        let mut in_heading = false;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    heading_level = level as u32;
                    in_heading = true;
                }
                Event::End(TagEnd::Heading(_)) => {
                    if in_heading {
                        let text = headings.last().map(|(_, t)| t.clone()).unwrap_or_default();
                        if !text.is_empty() {
                            results.push(
                                ProcessedObservation::new(
                                    format!("{id}_h{heading_level}_{}", headings.len()),
                                    "markdown.heading",
                                    text.into_bytes(),
                                    "text/plain",
                                )
                                .with_metadata("heading_level", &heading_level.to_string())
                                .with_metadata("source_observation", id),
                            );
                        }
                    }
                    in_heading = false;
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code_block = true;
                    code_blocks.push(String::new());
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                }
                Event::Text(t) | Event::Code(t) => {
                    let text = t.to_string();
                    if in_heading {
                        if let Some((_, existing)) = headings.last_mut() {
                            existing.push_str(&text);
                        } else {
                            headings.push((heading_level, text.clone()));
                        }
                    } else if in_code_block {
                        if let Some(last) = code_blocks.last_mut() {
                            last.push_str(&text);
                        }
                    } else {
                        text_parts.push(text);
                    }
                }
                _ => {}
            }
        }

        // Plain text output
        let plain_text = text_parts.join(" ");
        if !plain_text.is_empty() {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_text"),
                    "markdown.text",
                    plain_text.into_bytes(),
                    "text/plain",
                )
                .with_metadata("source_observation", id),
            );
        }

        // Code blocks
        for (i, code) in code_blocks.iter().enumerate() {
            if !code.is_empty() {
                results.push(
                    ProcessedObservation::new(
                        format!("{id}_code_{i}"),
                        "markdown.code_block",
                        code.as_bytes().to_vec(),
                        "text/plain",
                    )
                    .with_metadata("source_observation", id),
                );
            }
        }

        if results.is_empty() {
            return Err(ProcessorError::ProcessingFailed(
                "no content extracted from Markdown".to_string(),
            ));
        }

        Ok(results)
    }
}

fn extract_frontmatter(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with("---") {
        if let Some(end) = s[3..].find("\n---") {
            let fm = s[3..3 + end].trim().to_string();
            if !fm.is_empty() {
                return Some(fm);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_plain_text() {
        let proc = MarkdownProcessor::new();
        let md = b"Hello **world** and *italic*";
        let results = proc.process("m1", md, Some("text/markdown"), HashMap::new()).unwrap();
        let texts: Vec<_> = results.iter().filter(|o| o.kind == "markdown.text").collect();
        assert_eq!(texts.len(), 1);
        let text = std::str::from_utf8(&texts[0].data).unwrap();
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
    }

    #[test]
    fn extract_headings() {
        let proc = MarkdownProcessor::new();
        let md = b"# Title\n\n## Subtitle\n\n### Section";
        let results = proc.process("m2", md, Some("text/markdown"), HashMap::new()).unwrap();
        let headings: Vec<_> = results.iter().filter(|o| o.kind == "markdown.heading").collect();
        assert_eq!(headings.len(), 3);
    }

    #[test]
    fn extract_frontmatter() {
        let proc = MarkdownProcessor::new();
        let md = b"---\ntitle: Test\ndate: 2024-01-01\n---\n\n# Content";
        let results = proc.process("m3", md, Some("text/markdown"), HashMap::new()).unwrap();
        let fms: Vec<_> = results.iter().filter(|o| o.kind == "markdown.frontmatter").collect();
        assert_eq!(fms.len(), 1);
        let fm = std::str::from_utf8(&fms[0].data).unwrap();
        assert!(fm.contains("title: Test"));
    }

    #[test]
    fn extract_code_blocks() {
        let proc = MarkdownProcessor::new();
        let md = b"# Code\n\n```rust\nfn main() {}\n```";
        let results = proc.process("m4", md, Some("text/markdown"), HashMap::new()).unwrap();
        let codes: Vec<_> = results.iter().filter(|o| o.kind == "markdown.code_block").collect();
        assert_eq!(codes.len(), 1);
    }

    #[test]
    fn error_on_empty() {
        let proc = MarkdownProcessor::new();
        let result = proc.process("m5", b"", Some("text/markdown"), HashMap::new());
        assert!(result.is_err());
    }
}
