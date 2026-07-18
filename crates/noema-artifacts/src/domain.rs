//! Artifact semantic records and validation.

use serde_json::Value;

use crate::ArtifactDomainError;

/// Concrete owner reference for a governed artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactOwnerRef {
    /// Concrete owner object type.
    pub object_type: String,
    /// Concrete owner object id.
    pub object_id: String,
}

impl ArtifactOwnerRef {
    /// Build a conversation-owned artifact reference.
    #[must_use]
    pub fn conversation(conversation_id: impl Into<String>) -> Self {
        Self {
            object_type: "conversation".to_string(),
            object_id: conversation_id.into(),
        }
    }

    /// Build a task-owned artifact reference.
    #[must_use]
    pub fn task(task_id: impl Into<String>) -> Self {
        Self {
            object_type: "task".to_string(),
            object_id: task_id.into(),
        }
    }

    /// Validate that this owner can hold local governed artifacts.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactDomainError::UnsupportedOwner`] for an empty id or an
    /// owner type other than `conversation` or `task`.
    pub fn validate(&self) -> Result<(), ArtifactDomainError> {
        if matches!(self.object_type.as_str(), "conversation" | "task")
            && !self.object_id.trim().is_empty()
        {
            Ok(())
        } else {
            Err(ArtifactDomainError::UnsupportedOwner {
                owner_object_type: self.object_type.clone(),
                owner_object_id: self.object_id.clone(),
            })
        }
    }
}

/// Durable storage kind for an artifact family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactStorageKind {
    /// Artifact bytes live in the local Noema filesystem.
    LocalFile,
    /// Artifact content is referenced by an external durable URL.
    ExternalUrl,
}

impl ArtifactStorageKind {
    /// Return the stable persistence representation for this storage kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalFile => "local_file",
            Self::ExternalUrl => "external_url",
        }
    }

    /// Parse a stable persistence label.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactDomainError::InvalidStorageKind`] for an unknown label.
    pub fn parse(value: &str) -> Result<Self, ArtifactDomainError> {
        match value {
            "local_file" => Ok(Self::LocalFile),
            "external_url" => Ok(Self::ExternalUrl),
            _ => Err(ArtifactDomainError::InvalidStorageKind {
                value: value.to_string(),
            }),
        }
    }
}

/// Version-specific storage reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactVersionStorage {
    /// Version bytes live under a relative local artifact path.
    LocalFile {
        /// Relative artifact path beneath the Noema root.
        relative_path: String,
    },
    /// Version content lives at an external URL.
    ExternalUrl {
        /// Durable external URL for the artifact version.
        url: String,
    },
}

impl ArtifactVersionStorage {
    /// Return the artifact storage kind implied by this version storage.
    #[must_use]
    pub const fn storage_kind(&self) -> ArtifactStorageKind {
        match self {
            Self::LocalFile { .. } => ArtifactStorageKind::LocalFile,
            Self::ExternalUrl { .. } => ArtifactStorageKind::ExternalUrl,
        }
    }

    /// Normalize external URL storage while preserving local relative paths.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactDomainError`] when an external URL is invalid.
    pub fn validated(self) -> Result<Self, ArtifactDomainError> {
        match self {
            Self::LocalFile { relative_path } => Ok(Self::LocalFile { relative_path }),
            Self::ExternalUrl { url } => Ok(Self::ExternalUrl {
                url: validate_external_artifact_url(&url)?,
            }),
        }
    }
}

/// Normalize and validate a durable external artifact URL.
///
/// # Errors
///
/// Returns [`ArtifactDomainError::InvalidExternalUrl`] when `value` is not a
/// valid HTTP(S) URL.
pub fn validate_external_artifact_url(value: &str) -> Result<String, ArtifactDomainError> {
    let url = url::Url::parse(value).map_err(|_| ArtifactDomainError::InvalidExternalUrl {
        url: value.to_string(),
    })?;
    match url.scheme() {
        "http" | "https" => Ok(url.into()),
        _ => Err(ArtifactDomainError::InvalidExternalUrl {
            url: value.to_string(),
        }),
    }
}

/// Optional transcript provenance for an artifact or artifact version.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactSource {
    /// Source conversation id, when the artifact originated from a conversation.
    pub conversation_id: Option<String>,
    /// Source turn id, when the artifact originated from a specific turn.
    pub turn_id: Option<String>,
    /// Source transcript item id, when the artifact originated from a specific item.
    pub item_id: Option<String>,
}

