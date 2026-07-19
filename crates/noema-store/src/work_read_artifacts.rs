use noema_tasks::TaskId;
use rusqlite::{Transaction, params};

use crate::{
    StoreError, WorkTaskArtifact,
    artifacts::{
        artifact_from_row, artifact_row_at, artifact_version_from_row, artifact_version_row_at,
    },
};

const DETAIL_ARTIFACT_LIMIT: i64 = 20;
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

pub(crate) fn load_recent_task_artifacts(
    transaction: &Transaction<'_>,
    task_id: &TaskId,
) -> Result<Vec<WorkTaskArtifact>, StoreError> {
    let sql = format!(
        "SELECT {ARTIFACT_COLUMNS}, {VERSION_COLUMNS}
         FROM artifacts artifact
         JOIN artifact_versions version
           ON version.artifact_version_id = artifact.current_version_id
          AND version.artifact_id = artifact.artifact_id
         WHERE artifact.owner_object_type = 'task'
           AND artifact.owner_object_id = ?1
           AND artifact.deleted_at IS NULL
         ORDER BY artifact.updated_at DESC, artifact.artifact_id DESC LIMIT ?2"
    );
    let rows = transaction
        .prepare(&sql)?
        .query_map(params![task_id.as_str(), DETAIL_ARTIFACT_LIMIT], |row| {
            Ok((artifact_row_at(row, 0)?, artifact_version_row_at(row, 15)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(artifact_row, version_row)| {
            let artifact = artifact_from_row(artifact_row)?;
            let current_version = artifact_version_from_row(version_row)?;
            if artifact.owner.object_type != "task"
                || artifact.owner.object_id != task_id.as_str()
                || artifact.current_version_id.as_deref()
                    != Some(current_version.artifact_version_id.as_str())
                || current_version.artifact_id != artifact.artifact_id
            {
                return Err(StoreError::InvariantViolation {
                    message: "task artifact current-version ownership is inconsistent".to_string(),
                });
            }
            Ok(WorkTaskArtifact {
                artifact,
                current_version,
            })
        })
        .collect()
}
