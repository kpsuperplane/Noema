//! Synchronous SQLite artifact write transactions.

use noema_artifacts::{
    ArtifactDomainError, ArtifactMetadataError, ArtifactStorageKind, ArtifactVersionRecord,
    ArtifactVersionStorage, ArtifactWithVersions, NewArtifact, NewArtifactVersion,
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::{
    artifacts::{
        ARTIFACT_SELECT, ARTIFACT_VERSION_SELECT, artifact_from_row, artifact_row,
        artifact_version_from_row, artifact_version_row,
    },
    ids::allocate_id,
};

pub(super) struct PreparedArtifactCreate {
    artifact: NewArtifact,
    initial_version: NewArtifactVersion,
    artifact_id: String,
    artifact_version_id: String,
    artifact_metadata_json: String,
    version_metadata_json: String,
    storage: VersionStorageParts,
}

pub(super) struct PreparedArtifactAppend<'a> {
    artifact_id: &'a str,
    expected_next_version_index: i64,
    version: NewArtifactVersion,
    artifact_version_id: String,
    version_metadata_json: String,
    storage: VersionStorageParts,
}

#[derive(Debug)]
pub(super) enum ArtifactTransactionError {
    Busy,
    Metadata(ArtifactMetadataError),
    AppendConflict {
        artifact_id: String,
        expected_next_version_index: i64,
        actual_next_version_index: i64,
    },
}

pub(super) fn prepare_artifact_create(
    artifact: NewArtifact,
    initial_version: NewArtifactVersion,
) -> Result<PreparedArtifactCreate, ArtifactMetadataError> {
    let artifact = artifact.validated()?;
    let initial_version = initial_version.validated()?;
    if initial_version.storage.storage_kind() != artifact.storage_kind {
        return Err(ArtifactDomainError::StorageKindMismatch.into());
    }
    let artifact_id = artifact
        .artifact_id
        .clone()
        .unwrap_or_else(|| allocate_id("artifact"));
    let artifact_version_id = initial_version
        .artifact_version_id
        .clone()
        .unwrap_or_else(|| allocate_id("artifact_version"));
    let artifact_metadata_json = serialize_metadata(&artifact.metadata)?;
    let version_metadata_json = serialize_metadata(&initial_version.metadata)?;
    let storage = VersionStorageParts::from_storage(initial_version.storage.clone());
    Ok(PreparedArtifactCreate {
        artifact,
        initial_version,
        artifact_id,
        artifact_version_id,
        artifact_metadata_json,
        version_metadata_json,
        storage,
    })
}

pub(super) fn prepare_artifact_append<'a>(
    artifact_id: &'a str,
    expected_next_version_index: i64,
    version: NewArtifactVersion,
) -> Result<PreparedArtifactAppend<'a>, ArtifactMetadataError> {
    let version = version.validated()?;
    let artifact_version_id = version
        .artifact_version_id
        .clone()
        .unwrap_or_else(|| allocate_id("artifact_version"));
    let version_metadata_json = serialize_metadata(&version.metadata)?;
    let storage = VersionStorageParts::from_storage(version.storage.clone());
    Ok(PreparedArtifactAppend {
        artifact_id,
        expected_next_version_index,
        version,
        artifact_version_id,
        version_metadata_json,
        storage,
    })
}

