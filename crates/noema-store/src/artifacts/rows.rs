use noema_artifacts::{
    ArtifactOwnerRef, ArtifactRecord, ArtifactSource, ArtifactStorageKind, ArtifactVersionRecord,
    ArtifactVersionStorage, validate_external_artifact_url,
};

use super::{ArtifactRow, ArtifactVersionRow, StoreError};
use crate::sqlite::json_from_string;

pub(crate) const ARTIFACT_SELECT: &str = r#"
artifact_id, owner_object_type, owner_object_id, title, description,
artifact_kind, storage_kind, current_version_id, created_by_actor_id,
source_conversation_id, source_turn_id, source_item_id, metadata_json,
created_at, updated_at
"#;

pub(crate) const ARTIFACT_VERSION_SELECT: &str = r#"
artifact_version_id, artifact_id, version_index, title, local_relative_path,
external_url, media_type, byte_size, content_sha256, created_by_actor_id,
source_conversation_id, source_turn_id, source_item_id, metadata_json,
created_at
"#;

pub(crate) fn artifact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRow> {
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

pub(crate) fn artifact_version_row(
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

pub(crate) fn artifact_from_row(row: ArtifactRow) -> Result<ArtifactRecord, StoreError> {
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

pub(crate) fn artifact_version_from_row(
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
