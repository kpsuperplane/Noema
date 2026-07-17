use noema_artifacts::{
    ArtifactOwnerRef, ArtifactRecord, ArtifactVersionRecord, ArtifactWithVersions,
};
use rusqlite::{OptionalExtension, params};

use super::{
    ARTIFACT_SELECT, ARTIFACT_VERSION_SELECT, NoemaStore, StoreError, artifact_from_row,
    artifact_row, artifact_version_from_row, artifact_version_row,
};

impl NoemaStore {
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
    pub async fn get_local_artifact_version_for_human(
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
