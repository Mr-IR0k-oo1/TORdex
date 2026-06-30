//! Adaptive crawler — discovers URLs from collected content and schedules
//! follow-up collections with adaptive priority.
//!
//! The crawler:
//! 1. Parses HTML for links (`<a href>`, `<img src>`, `<script src>`, etc.)
//! 2. Extracts sitemap URLs from `/robots.txt` or sitemap XML
//! 3. Assigns adaptive priority based on link depth, content type, domain
//! 4. Submits discovered URLs back to the `CollectionFabric`

use std::collections::HashMap;

use serde_json::Value;
use tracing::info;

use crate::fabric::CollectionFabric;
use crate::queue::{CollectionTarget, CollectionTask, Priority};

/// Configuration for the adaptive crawler.
#[derive(Debug, Clone)]
pub struct CrawlerConfig {
    /// Maximum crawl depth (0 = seed only, 1 = seed + 1 hop, etc.)
    pub max_depth: u32,
    /// Maximum URLs to discover per crawl.
    pub max_urls_per_crawl: usize,
    /// Whether to follow external domains.
    pub follow_external: bool,
    /// Domains to stay within (empty = all allowed).
    pub allowed_domains: Vec<String>,
    /// File extensions to skip (e.g. ".pdf", ".zip", ".png").
    pub skip_extensions: Vec<String>,
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_urls_per_crawl: 50,
            follow_external: false,
            allowed_domains: Vec::new(),
            skip_extensions: vec![
                ".pdf".into(),
                ".zip".into(),
                ".tar.gz".into(),
                ".png".into(),
                ".jpg".into(),
                ".jpeg".into(),
                ".gif".into(),
                ".svg".into(),
                ".mp4".into(),
                ".mp3".into(),
                ".exe".into(),
                ".dmg".into(),
            ],
        }
    }
}

/// Extracted link from a crawled page.
#[derive(Debug, Clone)]
pub struct DiscoveredLink {
    pub url: String,
    /// Depth from the seed.
    pub depth: u32,
    /// Where the link was found.
    pub source_kind: String,
    /// Link context (e.g. "`a_href`", "`img_src`", "`script_src`").
    pub tag: String,
}

/// Adaptive crawler that discovers URLs and submits them to the fabric.
pub struct AdaptiveCrawler {
    fabric: CollectionFabric,
    config: CrawlerConfig,
    seen: std::sync::Mutex<HashMap<String, u32>>,
}