pub(super) fn create_artifact_transaction(
    conn: &mut rusqlite::Connection,
    prepared: &PreparedArtifactCreate,
    behavior: TransactionBehavior,
) -> Result<ArtifactWithVersions, ArtifactTransactionError> {
    let tx = conn
        .transaction_with_behavior(behavior)
        .map_err(transaction_sql_error)?;
    require_owner(&tx, &prepared.artifact.owner)?;
    tx.execute(
        r#"
            INSERT INTO artifacts (
              artifact_id, owner_object_type, owner_object_id, title, description,
              artifact_kind, storage_kind, current_version_id, created_by_actor_id,
              source_conversation_id, source_turn_id, source_item_id, metadata_json,
              created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            "#,
        params![
            prepared.artifact_id,
            prepared.artifact.owner.object_type,
            prepared.artifact.owner.object_id,
            prepared.artifact.title,
            prepared.artifact.description,
            prepared.artifact.artifact_kind,
            prepared.artifact.storage_kind.as_str(),
            prepared.artifact_version_id,
            prepared.artifact.created_by_actor_id,
            prepared.artifact.source.conversation_id,
            prepared.artifact.source.turn_id,
            prepared.artifact.source.item_id,
            prepared.artifact_metadata_json,
        ],
    )
    .map_err(transaction_sql_error)?;
    tx.execute(
        r#"
            INSERT INTO artifact_versions (
              artifact_version_id, artifact_id, version_index, title, local_relative_path,
              external_url, media_type, byte_size, content_sha256, created_by_actor_id,
              source_conversation_id, source_turn_id, source_item_id, metadata_json,
              created_at
            )
            VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            "#,
        params![
            prepared.artifact_version_id,
            prepared.artifact_id,
            prepared.initial_version.title,
            prepared.storage.local_relative_path,
            prepared.storage.external_url,
            prepared.initial_version.media_type,
            prepared.initial_version.byte_size,
            prepared.initial_version.content_sha256,
            prepared.initial_version.created_by_actor_id,
            prepared.initial_version.source.conversation_id,
            prepared.initial_version.source.turn_id,
            prepared.initial_version.source.item_id,
            prepared.version_metadata_json,
        ],
    )
    .map_err(transaction_sql_error)?;

    let artifact = tx
        .query_row(
            format!("SELECT {ARTIFACT_SELECT} FROM artifacts WHERE artifact_id = ?1 LIMIT 1")
                .as_str(),
            [&prepared.artifact_id],
            artifact_row,
        )
        .map_err(transaction_sql_error)
        .and_then(|row| {
            artifact_from_row(row)
                .map_err(|_| metadata_invariant("inserted artifact metadata could not be decoded"))
        })?;
    let version = tx
        .query_row(
            format!(
                "SELECT {ARTIFACT_VERSION_SELECT} FROM artifact_versions WHERE artifact_version_id = ?1 LIMIT 1"
            )
            .as_str(),
            [&prepared.artifact_version_id],
            artifact_version_row,
        )
        .map_err(transaction_sql_error)
        .and_then(|row| {
            artifact_version_from_row(row).map_err(|_| {
                metadata_invariant("inserted artifact version metadata could not be decoded")
            })
        })?;
    let result = ArtifactWithVersions {
        artifact,
        current_version: version.clone(),
        versions: vec![version],
    };
    tx.commit().map_err(transaction_sql_error)?;
    Ok(result)
}

pub(super) fn append_artifact_transaction(
    conn: &mut rusqlite::Connection,
    prepared: &PreparedArtifactAppend<'_>,
    behavior: TransactionBehavior,
) -> Result<ArtifactVersionRecord, ArtifactTransactionError> {
    let tx = conn
        .transaction_with_behavior(behavior)
        .map_err(transaction_sql_error)?;
    let (stored_kind, actual_next_version_index) = tx
        .query_row(
            r#"
            SELECT artifact.storage_kind,
                   COALESCE(MAX(version.version_index), 0) + 1
            FROM artifacts AS artifact
            LEFT JOIN artifact_versions AS version
              ON version.artifact_id = artifact.artifact_id
            WHERE artifact.artifact_id = ?1
              AND artifact.deleted_at IS NULL
            GROUP BY artifact.artifact_id
            LIMIT 1
            "#,
            [prepared.artifact_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(transaction_sql_error)?
        .ok_or_else(|| {
            ArtifactTransactionError::Metadata(ArtifactMetadataError::NotFound {
                artifact_id: prepared.artifact_id.to_string(),
            })
        })?;
    let actual_storage_kind = ArtifactStorageKind::parse(&stored_kind).map_err(|_| {
        metadata_invariant("stored artifact metadata contains an invalid storage kind")
    })?;
    if prepared.version.storage.storage_kind() != actual_storage_kind {
        return Err(ArtifactTransactionError::Metadata(
            ArtifactDomainError::StorageKindMismatch.into(),
        ));
    }
    if prepared.expected_next_version_index != actual_next_version_index {
        return Err(ArtifactTransactionError::AppendConflict {
            artifact_id: prepared.artifact_id.to_string(),
            expected_next_version_index: prepared.expected_next_version_index,
            actual_next_version_index,
        });
    }

    tx.execute(
        r#"
            INSERT INTO artifact_versions (
              artifact_version_id, artifact_id, version_index, title, local_relative_path,
              external_url, media_type, byte_size, content_sha256, created_by_actor_id,
              source_conversation_id, source_turn_id, source_item_id, metadata_json,
              created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            "#,
        params![
            prepared.artifact_version_id,
            prepared.artifact_id,
            actual_next_version_index,
            prepared.version.title,
            prepared.storage.local_relative_path,
            prepared.storage.external_url,
            prepared.version.media_type,
            prepared.version.byte_size,
            prepared.version.content_sha256,
            prepared.version.created_by_actor_id,
            prepared.version.source.conversation_id,
            prepared.version.source.turn_id,
            prepared.version.source.item_id,
            prepared.version_metadata_json,
        ],
    )
    .map_err(transaction_sql_error)?;
    let updated = tx
        .execute(
            "UPDATE artifacts SET current_version_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE artifact_id = ?1 AND deleted_at IS NULL",
            params![prepared.artifact_id, prepared.artifact_version_id],
        )
        .map_err(transaction_sql_error)?;
    if updated != 1 {
        return Err(metadata_invariant(
            "artifact current version update affected an unexpected row count",
        ));
    }
    let result = tx
        .query_row(
            format!(
                "SELECT {ARTIFACT_VERSION_SELECT} FROM artifact_versions WHERE artifact_version_id = ?1 LIMIT 1"
            )
            .as_str(),
            [&prepared.artifact_version_id],
            artifact_version_row,
        )
        .map_err(transaction_sql_error)
        .and_then(|row| {
            artifact_version_from_row(row).map_err(|_| {
                metadata_invariant("inserted artifact version metadata could not be decoded")
            })
        })?;
    tx.commit().map_err(transaction_sql_error)?;
    Ok(result)
}

fn require_owner(
    conn: &rusqlite::Connection,
    owner: &noema_artifacts::ArtifactOwnerRef,
) -> Result<(), ArtifactTransactionError> {
    let exists = match owner.object_type.as_str() {
        "conversation" => conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE conversation_id = ?1 AND lifecycle_status = 'active' AND deleted_at IS NULL)",
            [&owner.object_id],
            |row| row.get::<_, bool>(0),
        ),
        "task" => conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id = ?1)",
            [&owner.object_id],
            |row| row.get::<_, bool>(0),
        ),
        _ => unreachable!("prepared artifact owners are validated"),
    }
    .map_err(transaction_sql_error)?;
    if exists {
        Ok(())
    } else {
        Err(ArtifactTransactionError::Metadata(
            ArtifactDomainError::UnsupportedOwner {
                owner_object_type: owner.object_type.clone(),
                owner_object_id: owner.object_id.clone(),
            }
            .into(),
        ))
    }
}

