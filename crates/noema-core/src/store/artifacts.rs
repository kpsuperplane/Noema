use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use super::{
    NoemaStore, StoreError,
    ids::{allocate_id, invalid_enum},
    sqlite::{json_from_string, json_to_string, now_timestamp_sql},
};

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
            _ => invalid_enum("artifact_storage_kind", value),
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

/// Normalize and validate a durable external artifact URL.
///
/// # Errors
///
/// Returns [`StoreError`] when `value` is not a valid HTTP(S) URL.
pub fn validate_external_artifact_url(value: &str) -> Result<String, StoreError> {
    let url = url::Url::parse(value).map_err(|_| StoreError::InvalidArtifactExternalUrl {
        url: value.to_string(),
    })?;
    match url.scheme() {
        "http" | "https" => Ok(url.into()),
        _ => Err(StoreError::InvalidArtifactExternalUrl {
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

impl NoemaStore {
    /// Create an artifact and its first immutable version in one transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the input is invalid, the owner conversation
    /// does not exist, or the embedded store write/read fails.
    pub async fn create_artifact_with_initial_version(
        &self,
        artifact: NewArtifact,
        initial_version: NewArtifactVersion,
    ) -> Result<ArtifactWithVersions, StoreError> {
        let title = trim_non_empty(artifact.title, StoreError::ArtifactTitleEmpty)?;
        let artifact_kind = trim_non_empty(artifact.artifact_kind, StoreError::ArtifactKindEmpty)?;
        self.require_artifact_creation_owner(&artifact.owner)
            .await?;
        if initial_version.storage.storage_kind() != artifact.storage_kind {
            return Err(StoreError::ArtifactStorageKindMismatch);
        }

        let artifact_id = match artifact.artifact_id {
            Some(artifact_id) => artifact_id,
            None => self.new_artifact_id()?,
        };
        let artifact_version_id = match initial_version.artifact_version_id {
            Some(artifact_version_id) => artifact_version_id,
            None => self.new_artifact_version_id()?,
        };
        let artifact_metadata_json = json_to_string(&artifact.metadata)?;
        let version_metadata_json = json_to_string(&initial_version.metadata)?;
        let version_storage = VersionStorageParts::try_from_storage(initial_version.storage)?;
        let (owner_human_id, owner_agent_id, owner_conversation_id) =
            concrete_owner_columns(&artifact.owner)?;
        let (artifact_creator_human_id, artifact_creator_agent_id) =
            concrete_actor_columns(&artifact.created_by_actor_id);
        let (version_creator_human_id, version_creator_agent_id) =
            concrete_actor_columns(&initial_version.created_by_actor_id);

        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                format!(
                    r#"
                    INSERT INTO artifacts (
                      artifact_id, owner_human_id, owner_agent_id, owner_conversation_id,
                      title, description, artifact_kind, storage_kind, current_version_id,
                      created_by_human_id, created_by_agent_id,
                      source_conversation_id, source_turn_id, source_item_id, metadata_json,
                      created_at, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, {now}, {now})
                    "#,
                    now = now_timestamp_sql()
                )
                .as_str(),
                params![
                    artifact_id,
                    owner_human_id,
                    owner_agent_id,
                    owner_conversation_id,
                    title,
                    artifact.description,
                    artifact_kind,
                    artifact.storage_kind.as_str(),
                    artifact_version_id,
                    artifact_creator_human_id,
                    artifact_creator_agent_id,
                    artifact.source.conversation_id,
                    artifact.source.turn_id,
                    artifact.source.item_id,
                    artifact_metadata_json,
                ],
            )?;
            tx.execute(
                format!(
                    r#"
                    INSERT INTO artifact_versions (
                      artifact_version_id, artifact_id, version_index, title, local_relative_path,
                      external_url, media_type, byte_size, content_sha256,
                      created_by_human_id, created_by_agent_id,
                      source_conversation_id, source_turn_id, source_item_id, metadata_json,
                      created_at
                    )
                    VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, {now})
                    "#,
                    now = now_timestamp_sql()
                )
                .as_str(),
                params![
                    artifact_version_id,
                    artifact_id,
                    initial_version.title,
                    version_storage.local_relative_path,
                    version_storage.external_url,
                    initial_version.media_type,
                    initial_version.byte_size,
                    initial_version.content_sha256,
                    version_creator_human_id,
                    version_creator_agent_id,
                    initial_version.source.conversation_id,
                    initial_version.source.turn_id,
                    initial_version.source.item_id,
                    version_metadata_json,
                ],
            )?;
            tx.commit().map_err(StoreError::Sqlite)
        })
        .await?;

        self.get_artifact(&artifact_id)
            .await?
            .ok_or(StoreError::ArtifactNotFound { artifact_id })
    }

    /// Append a new immutable version to an existing artifact.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the artifact is missing, the storage kind is
    /// inconsistent, or the embedded store write/read fails.
    pub async fn append_artifact_version(
        &self,
        artifact_id: &str,
        version: NewArtifactVersion,
    ) -> Result<ArtifactVersionRecord, StoreError> {
        let Some(existing) = self.get_artifact_row(artifact_id).await? else {
            return Err(StoreError::ArtifactNotFound {
                artifact_id: artifact_id.to_string(),
            });
        };
        if version.storage.storage_kind() != existing.storage_kind {
            return Err(StoreError::ArtifactStorageKindMismatch);
        }

        let artifact_version_id = match version.artifact_version_id {
            Some(artifact_version_id) => artifact_version_id,
            None => self.new_artifact_version_id()?,
        };
        let version_metadata_json = json_to_string(&version.metadata)?;
        let version_storage = VersionStorageParts::try_from_storage(version.storage)?;
        let (creator_human_id, creator_agent_id) =
            concrete_actor_columns(&version.created_by_actor_id);

        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let version_index = tx.query_row(
                "SELECT COALESCE(MAX(version_index), 0) + 1 FROM artifact_versions WHERE artifact_id = ?1",
                [artifact_id],
                |row| row.get::<_, i64>(0),
            )?;
            tx.execute(
                format!(
                    r#"
                    INSERT INTO artifact_versions (
                      artifact_version_id, artifact_id, version_index, title, local_relative_path,
                      external_url, media_type, byte_size, content_sha256,
                      created_by_human_id, created_by_agent_id,
                      source_conversation_id, source_turn_id, source_item_id, metadata_json,
                      created_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, {now})
                    "#
                , now = now_timestamp_sql())
                .as_str(),
                params![
                    artifact_version_id,
                    artifact_id,
                    version_index,
                    version.title,
                    version_storage.local_relative_path,
                    version_storage.external_url,
                    version.media_type,
                    version.byte_size,
                    version.content_sha256,
                    creator_human_id,
                    creator_agent_id,
                    version.source.conversation_id,
                    version.source.turn_id,
                    version.source.item_id,
                    version_metadata_json,
                ],
            )?;
            tx.execute(
                format!(
                    "UPDATE artifacts SET current_version_id = ?2, updated_at = {now} WHERE artifact_id = ?1",
                    now = now_timestamp_sql()
                )
                .as_str(),
                params![artifact_id, artifact_version_id],
            )?;
            tx.commit().map_err(StoreError::Sqlite)
        })
        .await?;

        self.get_artifact_version(&artifact_version_id)
            .await?
            .ok_or(StoreError::ArtifactNotFound {
                artifact_id: artifact_id.to_string(),
            })
    }

    /// Load one artifact and all immutable versions.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or stored rows
    /// violate artifact invariants.
    pub async fn get_artifact(
        &self,
        artifact_id: &str,
    ) -> Result<Option<ArtifactWithVersions>, StoreError> {
        let Some(artifact) = self.get_artifact_row(artifact_id).await? else {
            return Ok(None);
        };
        let versions = self.list_artifact_versions(artifact_id).await?;
        Ok(Some(assemble_artifact_with_versions(artifact, versions)?))
    }

    /// Load one immutable artifact version by id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or stored rows
    /// violate artifact invariants.
    pub async fn get_artifact_version(
        &self,
        artifact_version_id: &str,
    ) -> Result<Option<ArtifactVersionRecord>, StoreError> {
        let row = self
            .with_connection(|conn| {
                conn.query_row(
                    format!(
                        r#"
                        SELECT {ARTIFACT_VERSION_SELECT}
                        FROM artifact_versions
                        WHERE artifact_version_id = ?1
                        LIMIT 1
                        "#
                    )
                    .as_str(),
                    [artifact_version_id],
                    artifact_version_row,
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        row.map(artifact_version_from_row).transpose()
    }

    /// List non-deleted artifacts for one owner in newest-first update order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or stored rows
    /// violate artifact invariants.
    pub async fn list_artifacts_for_owner(
        &self,
        owner: ArtifactOwnerRef,
        limit: i64,
    ) -> Result<Vec<ArtifactWithVersions>, StoreError> {
        let limit = limit.clamp(1, 100);
        let rows = self
            .with_connection(|conn| {
                let mut statement = conn.prepare(
                    format!(
                        r#"
                        SELECT {ARTIFACT_SELECT}
                        FROM artifacts
                        WHERE ((?1 = 'human' AND owner_human_id = ?2)
                            OR (?1 = 'agent' AND owner_agent_id = ?2)
                            OR (?1 = 'conversation' AND owner_conversation_id = ?2))
                          AND deleted_at IS NULL
                        ORDER BY updated_at DESC, artifact_id DESC
                        LIMIT ?3
                        "#
                    )
                    .as_str(),
                )?;
                let rows = statement.query_map(
                    params![owner.object_type, owner.object_id, limit],
                    artifact_row,
                )?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;

        let mut artifacts = Vec::with_capacity(rows.len());
        for row in rows {
            let artifact = artifact_from_row(row)?;
            let versions = self.list_artifact_versions(&artifact.artifact_id).await?;
            artifacts.push(assemble_artifact_with_versions(artifact, versions)?);
        }
        Ok(artifacts)
    }

    /// List all immutable versions for one artifact in version order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the artifact is missing, the embedded store
    /// read fails, or stored rows violate artifact invariants.
    pub async fn list_artifact_versions(
        &self,
        artifact_id: &str,
    ) -> Result<Vec<ArtifactVersionRecord>, StoreError> {
        if self.get_artifact_row(artifact_id).await?.is_none() {
            return Err(StoreError::ArtifactNotFound {
                artifact_id: artifact_id.to_string(),
            });
        }
        let rows = self
            .with_connection(|conn| {
                let mut statement = conn.prepare(
                    format!(
                        r#"
                        SELECT {ARTIFACT_VERSION_SELECT}
                        FROM artifact_versions
                        WHERE artifact_id = ?1
                        ORDER BY version_index ASC
                        "#
                    )
                    .as_str(),
                )?;
                let rows = statement.query_map([artifact_id], artifact_version_row)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;
        rows.into_iter().map(artifact_version_from_row).collect()
    }

    /// Allocate a new artifact id using the canonical store prefix.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::RandomnessUnavailable`] when the operating-system
    /// cryptographic random source fails.
    pub fn new_artifact_id(&self) -> Result<String, StoreError> {
        allocate_id("artifact")
    }

    /// Allocate a new artifact version id using the canonical store prefix.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::RandomnessUnavailable`] when the operating-system
    /// cryptographic random source fails.
    pub fn new_artifact_version_id(&self) -> Result<String, StoreError> {
        allocate_id("artifact_version")
    }

    async fn get_artifact_row(
        &self,
        artifact_id: &str,
    ) -> Result<Option<ArtifactRecord>, StoreError> {
        let row = self
            .with_connection(|conn| {
                conn.query_row(
                    format!(
                        r#"
                        SELECT {ARTIFACT_SELECT}
                        FROM artifacts
                        WHERE artifact_id = ?1
                          AND deleted_at IS NULL
                        LIMIT 1
                        "#
                    )
                    .as_str(),
                    [artifact_id],
                    artifact_row,
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        row.map(artifact_from_row).transpose()
    }

    async fn require_artifact_creation_owner(
        &self,
        owner: &ArtifactOwnerRef,
    ) -> Result<(), StoreError> {
        if owner.object_type != "conversation" {
            return Err(StoreError::UnsupportedArtifactOwner {
                owner_object_type: owner.object_type.clone(),
                owner_object_id: owner.object_id.clone(),
            });
        }
        self.require_conversation(&owner.object_id).await
    }
}

const ARTIFACT_SELECT: &str = r#"
artifact_id,
CASE WHEN owner_human_id IS NOT NULL THEN 'human'
     WHEN owner_agent_id IS NOT NULL THEN 'agent' ELSE 'conversation' END,
COALESCE(owner_human_id, owner_agent_id, owner_conversation_id), title, description,
artifact_kind, storage_kind, current_version_id,
COALESCE(created_by_human_id, created_by_agent_id),
source_conversation_id, source_turn_id, source_item_id, metadata_json,
created_at, updated_at
"#;

const ARTIFACT_VERSION_SELECT: &str = r#"
artifact_version_id, artifact_id, version_index, title, local_relative_path,
external_url, media_type, byte_size, content_sha256,
COALESCE(created_by_human_id, created_by_agent_id),
source_conversation_id, source_turn_id, source_item_id, metadata_json,
created_at
"#;

#[derive(Debug)]
struct ArtifactRow {
    artifact_id: String,
    owner_object_type: String,
    owner_object_id: String,
    title: String,
    description: Option<String>,
    artifact_kind: String,
    storage_kind: String,
    current_version_id: Option<String>,
    created_by_actor_id: String,
    source_conversation_id: Option<String>,
    source_turn_id: Option<String>,
    source_item_id: Option<String>,
    metadata_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug)]
struct ArtifactVersionRow {
    artifact_version_id: String,
    artifact_id: String,
    version_index: i64,
    title: Option<String>,
    local_relative_path: Option<String>,
    external_url: Option<String>,
    media_type: Option<String>,
    byte_size: Option<i64>,
    content_sha256: Option<String>,
    created_by_actor_id: String,
    source_conversation_id: Option<String>,
    source_turn_id: Option<String>,
    source_item_id: Option<String>,
    metadata_json: String,
    created_at: String,
}

#[derive(Debug)]
struct VersionStorageParts {
    local_relative_path: Option<String>,
    external_url: Option<String>,
}

impl VersionStorageParts {
    fn try_from_storage(storage: ArtifactVersionStorage) -> Result<Self, StoreError> {
        match storage {
            ArtifactVersionStorage::LocalFile { relative_path } => Ok(Self {
                local_relative_path: Some(relative_path),
                external_url: None,
            }),
            ArtifactVersionStorage::ExternalUrl { url } => Ok(Self {
                local_relative_path: None,
                external_url: Some(validate_external_artifact_url(&url)?),
            }),
        }
    }
}

fn artifact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRow> {
    Ok(ArtifactRow {
        artifact_id: row.get(0)?,
        owner_object_type: row.get(1)?,
        owner_object_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        artifact_kind: row.get(5)?,
        storage_kind: row.get(6)?,
        current_version_id: row.get(7)?,
        created_by_actor_id: row.get(8)?,
        source_conversation_id: row.get(9)?,
        source_turn_id: row.get(10)?,
        source_item_id: row.get(11)?,
        metadata_json: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn artifact_version_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactVersionRow> {
    Ok(ArtifactVersionRow {
        artifact_version_id: row.get(0)?,
        artifact_id: row.get(1)?,
        version_index: row.get(2)?,
        title: row.get(3)?,
        local_relative_path: row.get(4)?,
        external_url: row.get(5)?,
        media_type: row.get(6)?,
        byte_size: row.get(7)?,
        content_sha256: row.get(8)?,
        created_by_actor_id: row.get(9)?,
        source_conversation_id: row.get(10)?,
        source_turn_id: row.get(11)?,
        source_item_id: row.get(12)?,
        metadata_json: row.get(13)?,
        created_at: row.get(14)?,
    })
}

fn artifact_from_row(row: ArtifactRow) -> Result<ArtifactRecord, StoreError> {
    Ok(ArtifactRecord {
        artifact_id: row.artifact_id,
        owner: ArtifactOwnerRef {
            object_type: row.owner_object_type,
            object_id: row.owner_object_id,
        },
        title: row.title,
        description: row.description,
        artifact_kind: row.artifact_kind,
        storage_kind: ArtifactStorageKind::parse(&row.storage_kind)?,
        current_version_id: row.current_version_id,
        created_by_actor_id: row.created_by_actor_id,
        source: ArtifactSource {
            conversation_id: row.source_conversation_id,
            turn_id: row.source_turn_id,
            item_id: row.source_item_id,
        },
        metadata: json_from_string(row.metadata_json)?,
        created_at: super::ids::validate_timestamp(row.created_at)?,
        updated_at: super::ids::validate_timestamp(row.updated_at)?,
    })
}

fn artifact_version_from_row(row: ArtifactVersionRow) -> Result<ArtifactVersionRecord, StoreError> {
    Ok(ArtifactVersionRecord {
        artifact_version_id: row.artifact_version_id,
        artifact_id: row.artifact_id,
        version_index: row.version_index,
        title: row.title,
        storage: artifact_version_storage_from_row(row.local_relative_path, row.external_url)?,
        media_type: row.media_type,
        byte_size: row.byte_size,
        content_sha256: row.content_sha256,
        created_by_actor_id: row.created_by_actor_id,
        source: ArtifactSource {
            conversation_id: row.source_conversation_id,
            turn_id: row.source_turn_id,
            item_id: row.source_item_id,
        },
        metadata: json_from_string(row.metadata_json)?,
        created_at: super::ids::validate_timestamp(row.created_at)?,
    })
}

fn artifact_version_storage_from_row(
    local_relative_path: Option<String>,
    external_url: Option<String>,
) -> Result<ArtifactVersionStorage, StoreError> {
    match (local_relative_path, external_url) {
        (Some(relative_path), None) => Ok(ArtifactVersionStorage::LocalFile { relative_path }),
        (None, Some(url)) => Ok(ArtifactVersionStorage::ExternalUrl {
            url: validate_external_artifact_url(&url)?,
        }),
        (Some(_), Some(_)) | (None, None) => Err(StoreError::InvariantViolation {
            message: "artifact version row must contain exactly one storage location".to_string(),
        }),
    }
}

type ConcreteOwnerColumns = (Option<String>, Option<String>, Option<String>);

fn concrete_owner_columns(owner: &ArtifactOwnerRef) -> Result<ConcreteOwnerColumns, StoreError> {
    match owner.object_type.as_str() {
        "human" => Ok((Some(owner.object_id.clone()), None, None)),
        "agent" => Ok((None, Some(owner.object_id.clone()), None)),
        "conversation" => Ok((None, None, Some(owner.object_id.clone()))),
        _ => Err(StoreError::UnsupportedArtifactOwner {
            owner_object_type: owner.object_type.clone(),
            owner_object_id: owner.object_id.clone(),
        }),
    }
}

fn concrete_actor_columns(actor_id: &str) -> (Option<String>, Option<String>) {
    if actor_id.starts_with("human:") {
        (Some(actor_id.to_string()), None)
    } else {
        (None, Some(actor_id.to_string()))
    }
}

fn assemble_artifact_with_versions(
    artifact: ArtifactRecord,
    versions: Vec<ArtifactVersionRecord>,
) -> Result<ArtifactWithVersions, StoreError> {
    if versions.is_empty() {
        return Err(StoreError::InvariantViolation {
            message: format!("artifact {} is missing version rows", artifact.artifact_id),
        });
    }
    let current_version_id =
        artifact
            .current_version_id
            .as_deref()
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!(
                    "artifact {} is missing current_version_id",
                    artifact.artifact_id
                ),
            })?;
    let current_version = versions
        .iter()
        .find(|version| version.artifact_version_id == current_version_id)
        .cloned()
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!(
                "artifact {} current_version_id {} does not reference a stored version",
                artifact.artifact_id, current_version_id
            ),
        })?;
    if versions
        .iter()
        .any(|version| version.storage.storage_kind() != artifact.storage_kind)
    {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "artifact {} has version storage that does not match storage_kind",
                artifact.artifact_id
            ),
        });
    }

    Ok(ArtifactWithVersions {
        artifact,
        current_version,
        versions,
    })
}

fn trim_non_empty<T>(value: String, error: T) -> Result<String, T> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}
