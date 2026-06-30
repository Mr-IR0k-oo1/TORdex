//! Source descriptor and supporting types.
//!
//! A `SourceDescriptor` is the *what can be collected* record. It does not
//! itself know how to collect — that responsibility belongs to Layer 1.
//! Keeping the descriptor declarative means the same record can be re-collected
//! with different collectors or routing policies over time.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

use tordex_core::id::SourceId;

/// Categories of external information the platform can collect from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// A regular website over HTTP(S).
    Website,
    /// A `.onion` service via Tor.
    OnionService,
    /// A documented HTTP API.
    Api,
    /// A Git repository, e.g. `owner/repo` on GitHub.
    Repository,
    /// A PDF / Office document available at a URL or local path.
    Document,
    /// An RSS / Atom feed.
    RssFeed,
    /// A local file on the filesystem.
    LocalFile,
    /// A research paper (PDF + metadata).
    Paper,
}

impl SourceKind {
    /// All variants, in stable order. Useful for migrations and tests.
    #[must_use]
    pub const fn all() -> &'static [SourceKind] {
        &[
            Self::Website,
            Self::OnionService,
            Self::Api,
            Self::Repository,
            Self::Document,
            Self::RssFeed,
            Self::LocalFile,
            Self::Paper,
        ]
    }
}

/// Routing policy for collection attempts.
///
/// `Auto` lets Layer 1 pick a collector based on the source metadata and the
/// shape of the first HTTP response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RoutingPolicy {
    #[default]
    Auto,
    Http,
    Browser,
}

/// Hints that influence which collector Layer 1 should use.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollectionHints {
    /// Force browser-based collection, even if HTTP succeeds.
    #[serde(default)]
    pub requires_js: bool,
    /// Preferred `Accept-Language` value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_locale: Option<String>,
    /// Cap on bytes fetched; the collector aborts when exceeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
}

/// Lifecycle state of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SourceStatus {
    #[default]
    Active,
    Paused,
    Errored,
}

/// A registered source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceDescriptor {
    pub id: SourceId,
    pub kind: SourceKind,
    pub display_name: String,
    pub locator: String,
    pub routing_policy: RoutingPolicy,
    pub hints: CollectionHints,
    pub status: SourceStatus,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "serde_json::Value::default")]
    pub metadata: serde_json::Value,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Payload accepted by `POST /sources` and `PATCH /sources/{id}`.
///
/// `id`, `created_at`, and `updated_at` are server-managed; clients supply
/// everything else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInput {
    pub kind: SourceKind,
    pub display_name: String,
    pub locator: String,
    #[serde(default)]
    pub routing_policy: RoutingPolicy,
    #[serde(default)]
    pub hints: CollectionHints,
    #[serde(default)]
    pub status: SourceStatus,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "serde_json::Value::default")]
    pub metadata: serde_json::Value,
}

/// Validation errors that can occur when ingesting a `SourceInput`.
#[derive(Debug, thiserror::Error)]
pub enum SourceValidationError {
    #[error("display_name must not be empty")]
    EmptyDisplayName,
    #[error("locator must not be empty")]
    EmptyLocator,
    #[error("locator {locator:?} is not a valid URL for {kind:?}")]
    InvalidUrl {
        kind: SourceKind,
        locator: String,
    },
    #[error("repository locator must be of the form owner/repo, got {0:?}")]
    InvalidRepositoryLocator(String),
    #[error("local file locator must be an absolute path, got {0:?}")]
    NotAbsoluteLocalPath(String),
    #[error("invalid metadata JSON: {0}")]
    InvalidMetadata(String),
}

impl SourceInput {
    /// Validate this input. Returns the validation error or `Ok(())`.
    pub fn validate(&self) -> Result<(), SourceValidationError> {
        if self.display_name.trim().is_empty() {
            return Err(SourceValidationError::EmptyDisplayName);
        }
        if self.locator.trim().is_empty() {
            return Err(SourceValidationError::EmptyLocator);
        }
        match self.kind {
            SourceKind::Website | SourceKind::OnionService | SourceKind::Api | SourceKind::RssFeed => {
                Url::parse(&self.locator)
                    .map_err(|_| SourceValidationError::InvalidUrl {
                        kind: self.kind,
                        locator: self.locator.clone(),
                    })?;
            }
            SourceKind::Repository => {
                let mut parts = self.locator.split('/');
                let owner = parts.next();
                let repo = parts.next();
                let extra = parts.next();
                let owner_ok = owner.is_some_and(|s| !s.is_empty());
                let repo_ok = repo.is_some_and(|s| !s.is_empty());
                if !owner_ok || !repo_ok || extra.is_some() {
                    return Err(SourceValidationError::InvalidRepositoryLocator(
                        self.locator.clone(),
                    ));
                }
            }
            SourceKind::LocalFile => {
                let p = std::path::Path::new(&self.locator);
                if !p.is_absolute() {
                    return Err(SourceValidationError::NotAbsoluteLocalPath(
                        self.locator.clone(),
                    ));
                }
            }
            SourceKind::Document | SourceKind::Paper => {
                // Accept either a URL or a local path.
                if Url::parse(&self.locator).is_err() {
                    let p = std::path::Path::new(&self.locator);
                    if !p.is_absolute() {
                        return Err(SourceValidationError::InvalidUrl {
                            kind: self.kind,
                            locator: self.locator.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}