use serde_json::Value;

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
    /// Return the SQLite representation for this storage kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalFile => "local_file",
            Self::ExternalUrl => "external_url",
        }
    }

    /// Parse a SQLite storage kind label.
    ///
    /// # Errors
    ///
    /// Returns [`crate::StoreError`] when `value` is not a known storage kind.
    pub fn parse(value: &str) -> Result<Self, crate::StoreError> {
        match value {
            "local_file" => Ok(Self::LocalFile),
            "external_url" => Ok(Self::ExternalUrl),
            _ => Err(crate::StoreError::InvalidEnum {
                kind: "artifact_storage_kind",
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
        /// Relative artifact path beneath the owner artifact version directory.
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
    pub fn storage_kind(&self) -> ArtifactStorageKind {
        match self {
            Self::LocalFile { .. } => ArtifactStorageKind::LocalFile,
            Self::ExternalUrl { .. } => ArtifactStorageKind::ExternalUrl,
        }
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