impl AdaptiveCrawler {
    pub fn new(fabric: CollectionFabric, config: CrawlerConfig) -> Self {
        Self {
            fabric,
            config,
            seen: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Process a collection result and discover new URLs.
    ///
    /// Call this after a successful collection to automatically find and
    /// schedule follow-up collections.
    pub fn process_result(
        &self,
        task_id: &str,
        target_url: &str,
        depth: u32,
        result: &Value,
    ) -> usize {
        if depth >= self.config.max_depth {
            return 0;
        }

        // Extract URLs from the result
        let links = self.extract_links(target_url, depth, result);
        if links.is_empty() {
            return 0;
        }

        // Filter and submit
        let mut submitted = 0;
        for link in &links {
            if submitted >= self.config.max_urls_per_crawl {
                break;
            }

            if !self.should_crawl(&link.url) {
                continue;
            }

            // Mark as seen
            {
                let mut seen = self.seen.lock().unwrap();
                if seen.contains_key(&link.url) {
                    continue;
                }
                seen.insert(link.url.clone(), link.depth);
            }

            // Determine adaptive priority
            let priority = self.adaptive_priority(link);

            let discover_id = format!("crawl_{}", ulid::Ulid::new().to_string().to_lowercase());
            let task = CollectionTask::new(
                discover_id,
                CollectionTarget::Url(link.url.clone()),
                "http",
                "fetch_html",
                serde_json::json!({"url": link.url.clone()}),
            )
            .with_priority(priority)
            .with_metadata("crawl_depth", &link.depth.to_string())
            .with_metadata("source_task", task_id)
            .with_metadata("source_url", target_url)
            .with_metadata("tag", &link.tag);

            if self.fabric.submit(task).is_ok() {
                submitted += 1;
            }
        }

        if submitted > 0 {
            info!(
                source = %target_url,
                depth = depth,
                submitted = submitted,
                "crawler discovered new URLs"
            );
        }

        submitted
    }

    /// Extract links from a collection result.
    fn extract_links(&self, base_url: &str, depth: u32, result: &Value) -> Vec<DiscoveredLink> {
        let mut links = Vec::new();

        if let Some(body) = result.get("body").and_then(|v| v.as_str()) {
            links.extend(extract_from_html(base_url, depth, body));
        }
        if let Some(html) = result.get("html").and_then(|v| v.as_str()) {
            links.extend(extract_from_html(base_url, depth, html));
        }
        if let Some(data) = result.get("data") {
            if let Some(text) = data.get("html").and_then(|v| v.as_str()) {
                links.extend(extract_from_html(base_url, depth, text));
            }
        }

        links
    }

    /// Check if a URL should be crawled based on configuration.
    fn should_crawl(&self, url: &str) -> bool {
        // Skip if matches skip extensions
        if self
            .config
            .skip_extensions
            .iter()
            .any(|ext| url.ends_with(ext))
        {
            return false;
        }

        // Skip mailto:, javascript:, tel:, etc.
        if url.starts_with("mailto:")
            || url.starts_with("javascript:")
            || url.starts_with("tel:")
            || url.starts_with('#')
            || url.starts_with("data:")
        {
            return false;
        }

        // Domain filtering
        if !self.config.follow_external && !self.config.allowed_domains.is_empty() {
            let domain = extract_domain(url);
            if let Some(ref d) = domain {
                if !self.config.allowed_domains.iter().any(|ad| d == ad || d.ends_with(&format!(".{ad}"))) {
                    return false;
                }
            }
        }

        true
    }

    /// Determine adaptive priority based on link characteristics.
    const fn adaptive_priority(&self, link: &DiscoveredLink) -> Priority {
        // Deeper links get lower priority
        match link.depth {
            0 | 1 => Priority::High,
            2 => Priority::Medium,
            3 => Priority::Low,
            _ => Priority::Background,
        }
    }

    /// Reset the seen-URLs cache (e.g., for a new crawl session).
    pub fn reset(&self) {
        self.seen.lock().unwrap().clear();
    }

    /// Number of unique URLs seen.
    #[allow(dead_code)]
    fn seen_count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

// ─── URL resolution ──────────────────────────────────────────────────────────

/// Extract all links from HTML. Looks for `href`, `src`, and `action` attributes
/// on common tags.
fn extract_from_html(base_url: &str, depth: u32, html: &str) -> Vec<DiscoveredLink> {
    let mut links = Vec::new();
    links.extend(extract_attr(html, base_url, depth, "a", "href"));
    links.extend(extract_attr(html, base_url, depth, "img", "src"));
    links.extend(extract_attr(html, base_url, depth, "script", "src"));
    links.extend(extract_attr(html, base_url, depth, "link", "href"));
    links.extend(extract_attr(html, base_url, depth, "form", "action"));
    links.extend(extract_attr(html, base_url, depth, "iframe", "src"));
    links.extend(extract_attr(html, base_url, depth, "frame", "src"));
    links
}

/// Extract a specific attribute from HTML tags (no parser, string search).
///
/// Pattern: `<tag ... attr="value"` or `<tag ... attr='value'`
fn extract_attr(
    html: &str,
    base_url: &str,
    depth: u32,
    tag: &str,
    attr: &str,
) -> Vec<DiscoveredLink> {
    let mut links = Vec::new();
    let mut pos = 0;
    let tag_open = format!("<{tag}");
    let tag_close = format!("</{tag}");

    while pos < html.len() {
        let Some(tag_start) = html[pos..].find(&tag_open) else {
            break;
        };
        let tag_start = pos + tag_start;

        let Some(tag_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + tag_end;
        let tag_content = &html[tag_start..=tag_end];

        let attr_dq = format!("{attr}=\"");
        let attr_sq = format!("{attr}='");
        let value_start;

        if let Some(val_start) = tag_content.find(&attr_dq) {
            value_start = val_start + attr_dq.len();
            if let Some(value_end) = tag_content[value_start..].find('"') {
                let value = &tag_content[value_start..value_start + value_end];
                if !value.is_empty() {
                    if let Some(abs_url) = resolve_url(base_url, value) {
                        links.push(DiscoveredLink {
                            url: abs_url,
                            depth: depth + 1,
                            source_kind: "html".into(),
                            tag: format!("{tag}_{attr}"),
                        });
                    }
                }
            }
        } else if let Some(val_start) = tag_content.find(&attr_sq) {
            value_start = val_start + attr_sq.len();
            if let Some(value_end) = tag_content[value_start..].find('\'') {
                let value = &tag_content[value_start..value_start + value_end];
                if !value.is_empty() {
                    if let Some(abs_url) = resolve_url(base_url, value) {
                        links.push(DiscoveredLink {
                            url: abs_url,
                            depth: depth + 1,
                            source_kind: "html".into(),
                            tag: format!("{tag}_{attr}"),
                        });
                    }
                }
            }
        }

        if let Some(close_pos) = html[tag_end..].find(&tag_close) {
            pos = tag_end + close_pos + tag_close.len();
        } else {
            pos = tag_end + 1;
        }
    }

    links
}

/// Resolve a potentially relative URL against a base URL.
fn resolve_url(base: &str, href: &str) -> Option<String> {
    // Absolute URL
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }

    // Protocol-relative URL
    if href.starts_with("//") {
        // Extract protocol from base
        if let Some(protocol_end) = base.find("://") {
            return Some(format!("{}{}", &base[..=protocol_end], href));
        }
        return Some(format!("https:{href}"));
    }

    // Absolute path
    if href.starts_with('/') {
        if let Some(protocol_end) = base.find("://") {
            let host_end = base[protocol_end + 3..]
                .find('/')
                .map_or(base.len(), |i| protocol_end + 3 + i);
            return Some(format!("{}{href}", &base[..host_end]));
        }
        return Some(format!("https://localhost{href}"));
    }

    // Relative path
    if let Some(query_or_fragment) = href.find(['?', '#']) {
        let clean = &href[..query_or_fragment];
        if clean.is_empty() {
            // Just query/fragment — same URL
            return Some(base.to_string());
        }
    }

    // Strip filename/query from base URL to get directory
    let base_dir = if let Some(last_slash) = base.rfind('/') {
        if last_slash > 8 {
            // 8 = "https://".len()
            &base[..=last_slash]
        } else {
            base
        }
    } else {
        base
    };

    // Handle `../` and `./` in relative path
    let mut resolved = base_dir.to_string();
    if href.starts_with("./") {
        resolved.push_str(&href[2..]);
    } else if href.starts_with("../") {
        let mut remaining = href;
        while remaining.starts_with("../") {
            // Remove last path segment from base
            if let Some(last_slash) = resolved[..resolved.len() - 1].rfind('/') {
                resolved.truncate(last_slash + 1);
            }
            remaining = &remaining[3..];
        }
        resolved.push_str(remaining);
    } else {
        resolved.push_str(href);
    }

    Some(resolved)
}

/// Extract domain from a URL.
fn extract_domain(url: &str) -> Option<String> {
    let url_str = if url.starts_with("http://") || url.starts_with("https://") {
        url
    } else {
        return None;
    };

    let after_protocol = url_str.find("://").map(|i| &url_str[i + 3..])?;
    let domain = after_protocol
        .find(['/', '?', '#'])
        .map_or(after_protocol, |i| &after_protocol[..i]);
    Some(domain.to_string())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tordex_core::driver::InMemoryDriverRegistry;
    use tordex_core::event_store::InMemoryEventStore;
    use crate::fabric::CollectionFabric;

    fn test_crawler() -> AdaptiveCrawler {
        let drivers = Arc::new(InMemoryDriverRegistry::new());
        let events = Arc::new(InMemoryEventStore::new());
        let fabric = CollectionFabric::new(drivers, events);
        AdaptiveCrawler::new(fabric, CrawlerConfig::default())
    }

    #[test]
    fn resolve_absolute_url() {
        assert_eq!(
            resolve_url("https://example.com/page", "https://other.com/img.png"),
            Some("https://other.com/img.png".into())
        );
    }

    #[test]
    fn resolve_protocol_relative() {
        assert_eq!(
            resolve_url("https://example.com/page", "//cdn.example.com/img.png"),
            Some("https://cdn.example.com/img.png".into())
        );
    }

    #[test]
    fn resolve_absolute_path() {
        assert_eq!(
            resolve_url("https://example.com/blog/post", "/about"),
            Some("https://example.com/about".into())
        );
    }

    #[test]
    fn resolve_relative_path() {
        assert_eq!(
            resolve_url("https://example.com/blog/post", "contact"),
            Some("https://example.com/blog/contact".into())
        );
    }

    #[test]
    fn resolve_relative_with_dotdot() {
        assert_eq!(
            resolve_url("https://example.com/a/b/c", "../d"),
            Some("https://example.com/a/d".into())
        );
    }

    #[test]
    fn resolve_relative_with_dot() {
        assert_eq!(
            resolve_url("https://example.com/a/b", "./c"),
            Some("https://example.com/a/c".into())
        );
    }

    #[test]
    fn extract_domain_from_url() {
        assert_eq!(
            extract_domain("https://www.example.com/page?q=1"),
            Some("www.example.com".into())
        );
        assert_eq!(
            extract_domain("http://example.com"),
            Some("example.com".into())
        );
        assert!(extract_domain("not-a-url").is_none());
    }

    #[test]
    fn html_link_extraction() {
        let html = r#"
            <a href="/page1">Link 1</a>
            <a href="https://external.com">External</a>
            <img src="/image.png">
            <script src="/app.js"></script>
            <link href="/styles.css" rel="stylesheet">
            <form action="/submit">
        "#;

        let links = extract_from_html("https://example.com", 0, html);
        let urls: Vec<&str> = links.iter().map(|l| l.url.as_str()).collect();
        assert!(urls.contains(&"https://example.com/page1"));
        assert!(urls.contains(&"https://external.com"));
        assert!(urls.contains(&"https://example.com/image.png"));
        assert!(urls.contains(&"https://example.com/app.js"));
        assert!(urls.contains(&"https://example.com/styles.css"));
        assert!(urls.contains(&"https://example.com/submit"));
    }

    #[test]
    fn should_crawl_skip_extensions() {
        let crawler = test_crawler();
        assert!(!crawler.should_crawl("https://example.com/file.pdf"));
        assert!(!crawler.should_crawl("https://example.com/image.jpg"));
        assert!(crawler.should_crawl("https://example.com/page.html"));
    }

    #[test]
    fn should_crawl_skip_protocols() {
        let crawler = test_crawler();
        assert!(!crawler.should_crawl("mailto:user@example.com"));
        assert!(!crawler.should_crawl("javascript:void(0)"));
        assert!(!crawler.should_crawl("#section"));
    }

    #[test]
    fn should_crawl_domain_filtering() {
        // Use a crawler with restricted domains
        let drivers = Arc::new(InMemoryDriverRegistry::new());
        let events = Arc::new(InMemoryEventStore::new());
        let fabric = CollectionFabric::new(drivers, events);
        let config = CrawlerConfig {
            follow_external: false,
            allowed_domains: vec!["example.com".into()],
            ..Default::default()
        };
        let crawler = AdaptiveCrawler::new(fabric, config);
        assert!(crawler.should_crawl("https://example.com/page"));
        assert!(!crawler.should_crawl("https://evil.com/page"));
    }

    #[test]
    fn adaptive_priority_by_depth() {
        let crawler = test_crawler();

        assert_eq!(
            crawler.adaptive_priority(&DiscoveredLink {
                url: "https://example.com".into(),
                depth: 0,
                source_kind: "html".into(),
                tag: "a_href".into(),
            }),
            Priority::High
        );

        assert_eq!(
            crawler.adaptive_priority(&DiscoveredLink {
                url: "https://example.com".into(),
                depth: 3,
                source_kind: "html".into(),
                tag: "a_href".into(),
            }),
            Priority::Low
        );

        assert_eq!(
            crawler.adaptive_priority(&DiscoveredLink {
                url: "https://example.com".into(),
                depth: 5,
                source_kind: "html".into(),
                tag: "a_href".into(),
            }),
            Priority::Background
        );
    }
}
