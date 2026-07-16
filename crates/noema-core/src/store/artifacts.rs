use noema_artifacts::{
    ArtifactOwnerRef, ArtifactRecord, ArtifactSource, ArtifactStorageKind, ArtifactVersionRecord,
    ArtifactVersionStorage, ArtifactWithVersions, NewArtifact, NewArtifactVersion,
    validate_external_artifact_url,
};
use rusqlite::{OptionalExtension, params};

use super::{
    NoemaStore, StoreError,
    artifact_writes::{
        ArtifactTransactionError, append_artifact_transaction, create_artifact_transaction,
        prepare_artifact_append, prepare_artifact_create,
    },
    ids::allocate_id,
    sqlite::json_from_string,
};

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
        let prepared = prepare_artifact_create(self, artifact, initial_version)?;
        let mut conn = self.conn.lock().await;
        create_artifact_transaction(
            &mut conn,
            &prepared,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(inherent_transaction_error)
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
        let prepared = prepare_artifact_append(self, artifact_id, None, version)?;
        let mut conn = self.conn.lock().await;
        append_artifact_transaction(
            &mut conn,
            &prepared,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(inherent_transaction_error)
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

    /// Load a local artifact version owned by one live human conversation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or stored rows
    /// violate artifact invariants.
    pub(crate) async fn get_local_artifact_version_for_human(
        &self,
        artifact_version_id: &str,
        human_id: &str,
    ) -> Result<Option<(ArtifactRecord, ArtifactVersionRecord)>, StoreError> {
        let rows = self
            .with_connection(|conn| {
                let authorized = conn.query_row(
                    r#"
                    SELECT EXISTS(
                      SELECT 1
                      FROM artifact_versions AS version
                      JOIN artifacts AS artifact
                        ON artifact.artifact_id = version.artifact_id
                      LEFT JOIN conversations AS conversation
                        ON artifact.owner_object_type = 'conversation'
                       AND conversation.conversation_id = artifact.owner_object_id
                      LEFT JOIN tasks AS task
                        ON artifact.owner_object_type = 'task'
                       AND task.task_id = artifact.owner_object_id
                      WHERE version.artifact_version_id = ?1
                        AND version.local_relative_path IS NOT NULL
                        AND version.external_url IS NULL
                        AND artifact.storage_kind = 'local_file'
                        AND artifact.deleted_at IS NULL
                        AND (
                          (
                            artifact.owner_object_type = 'conversation'
                            AND conversation.owner_object_type = 'human'
                            AND conversation.owner_object_id = ?2
                            AND conversation.primary_human_id = ?2
                            AND conversation.lifecycle_status = 'active'
                            AND conversation.deleted_at IS NULL
                          )
                          OR (
                            artifact.owner_object_type = 'task'
                            AND task.owner_human_id = ?2
                          )
                        )
                    )
                    "#,
                    params![artifact_version_id, human_id],
                    |row| row.get::<_, bool>(0),
                )?;
                if !authorized {
                    return Ok(None);
                }

                let version = conn.query_row(
                    format!(
                        "SELECT {ARTIFACT_VERSION_SELECT} FROM artifact_versions WHERE artifact_version_id = ?1 LIMIT 1"
                    )
                    .as_str(),
                    [artifact_version_id],
                    artifact_version_row,
                )?;
                let artifact = conn.query_row(
                    format!(
                        "SELECT {ARTIFACT_SELECT} FROM artifacts WHERE artifact_id = ?1 AND deleted_at IS NULL LIMIT 1"
                    )
                    .as_str(),
                    [&version.artifact_id],
                    artifact_row,
                )?;
                Ok(Some((artifact, version)))
            })
            .await?;
        rows.map(|(artifact, version)| {
            Ok((
                artifact_from_row(artifact)?,
                artifact_version_from_row(version)?,
            ))
        })
        .transpose()
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
                        WHERE owner_object_type = ?1
                          AND owner_object_id = ?2
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
    #[must_use]
    pub fn new_artifact_id(&self) -> String {
        allocate_id("artifact")
    }

    /// Allocate a new artifact version id using the canonical store prefix.
    #[must_use]
    pub fn new_artifact_version_id(&self) -> String {
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
}

pub(super) const ARTIFACT_SELECT: &str = r#"
artifact_id, owner_object_type, owner_object_id, title, description,
artifact_kind, storage_kind, current_version_id, created_by_actor_id,
source_conversation_id, source_turn_id, source_item_id, metadata_json,
created_at, updated_at
"#;

pub(super) const ARTIFACT_VERSION_SELECT: &str = r#"
artifact_version_id, artifact_id, version_index, title, local_relative_path,
external_url, media_type, byte_size, content_sha256, created_by_actor_id,
source_conversation_id, source_turn_id, source_item_id, metadata_json,
created_at
"#;

#[derive(Debug)]
pub(super) struct ArtifactRow {
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
pub(super) struct ArtifactVersionRow {
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

pub(super) fn artifact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRow> {
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

pub(super) fn artifact_version_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ArtifactVersionRow> {
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

pub(super) fn artifact_from_row(row: ArtifactRow) -> Result<ArtifactRecord, StoreError> {
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
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

pub(super) fn artifact_version_from_row(
    row: ArtifactVersionRow,
) -> Result<ArtifactVersionRecord, StoreError> {
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
        created_at: row.created_at,
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

fn inherent_transaction_error(error: ArtifactTransactionError) -> StoreError {
    match error {
        ArtifactTransactionError::Busy(error) => StoreError::Sqlite(error),
        ArtifactTransactionError::Store(error) => error,
        ArtifactTransactionError::AppendConflict { artifact_id, .. } => {
            StoreError::InvariantViolation {
                message: format!("unexpected append conflict for {artifact_id}"),
            }
        }
    }
}
