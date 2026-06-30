//! Repository processor — analyzes Git repositories and version control metadata.
//!
//! Parses .git directory structure to extract refs, config, and basic metadata.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct RepositoryProcessor;

impl RepositoryProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn is_git_bundle(&self, data: &[u8]) -> bool {
        data.len() > 10 && &data[0..10] == b"# v2 git bundle"
    }

    fn is_git_pack(&self, data: &[u8]) -> bool {
        data.len() > 12 && &data[0..8] == b"PACK" && &data[4..8] == [0x00, 0x00, 0x00, 0x02]
    }

    fn parse_head(&self, data: &[u8]) -> Option<String> {
        let s = std::str::from_utf8(data).ok()?;
        if s.starts_with("ref: ") {
            Some(s[5..].trim().to_string())
        } else if s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()) {
            Some(format!("detached HEAD: {s}"))
        } else {
            None
        }
    }

    fn parse_git_config_section(&self, source: &str) -> HashMap<String, String> {
        let mut config = HashMap::new();
        let mut current_section = String::new();
        for line in source.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
            } else if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim().trim_matches('"').to_string();
                config.insert(format!("{current_section}.{key}"), value);
            }
        }
        config
    }
}

impl Default for RepositoryProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for RepositoryProcessor {
    fn name(&self) -> &str {
        "repositories"
    }

    fn description(&self) -> &str {
        "Analyzes Git repositories, extracts refs, config, and version control metadata"
    }

    fn content_types(&self) -> Vec<&str> {
        vec![
            "application/x-git",
            "application/vnd.github",
            "application/x-git-bundle",
            "application/x-git-pack",
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

        let repo_type = if self.is_git_bundle(data) {
            "git-bundle"
        } else if self.is_git_pack(data) {
            "git-packfile"
        } else if let Ok(s) = std::str::from_utf8(data) {
            // Try to parse as git config or HEAD
            if s.starts_with("[core]") || s.starts_with("[remote") {
                "git-config"
            } else if s.starts_with("ref: ") || (s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())) {
                "git-head"
            } else if data.len() > 8 && &data[..8] == b"DIRC" {
                "git-index"
            } else if data.len() > 4 && &data[..4] == b"OBJ_" {
                "git-object"
            } else {
                "unknown"
            }
        } else {
            "unknown"
        };

        results.push(
            ProcessedObservation::new(
                format!("{id}_type"),
                "repository.type",
                repo_type.as_bytes().to_vec(),
                "text/plain",
            )
            .with_metadata("source_observation", id),
        );

        // Parse HEAD
        if repo_type == "git-head" {
            if let Some(head_ref) = self.parse_head(data) {
                results.push(
                    ProcessedObservation::new(
                        format!("{id}_head"),
                        "repository.ref",
                        head_ref.into_bytes(),
                        "text/plain",
                    )
                    .with_metadata("ref_type", "HEAD")
                    .with_metadata("source_observation", id),
                );
            }
        }

        // Parse config
        if repo_type == "git-config" {
            if let Ok(s) = std::str::from_utf8(data) {
                let config = self.parse_git_config_section(s);
                if let Some(remote) = config.get("remote.origin.url") {
                    results.push(
                        ProcessedObservation::new(
                            format!("{id}_remote"),
                            "repository.remote",
                            remote.as_bytes().to_vec(),
                            "text/plain",
                        )
                        .with_metadata("remote", "origin")
                        .with_metadata("source_observation", id),
                    );
                }
                if !config.is_empty() {
                    let json = serde_json::to_string(&config).unwrap_or_default();
                    results.push(
                        ProcessedObservation::new(
                            format!("{id}_config"),
                            "repository.config",
                            json.into_bytes(),
                            "application/json",
                        )
                        .with_metadata("entry_count", &config.len().to_string())
                        .with_metadata("source_observation", id),
                    );
                }
            }
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "repository.metadata",
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
    fn detect_git_head() {
        let proc = RepositoryProcessor::new();
        let data = b"ref: refs/heads/main\n";
        let results = proc.process("r1", data, Some("application/x-git"), HashMap::new()).unwrap();
        let types: Vec<_> = results.iter().filter(|o| o.kind == "repository.type").collect();
        assert!(!types.is_empty());
        let heads: Vec<_> = results.iter().filter(|o| o.kind == "repository.ref").collect();
        assert_eq!(std::str::from_utf8(&heads[0].data).unwrap(), "refs/heads/main");
    }

    #[test]
    fn detect_git_config() {
        let proc = RepositoryProcessor::new();
        let data = b"[core]\n\trepositoryformatversion = 0\n[remote \"origin\"]\n\turl = https://github.com/user/repo.git\n";
        let results = proc.process("r2", data, Some("application/x-git"), HashMap::new()).unwrap();
        let remotes: Vec<_> = results.iter().filter(|o| o.kind == "repository.remote").collect();
        assert!(!remotes.is_empty());
        assert_eq!(std::str::from_utf8(&remotes[0].data).unwrap(), "https://github.com/user/repo.git");
    }

    #[test]
    fn detect_git_bundle() {
        let proc = RepositoryProcessor::new();
        let data = b"# v2 git bundle\n";
        let results = proc.process("r3", data, Some("application/x-git-bundle"), HashMap::new()).unwrap();
        let types: Vec<_> = results.iter().filter(|o| o.kind == "repository.type").collect();
        assert_eq!(std::str::from_utf8(&types[0].data).unwrap(), "git-bundle");
    }

    #[test]
    fn unknown_data_still_produces_type_and_size() {
        let proc = RepositoryProcessor::new();
        let data = b"some random content";
        let results = proc.process("r4", data, Some("application/x-git"), HashMap::new()).unwrap();
        let types: Vec<_> = results.iter().filter(|o| o.kind == "repository.type").collect();
        assert!(!types.is_empty());
    }
}