fn transaction_sql_error(error: rusqlite::Error) -> ArtifactTransactionError {
    if matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    ) {
        ArtifactTransactionError::Busy
    } else {
        ArtifactTransactionError::Metadata(metadata_persistence_error(
            "SQLite artifact metadata operation failed",
        ))
    }
}

struct VersionStorageParts {
    local_relative_path: Option<String>,
    external_url: Option<String>,
}

impl VersionStorageParts {
    fn from_storage(storage: ArtifactVersionStorage) -> Self {
        match storage {
            ArtifactVersionStorage::LocalFile { relative_path } => Self {
                local_relative_path: Some(relative_path),
                external_url: None,
            },
            ArtifactVersionStorage::ExternalUrl { url } => Self {
                local_relative_path: None,
                external_url: Some(url),
            },
        }
    }
}

fn serialize_metadata(value: &serde_json::Value) -> Result<String, ArtifactMetadataError> {
    serde_json::to_string(value)
        .map_err(|_| metadata_persistence_error("artifact metadata JSON serialization failed"))
}

fn metadata_invariant(message: &str) -> ArtifactTransactionError {
    ArtifactTransactionError::Metadata(ArtifactMetadataError::Invariant {
        message: message.to_string(),
    })
}

pub(super) fn metadata_persistence_error(message: &str) -> ArtifactMetadataError {
    ArtifactMetadataError::Persistence {
        message: message.to_string(),
    }
}
