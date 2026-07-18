//! Bounded task-owned artifact connection with current-version hydration.

use noema_tasks::{TaskId, WorkDomainError};
use ring::digest::{SHA256, digest};
use rusqlite::params;

use crate::{
    NoemaStore, StoreError, WorkPageInfo, WorkTaskArtifact, WorkTaskArtifactConnection,
    WorkTaskArtifactCursor, WorkTaskArtifactEdge, WorkTaskArtifactQuery,
    artifacts::{
        artifact_from_row, artifact_row_at, artifact_version_from_row, artifact_version_row_at,
    },
};

const ARTIFACT_COLUMNS: &str = "
    artifact.artifact_id, artifact.owner_object_type, artifact.owner_object_id,
    artifact.title, artifact.description, artifact.artifact_kind, artifact.storage_kind,
    artifact.current_version_id, artifact.created_by_actor_id,
    artifact.source_conversation_id, artifact.source_turn_id, artifact.source_item_id,
    artifact.metadata_json, artifact.created_at, artifact.updated_at";

const VERSION_COLUMNS: &str = "
    version.artifact_version_id, version.artifact_id, version.version_index, version.title,
    version.local_relative_path, version.external_url, version.media_type, version.byte_size,
    version.content_sha256, version.created_by_actor_id, version.source_conversation_id,
    version.source_turn_id, version.source_item_id, version.metadata_json, version.created_at";

impl NoemaStore {
    /// List newest directly task-owned artifacts and each current version in one bounded read.
    ///
    /// # Errors
    ///
    /// Returns an error when query validation, cursor decoding, or artifact
    /// hydration fails.
    pub async fn list_work_task_artifacts(
        &self,
        query: WorkTaskArtifactQuery,
    ) -> Result<WorkTaskArtifactConnection, StoreError> {
        let query_hash = task_artifact_query_hash(&query.task_id);
        if query
            .after
            .as_ref()
            .is_some_and(|cursor| cursor.query_hash != query_hash)
        {
            return Err(invalid_artifact_cursor());
        }
        let after_updated_at = query.after.as_ref().map(|cursor| cursor.updated_at.clone());
        let after_artifact_id = query
            .after
            .as_ref()
            .map(|cursor| cursor.artifact_id.clone());
        let task_id = query.task_id.into_string();
        let first = query.first.get();
        let limit = i64::try_from(first + 1).map_err(|_| StoreError::InvariantViolation {
            message: "task artifact page size exceeds SQLite range".to_string(),
        })?;

        self.with_connection(move |connection| {
            let sql = format!(
                "SELECT {ARTIFACT_COLUMNS}, {VERSION_COLUMNS}
                 FROM artifacts artifact
                 JOIN artifact_versions version
                   ON version.artifact_version_id = artifact.current_version_id
                  AND version.artifact_id = artifact.artifact_id
                 WHERE artifact.owner_object_type = 'task'
                   AND artifact.owner_object_id = ?1
                   AND artifact.deleted_at IS NULL
                   AND (?2 IS NULL OR artifact.updated_at < ?2
                        OR (artifact.updated_at = ?2 AND artifact.artifact_id < ?3))
                 ORDER BY artifact.updated_at DESC, artifact.artifact_id DESC
                 LIMIT ?4"
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(
                params![task_id.as_str(), after_updated_at, after_artifact_id, limit],
                |row| Ok((artifact_row_at(row, 0)?, artifact_version_row_at(row, 15)?)),
            )?;
            let mut rows = rows.collect::<Result<Vec<_>, _>>()?;
            let has_next_page = rows.len() > first;
            rows.truncate(first);
            let edges = rows
                .into_iter()
                .map(|(artifact_row, version_row)| {
                    let artifact = artifact_from_row(artifact_row)?;
                    let current_version = artifact_version_from_row(version_row)?;
                    if artifact.owner.object_type != "task"
                        || artifact.owner.object_id != task_id
                        || artifact.current_version_id.as_deref()
                            != Some(current_version.artifact_version_id.as_str())
                        || current_version.artifact_id != artifact.artifact_id
                    {
                        return Err(StoreError::InvariantViolation {
                            message: "task artifact current-version ownership is inconsistent"
                                .to_string(),
                        });
                    }
                    let cursor = WorkTaskArtifactCursor::new(
                        query_hash.clone(),
                        artifact.updated_at.clone(),
                        artifact.artifact_id.clone(),
                    )
                    .map_err(|_| StoreError::InvariantViolation {
                        message: "task artifact cursor fields are not canonical".to_string(),
                    })?
                    .encode();
                    Ok(WorkTaskArtifactEdge {
                        cursor,
                        node: WorkTaskArtifact {
                            artifact,
                            current_version,
                        },
                    })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            Ok(WorkTaskArtifactConnection {
                page_info: WorkPageInfo {
                    end_cursor: edges.last().map(|edge| edge.cursor.clone()),
                    has_next_page,
                },
                edges,
            })
        })
        .await
    }
}

fn task_artifact_query_hash(task_id: &TaskId) -> String {
    let hash = digest(
        &SHA256,
        format!("work-task-artifact-query:v1\0{}", task_id.as_str()).as_bytes(),
    );
    hash.as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn invalid_artifact_cursor() -> StoreError {
    StoreError::Work(WorkDomainError::InvalidInput {
        field: "work_task_artifact.cursor",
        message: "invalid_cursor".to_string(),
    })
}