/// Input for creating an artifact record.
#[derive(Debug, Clone, PartialEq)]
pub struct NewArtifact {
    /// Caller-supplied artifact id, or `None` to allocate one later.
    pub artifact_id: Option<String>,
    /// Concrete owner for the artifact.
    pub owner: ArtifactOwnerRef,
    /// Human-readable artifact title.
    pub title: String,
    /// Optional artifact description.
    pub description: Option<String>,
    /// Product-defined artifact kind label.
    pub artifact_kind: String,
    /// Durable storage kind shared by all versions.
    pub storage_kind: ArtifactStorageKind,
    /// Actor responsible for creating the artifact.
    pub created_by_actor_id: String,
    /// Optional transcript provenance.
    pub source: ArtifactSource,
    /// Arbitrary artifact metadata.
    pub metadata: Value,
}

impl NewArtifact {
    /// Normalize and validate semantic creation fields.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactDomainError`] when title, kind, or owner is invalid.
    pub fn validated(mut self) -> Result<Self, ArtifactDomainError> {
        self.title = non_empty(self.title, ArtifactDomainError::TitleEmpty)?;
        self.artifact_kind = non_empty(self.artifact_kind, ArtifactDomainError::KindEmpty)?;
        self.owner.validate()?;
        Ok(self)
    }
}

/// Input for creating an immutable artifact version record.
#[derive(Debug, Clone, PartialEq)]
pub struct NewArtifactVersion {
    /// Caller-supplied artifact version id, or `None` to allocate one later.
    pub artifact_version_id: Option<String>,
    /// Optional version title override.
    pub title: Option<String>,
    /// Storage location for this specific version.
    pub storage: ArtifactVersionStorage,
    /// Optional media type for the version payload.
    pub media_type: Option<String>,
    /// Optional byte size for the version payload.
    pub byte_size: Option<i64>,
    /// Optional SHA-256 digest of the version payload.
    pub content_sha256: Option<String>,
    /// Actor responsible for creating the version.
    pub created_by_actor_id: String,
    /// Optional transcript provenance.
    pub source: ArtifactSource,
    /// Arbitrary version metadata.
    pub metadata: Value,
}

impl NewArtifactVersion {
    /// Normalize and validate the version storage reference.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactDomainError`] when an external URL is invalid.
    pub fn validated(mut self) -> Result<Self, ArtifactDomainError> {
        self.storage = self.storage.validated()?;
        Ok(self)
    }
}

/// Persisted artifact metadata row.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactRecord {
    /// Stable artifact id.
    pub artifact_id: String,
    /// Concrete owner for the artifact.
    pub owner: ArtifactOwnerRef,
    /// Human-readable artifact title.
    pub title: String,
    /// Optional artifact description.
    pub description: Option<String>,
    /// Product-defined artifact kind label.
    pub artifact_kind: String,
    /// Durable storage kind shared by all versions.
    pub storage_kind: ArtifactStorageKind,
    /// Current version id, when one has been published.
    pub current_version_id: Option<String>,
    /// Actor responsible for creating the artifact.
    pub created_by_actor_id: String,
    /// Optional transcript provenance.
    pub source: ArtifactSource,
    /// Arbitrary artifact metadata.
    pub metadata: Value,
    /// Artifact creation timestamp.
    pub created_at: String,
    /// Artifact last update timestamp.
    pub updated_at: String,
}

/// Persisted immutable artifact version row.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactVersionRecord {
    /// Stable artifact version id.
    pub artifact_version_id: String,
    /// Parent artifact id.
    pub artifact_id: String,
    /// Monotonic 1-based version index within the artifact.
    pub version_index: i64,
    /// Optional version title override.
    pub title: Option<String>,
    /// Storage location for this specific version.
    pub storage: ArtifactVersionStorage,
    /// Optional media type for the version payload.
    pub media_type: Option<String>,
    /// Optional byte size for the version payload.
    pub byte_size: Option<i64>,
    /// Optional SHA-256 digest of the version payload.
    pub content_sha256: Option<String>,
    /// Actor responsible for creating the version.
    pub created_by_actor_id: String,
    /// Optional transcript provenance.
    pub source: ArtifactSource,
    /// Arbitrary version metadata.
    pub metadata: Value,
    /// Version creation timestamp.
    pub created_at: String,
}

/// Artifact row plus its immutable version history.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactWithVersions {
    /// Artifact metadata row.
    pub artifact: ArtifactRecord,
    /// Current version referenced by the artifact row.
    pub current_version: ArtifactVersionRecord,
    /// Full immutable version history in ascending version order.
    pub versions: Vec<ArtifactVersionRecord>,
}

fn non_empty(value: String, error: ArtifactDomainError) -> Result<String, ArtifactDomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_artifact_urls_normalize_http_and_reject_other_schemes() {
        assert_eq!(
            validate_external_artifact_url("https://example.com/a b").expect("url"),
            "https://example.com/a%20b"
        );
        assert!(validate_external_artifact_url("http://example.com").is_ok());
        for value in [
            "file:///tmp/report",
            "ssh://example.com/report",
            "not a URL",
        ] {
            assert!(validate_external_artifact_url(value).is_err(), "{value}");
        }
    }
}
